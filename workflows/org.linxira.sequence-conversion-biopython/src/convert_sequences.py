#!/usr/bin/env python3
"""Strict, local-only sequence conversion backed by Biopython SeqIO."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


PACK_ID = "org.linxira.sequence-conversion-biopython"
PACK_VERSION = "0.1.0"
CAPABILITY = "sequence.convert.biopython.v1"
EXPECTED_PYTHON = (3, 12)
EXPECTED_BIOPYTHON = "1.85"
EXPECTED_NUMPY = "2.2.4"
FORMATS = {"fasta", "fastq", "genbank", "embl"}
WINDOWS_FORBIDDEN_FILENAME_CHARACTERS = frozenset('<>:"/\\|?*')
WINDOWS_RESERVED_DEVICE_NAMES = frozenset(
    {
        "con",
        "prn",
        "aux",
        "nul",
        "clock$",
        "conin$",
        "conout$",
        *(f"com{number}" for number in range(1, 10)),
        *(f"lpt{number}" for number in range(1, 10)),
        "com\u00b9",
        "com\u00b2",
        "com\u00b3",
        "lpt\u00b9",
        "lpt\u00b2",
        "lpt\u00b3",
    }
)


class RequestError(ValueError):
    """A stable, user-correctable request validation failure."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_object(value: Any, context: str) -> dict[str, Any]:
    if type(value) is not dict:
        raise RequestError(f"{context} must be an object")
    return value


def require_exact_keys(
    value: dict[str, Any], required: set[str], optional: set[str], context: str
) -> None:
    missing = sorted(required - value.keys())
    unknown = sorted(value.keys() - required - optional)
    if missing:
        raise RequestError(f"{context} is missing: {', '.join(missing)}")
    if unknown:
        raise RequestError(f"{context} has unsupported fields: {', '.join(unknown)}")


def require_string(value: Any, context: str) -> str:
    if type(value) is not str or not value:
        raise RequestError(f"{context} must be a non-empty string")
    return value


def require_nonnegative_integer(value: Any, context: str) -> int:
    if type(value) is not int or value < 0:
        raise RequestError(f"{context} must be a non-negative integer")
    return value


def require_portable_output_filename(value: Any) -> str:
    filename = require_string(value, "parameters.output_filename")
    if any(
        character in WINDOWS_FORBIDDEN_FILENAME_CHARACTERS
        or ord(character) < 32
        or ord(character) == 127
        for character in filename
    ):
        raise RequestError(
            "parameters.output_filename contains a Windows-reserved character "
            "or ASCII control character"
        )
    if filename[-1] in {" ", "."}:
        raise RequestError("parameters.output_filename must not end in a space or dot")
    device_stem = filename.split(".", 1)[0].rstrip(" ").casefold()
    if device_stem in WINDOWS_RESERVED_DEVICE_NAMES:
        raise RequestError(
            "parameters.output_filename uses a Windows-reserved device name"
        )
    return filename


def paths_alias(left: Path, right: Path) -> bool:
    if left.resolve(strict=False) == right.resolve(strict=False):
        return True
    if left.exists() and right.exists():
        try:
            return os.path.samefile(left, right)
        except OSError:
            return False
    return False


def validate_request(document: Any, result_path: Path) -> dict[str, Any]:
    request = require_object(document, "request")
    require_exact_keys(
        request,
        {"schema_version", "job_id", "capability", "inputs", "execution", "parameters"},
        set(),
        "request",
    )
    if request["schema_version"] != "2":
        raise RequestError("schema_version must be '2'")
    require_string(request["job_id"], "job_id")
    if request["capability"] != CAPABILITY:
        raise RequestError(f"capability must be '{CAPABILITY}'")

    execution = require_object(request["execution"], "execution")
    require_exact_keys(execution, {"mode"}, set(), "execution")
    if execution["mode"] != "local-cpu":
        raise RequestError("execution.mode must be 'local-cpu'")

    inputs = request["inputs"]
    if type(inputs) is not list or len(inputs) != 1:
        raise RequestError("inputs must contain exactly one sequence artifact")
    artifact = require_object(inputs[0], "inputs[0]")
    require_exact_keys(
        artifact, {"artifact_id", "role", "cardinality", "files"}, set(), "inputs[0]"
    )
    require_string(artifact["artifact_id"], "inputs[0].artifact_id")
    if artifact["role"] != "sequences":
        raise RequestError("inputs[0].role must be 'sequences'")
    if artifact["cardinality"] != "single":
        raise RequestError("inputs[0].cardinality must be 'single'")
    files = artifact["files"]
    if type(files) is not list or len(files) != 1:
        raise RequestError("inputs[0].files must contain exactly one file")
    source = require_object(files[0], "inputs[0].files[0]")
    require_exact_keys(
        source,
        {"file_id", "path", "format", "compression", "size_bytes"},
        {"sha256"},
        "inputs[0].files[0]",
    )
    require_string(source["file_id"], "inputs[0].files[0].file_id")
    input_path = Path(require_string(source["path"], "inputs[0].files[0].path"))
    if not input_path.is_file():
        raise RequestError(f"input file does not exist: {input_path}")
    input_format = require_string(source["format"], "inputs[0].files[0].format")
    if input_format not in FORMATS:
        raise RequestError(f"input format must be one of: {', '.join(sorted(FORMATS))}")
    if source["compression"] != "none":
        raise RequestError("compressed input is not supported by this pack version")
    declared_size = require_nonnegative_integer(
        source["size_bytes"], "inputs[0].files[0].size_bytes"
    )
    actual_size = input_path.stat().st_size
    if declared_size != actual_size:
        raise RequestError(
            f"declared input size {declared_size} does not match actual size {actual_size}"
        )
    declared_sha256 = source.get("sha256")
    if declared_sha256 is not None:
        declared_sha256 = require_string(declared_sha256, "inputs[0].files[0].sha256")
        if len(declared_sha256) != 64 or any(c not in "0123456789abcdefABCDEF" for c in declared_sha256):
            raise RequestError("inputs[0].files[0].sha256 must contain 64 hexadecimal characters")

    parameters = require_object(request["parameters"], "parameters")
    require_exact_keys(
        parameters, {"output_directory", "output_filename", "output_format"}, set(), "parameters"
    )
    output_directory = Path(
        require_string(parameters["output_directory"], "parameters.output_directory")
    )
    output_filename = require_portable_output_filename(parameters["output_filename"])
    if output_filename.casefold() == "result.json":
        raise RequestError("parameters.output_filename must not replace result.json")
    output_format = require_string(parameters["output_format"], "parameters.output_format")
    if output_format not in FORMATS:
        raise RequestError(f"output format must be one of: {', '.join(sorted(FORMATS))}")
    if not output_directory.parent.is_dir():
        raise RequestError(f"output parent directory does not exist: {output_directory.parent}")
    if output_directory.exists():
        raise RequestError("parameters.output_directory must not already exist")
    output_directory = output_directory.resolve(strict=False)
    output_path = output_directory / output_filename
    expected_result = output_directory / "result.json"
    if result_path.resolve(strict=False) != expected_result.resolve(strict=False):
        raise RequestError("--result must be <parameters.output_directory>/result.json")
    if paths_alias(input_path, result_path):
        raise RequestError("result path must not alias the input file")

    return {
        "job_id": request["job_id"],
        "input_path": input_path,
        "input_format": input_format,
        "input_declared_size": declared_size,
        "input_declared_sha256": declared_sha256,
        "output_directory": output_directory,
        "output_filename": output_filename,
        "output_path": output_path,
        "output_format": output_format,
    }


def write_json_atomic(path: Path, document: dict[str, Any]) -> None:
    if not path.parent.is_dir():
        raise RequestError(f"result parent directory does not exist: {path.parent}")
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


def write_json_file(path: Path, document: dict[str, Any]) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        json.dump(document, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())


def write_error_result_atomic(path: Path, document: dict[str, Any]) -> bool:
    if path.parent.is_dir():
        if path.exists():
            return False
        write_json_atomic(path, document)
        return True
    grandparent = path.parent.parent
    if not grandparent.is_dir() or path.parent.exists():
        return False
    staging = Path(tempfile.mkdtemp(prefix=".linxira-sequence-error-", dir=grandparent))
    try:
        write_json_file(staging / path.name, document)
        if path.parent.exists():
            return False
        os.replace(staging, path.parent)
        return True
    finally:
        if staging.exists():
            shutil.rmtree(staging)


def convert_atomic(config: dict[str, Any], started_at: str) -> dict[str, Any]:
    if sys.version_info[:2] != EXPECTED_PYTHON:
        raise RuntimeError(
            f"locked runtime requires Python {EXPECTED_PYTHON[0]}.{EXPECTED_PYTHON[1]}, "
            f"found {sys.version_info.major}.{sys.version_info.minor}"
        )
    try:
        import Bio
        from Bio import SeqIO
    except ImportError as error:
        raise RuntimeError("locked dependency Biopython 1.85 is not installed") from error
    try:
        import numpy
    except ImportError as error:
        raise RuntimeError("locked dependency NumPy 2.2.4 is not installed") from error
    if Bio.__version__ != EXPECTED_BIOPYTHON:
        raise RuntimeError(
            f"locked dependency requires Biopython {EXPECTED_BIOPYTHON}, found {Bio.__version__}"
        )
    if numpy.__version__ != EXPECTED_NUMPY:
        raise RuntimeError(
            f"locked dependency requires NumPy {EXPECTED_NUMPY}, found {numpy.__version__}"
        )

    input_path: Path = config["input_path"]
    output_directory: Path = config["output_directory"]
    if input_path.stat().st_size != config["input_declared_size"]:
        raise RequestError("input size changed after request validation")
    input_sha256 = sha256_file(input_path)
    declared_sha256 = config["input_declared_sha256"]
    if declared_sha256 is not None and input_sha256.lower() != declared_sha256.lower():
        raise RequestError("declared input SHA-256 does not match file content")

    staging = Path(
        tempfile.mkdtemp(prefix=".linxira-sequence-conversion-", dir=output_directory.parent)
    )
    staged_output = staging / config["output_filename"]
    try:
        records = SeqIO.convert(
            str(input_path), config["input_format"], str(staged_output), config["output_format"]
        )
        with staged_output.open("rb") as handle:
            os.fsync(handle.fileno())
        if sha256_file(input_path) != input_sha256:
            raise RequestError("input file changed while conversion was running")
        result = success_result(
            config,
            started_at,
            records,
            staged_output.stat().st_size,
            input_sha256,
            sha256_file(staged_output),
        )
        write_json_file(staging / "result.json", result)
        if output_directory.exists():
            raise RequestError("output directory appeared while conversion was running")
        os.replace(staging, output_directory)
        return result
    finally:
        if staging.exists():
            shutil.rmtree(staging)


def success_result(
    config: dict[str, Any], started_at: str, records: int, size_bytes: int,
    input_sha256: str, output_sha256: str
) -> dict[str, Any]:
    lock_path = Path(__file__).resolve().parents[1] / "requirements.lock"
    return {
        "schema_version": "2",
        "job_id": config["job_id"],
        "capability": CAPABILITY,
        "status": "ok",
        "result": {
            "records_written": records,
            "input_format": config["input_format"],
            "output_format": config["output_format"],
        },
        "artifacts": [
            {
                "artifact_id": "converted-sequences",
                "role": "converted-sequences",
                "kind": "domain-file",
                "path": str(config["output_path"]),
                "format": config["output_format"],
                "size_bytes": size_bytes,
                "sha256": output_sha256,
            }
        ],
        "provenance": {
            "engine_version": PACK_VERSION,
            "execution_mode": "local-cpu",
            "started_at": started_at,
            "finished_at": utc_now(),
            "software": [
                {"name": "CPython", "version": sys.version.split()[0]},
                {"name": "Biopython", "version": EXPECTED_BIOPYTHON, "package_id": "biopython"},
                {"name": "NumPy", "version": EXPECTED_NUMPY, "package_id": "numpy"},
            ],
            "input_sha256": {"sequences": input_sha256},
            "command": ["python", "src/convert_sequences.py", "--request", "<request>", "--result", "<result>"],
            "dependency_lock_sha256": sha256_file(lock_path),
        },
        "diagnostics": [],
    }


def error_result(job_id: str, message: str, started_at: str) -> dict[str, Any]:
    return {
        "schema_version": "2",
        "job_id": job_id,
        "capability": CAPABILITY,
        "status": "error",
        "result": {},
        "artifacts": [],
        "provenance": {
            "engine_version": PACK_VERSION,
            "execution_mode": "local-cpu",
            "started_at": started_at,
            "finished_at": utc_now(),
        },
        "diagnostics": [{"code": "workflow_failed", "severity": "error", "message": message}],
    }


def parse_arguments(arguments: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Convert sequence files with locked Biopython")
    parser.add_argument("--request", required=True, type=Path, help="artifact-aware request JSON")
    parser.add_argument("--result", required=True, type=Path, help="machine-readable result JSON")
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    options = parse_arguments(arguments)
    started_at = utc_now()
    job_id = "workflow-error"
    try:
        if not options.request.is_file():
            raise RequestError(f"request file does not exist: {options.request}")
        if paths_alias(options.request, options.result):
            raise RequestError("result path must not alias the request file")
        with options.request.open("r", encoding="utf-8") as handle:
            document = json.load(handle)
        if type(document) is dict and type(document.get("job_id")) is str and document["job_id"]:
            job_id = document["job_id"]
        config = validate_request(document, options.result)
        convert_atomic(config, started_at)
        return 0
    except (OSError, json.JSONDecodeError, RequestError, RuntimeError, ValueError) as error:
        try:
            if not paths_alias(options.request, options.result):
                write_error_result_atomic(
                    options.result, error_result(job_id, str(error), started_at)
                )
        except (OSError, RequestError):
            pass
        print(f"{PACK_ID}: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
