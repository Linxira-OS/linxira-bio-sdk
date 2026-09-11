#!/usr/bin/env python3
"""Benchmark harness: the Python backend of `linxira-bio benchmark run`.

The worker invokes this pack when a request carries
``execution.backend = "python"``. It runs the independent Python
implementation of the requested capability (see ``implementations/``), writes
the same V2 result envelope the Rust engine produces, and self-reports the
in-process wall time and peak RSS as an ``info`` diagnostic so the report can
disclose interpreter start-up separately from the analysis itself
(ROADMAP M2-T3, methodology §7.1).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))

from implementations import IMPLEMENTATIONS  # noqa: E402

PACK_ID = "org.linxira.benchmark-python"
PACK_VERSION = "0.1.0"
BACKEND = "python"
SELF_REPORTED_CODE = "benchmark.self_reported"
LOCKED_BIOPYTHON = "1.85"


class RequestError(ValueError):
    """A stable, user-correctable request validation failure."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def core_version() -> str:
    return os.environ.get("LINXIRA_BIO_CORE_VERSION", "unknown")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def peak_rss_mb() -> float | None:
    """Peak resident set size of this process, or None where unavailable.

    ``resource`` does not exist on Windows; the report then keeps the field
    null instead of inventing a number (methodology §7.1: no fabricated I/O
    or memory figures).
    """
    try:
        import resource
    except ImportError:
        return None
    usage = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    if sys.platform == "darwin":
        return usage / (1024.0 * 1024.0)
    return usage / 1024.0


def require_object(value: Any, context: str) -> dict[str, Any]:
    if type(value) is not dict:
        raise RequestError(f"{context} must be an object")
    return value


def require_string(value: Any, context: str) -> str:
    if type(value) is not str or not value:
        raise RequestError(f"{context} must be a non-empty string")
    return value


def validate_request(document: Any, result_path: Path) -> dict[str, Any]:
    request = require_object(document, "request")
    if request.get("schema_version") != "2":
        raise RequestError("request schema_version must be \"2\"")
    job_id = require_string(request.get("job_id"), "job_id")
    capability = require_string(request.get("capability"), "capability")
    implementation = IMPLEMENTATIONS.get(capability)
    if implementation is None:
        supported = ", ".join(sorted(IMPLEMENTATIONS))
        raise RequestError(
            f"{PACK_ID} has no python implementation of {capability}; supported: {supported}"
        )
    execution = require_object(request.get("execution"), "execution")
    backend = execution.get("backend", BACKEND)
    if backend != BACKEND:
        raise RequestError(f"execution.backend must be {BACKEND!r}, got {backend!r}")

    inputs = request.get("inputs")
    if type(inputs) is not list:
        raise RequestError("inputs must be an array")
    resolved: dict[str, Path] = {}
    declared_sha256: dict[str, str | None] = {}
    for index, artifact in enumerate(inputs):
        artifact = require_object(artifact, f"inputs[{index}]")
        role = require_string(artifact.get("role"), f"inputs[{index}].role")
        if role not in implementation.INPUT_ROLES:
            raise RequestError(f"{capability} does not accept input role {role}")
        if role in resolved:
            raise RequestError(f"duplicate input role: {role}")
        if artifact.get("cardinality") != "single":
            raise RequestError(f"input role {role} requires cardinality \"single\"")
        files = artifact.get("files")
        if type(files) is not list or len(files) != 1:
            raise RequestError(f"input role {role} requires exactly one file")
        file = require_object(files[0], f"inputs[{index}].files[0]")
        path = Path(require_string(file.get("path"), f"inputs[{index}].files[0].path"))
        if not path.is_file():
            raise RequestError(f"input file does not exist: {path}")
        if file.get("compression", "none") != "none":
            raise RequestError("the benchmark harness reads uncompressed inputs only")
        sha = file.get("sha256")
        if sha is not None and (type(sha) is not str or len(sha) != 64):
            raise RequestError(f"inputs[{index}].files[0].sha256 must be a 64-character hex string")
        resolved[role] = path
        declared_sha256[role] = sha
    missing = [role for role in implementation.INPUT_ROLES if role not in resolved]
    if missing:
        raise RequestError(f"{capability} requires inputs: {', '.join(missing)}")

    parameters = require_object(request.get("parameters", {}), "parameters")
    output_directory = Path(
        require_string(parameters.get("output_directory"), "parameters.output_directory")
    )
    if not output_directory.is_absolute():
        raise RequestError("parameters.output_directory must be an absolute path")
    for name in parameters:
        if name != "output_directory" and name not in implementation.PARAMETERS:
            raise RequestError(f"{capability} does not accept parameter {name}")
    if result_path.resolve().parent != output_directory.resolve():
        raise RequestError("the result path must live directly inside parameters.output_directory")
    if not output_directory.parent.is_dir():
        raise RequestError(f"output parent directory does not exist: {output_directory.parent}")
    for path in resolved.values():
        if path.resolve() == output_directory.resolve():
            raise RequestError("output directory must differ from every input")

    return {
        "job_id": job_id,
        "capability": capability,
        "implementation": implementation,
        "inputs": resolved,
        "declared_sha256": declared_sha256,
        "parameters": {k: v for k, v in parameters.items() if k != "output_directory"},
        "output_directory": output_directory,
        "result_path": result_path,
    }


def write_json_file(path: Path, document: dict[str, Any]) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        json.dump(document, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())


def write_json_atomic(path: Path, document: dict[str, Any]) -> None:
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(document, handle, ensure_ascii=False, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, path)
    except BaseException:
        temporary_path.unlink(missing_ok=True)
        raise


def write_error_result_atomic(path: Path, document: dict[str, Any]) -> bool:
    if path.parent.is_dir():
        if path.exists():
            return False
        write_json_atomic(path, document)
        return True
    grandparent = path.parent.parent
    if not grandparent.is_dir() or path.parent.exists():
        return False
    staging = Path(tempfile.mkdtemp(prefix=".linxira-benchmark-error-", dir=grandparent))
    try:
        write_json_file(staging / path.name, document)
        if path.parent.exists():
            return False
        os.replace(staging, path.parent)
        return True
    finally:
        if staging.exists():
            shutil.rmtree(staging)


def software_provenance(implementation: Any) -> tuple[list[dict[str, str]], list[dict[str, Any]]]:
    entries: list[dict[str, str]] = [{"name": "CPython", "version": sys.version.split()[0]}]
    diagnostics: list[dict[str, Any]] = []
    for entry in implementation.software():
        entries.append(entry)
        if entry.get("package_id") == "biopython" and entry["version"] != LOCKED_BIOPYTHON:
            diagnostics.append(
                {
                    "code": "dependency_version_drift",
                    "severity": "warning",
                    "message": (
                        f"Biopython {entry['version']} is installed; the pack lock pins "
                        f"{LOCKED_BIOPYTHON}. Numbers remain comparable but the environment "
                        "disclosure records the installed version."
                    ),
                }
            )
    return entries, diagnostics


def run_benchmarked(config: dict[str, Any], started_at: str) -> dict[str, Any]:
    implementation = config["implementation"]
    capability: str = config["capability"]
    output_directory: Path = config["output_directory"]
    # The worker has already verified every declared hash against the file
    # content and re-checks the inputs after the pack exits, so the declared
    # digest is reused here: hashing a multi-terabyte input a third time would
    # only inflate the outer wall time of this backend. Standalone runs without
    # a declared digest compute it.
    input_sha256: dict[str, str] = {}
    for role, path in config["inputs"].items():
        declared = config["declared_sha256"][role]
        input_sha256[role] = declared.lower() if declared is not None else sha256_file(path)

    # Instrumentation brackets the analysis only. Interpreter start-up,
    # request validation and output writing are visible to the outer timer
    # (`/usr/bin/time -v` around the worker) but not to this one.
    started = time.perf_counter()
    result = implementation.run(config["inputs"], config["parameters"])
    wall_ms = (time.perf_counter() - started) * 1000.0
    peak = peak_rss_mb()

    software, diagnostics = software_provenance(implementation)
    diagnostics.append(
        {
            "code": SELF_REPORTED_CODE,
            "severity": "info",
            "message": json.dumps(
                {
                    "wall_ms": round(wall_ms, 3),
                    "peak_rss_mb": None if peak is None else round(peak, 3),
                    "instrument": "time.perf_counter + resource.getrusage(RUSAGE_SELF)",
                    "backend": BACKEND,
                },
                sort_keys=True,
            ),
        }
    )

    staging = Path(tempfile.mkdtemp(prefix=".linxira-benchmark-", dir=output_directory.parent))
    try:
        artifact_name = f"{capability}.result.json"
        staged_artifact = staging / artifact_name
        write_json_file(staged_artifact, result)
        lock_path = Path(__file__).resolve().parents[1] / "requirements.lock"
        envelope = {
            "schema_version": "2",
            "job_id": config["job_id"],
            "capability": capability,
            "status": "ok",
            "result": result,
            "artifacts": [
                {
                    "artifact_id": "benchmark-result",
                    "role": "benchmark-result",
                    "kind": "report",
                    "path": str(output_directory / artifact_name),
                    "format": "json",
                    "media_type": "application/json",
                    "size_bytes": staged_artifact.stat().st_size,
                    "sha256": sha256_file(staged_artifact),
                }
            ],
            "provenance": {
                "engine_version": PACK_VERSION,
                "execution_mode": "local-cpu",
                "core_version": core_version(),
                "started_at": started_at,
                "finished_at": utc_now(),
                "software": software,
                "input_sha256": input_sha256,
                "command": [
                    "python",
                    "src/benchmark_harness.py",
                    "--request",
                    "<request>",
                    "--result",
                    "<result>",
                ],
                "dependency_lock_sha256": sha256_file(lock_path),
            },
            "diagnostics": diagnostics,
        }
        write_json_file(staging / config["result_path"].name, envelope)
        if output_directory.exists():
            raise RequestError("output directory appeared while the analysis was running")
        os.replace(staging, output_directory)
        return envelope
    finally:
        if staging.exists():
            shutil.rmtree(staging)


def error_result(job_id: str, capability: str, message: str, started_at: str) -> dict[str, Any]:
    return {
        "schema_version": "2",
        "job_id": job_id,
        "capability": capability,
        "status": "error",
        "result": {},
        "artifacts": [],
        "provenance": {
            "engine_version": PACK_VERSION,
            "execution_mode": "local-cpu",
            "core_version": core_version(),
            "started_at": started_at,
            "finished_at": utc_now(),
        },
        "diagnostics": [{"code": "workflow_failed", "severity": "error", "message": message}],
    }


def parse_arguments(arguments: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the independent Python implementation of a Linxira Bio capability"
    )
    parser.add_argument("--request", required=True, type=Path, help="artifact-aware request JSON")
    parser.add_argument("--result", required=True, type=Path, help="result envelope JSON")
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    options = parse_arguments(arguments)
    started_at = utc_now()
    job_id = "workflow-error"
    capability = "unknown"
    try:
        if not options.request.is_file():
            raise RequestError(f"request file does not exist: {options.request}")
        if options.request.resolve() == options.result.resolve():
            raise RequestError("result path must not alias the request file")
        with options.request.open("r", encoding="utf-8") as handle:
            document = json.load(handle)
        if type(document) is dict:
            if type(document.get("job_id")) is str and document["job_id"]:
                job_id = document["job_id"]
            if type(document.get("capability")) is str and document["capability"]:
                capability = document["capability"]
        config = validate_request(document, options.result)
        run_benchmarked(config, started_at)
        return 0
    except (OSError, json.JSONDecodeError, RequestError, RuntimeError, ValueError) as error:
        try:
            if options.request.resolve() != options.result.resolve():
                write_error_result_atomic(
                    options.result, error_result(job_id, capability, str(error), started_at)
                )
        except (OSError, RequestError):
            pass
        print(f"{PACK_ID}: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
