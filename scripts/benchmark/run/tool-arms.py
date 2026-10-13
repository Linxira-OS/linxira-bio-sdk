#!/usr/bin/env python3
"""Run an ecosystem tool under the same protocol as the parity arms (tool arm).

`benchmark.run.v1` can only measure implementations we ship: it drives capability packs by
capability id. An ecosystem tool (pyfastx, seqkit, fastp, R `prcomp`) is not a backend of
ours - it has its own CLI, input expectations and output. This runner gives it the same
measurement treatment: GNU `time -v` for wall/RSS, one discarded warmup run, N timed
repeats, the tool's version captured by running its own version command, the exact argv
recorded, and a report that validates against `schemas/benchmark-report.schema.json` with
`arm_class: "tool"`.

Why a separate producer rather than a new capability id: the capability catalog is a
product contract that tells users what they can run, not a benchmark ledger.

Deliberately not done here: no ratio against our own engine is written into the report
(`speedup` and `memory_saving` stay null) and `consistency` is `not-applicable`, because a
tool arm has no cross-language twin. Comparing the two arm classes is a presentation
decision that ROADMAP §7.1 restricts to separate, labelled tables.

Usage:
    scripts/benchmark/run/tool-arms.py --list
    scripts/benchmark/run/tool-arms.py --tool pyfastx-stat \\
        --input benchmark-results/2026-10-11/inputs/big.fa \\
        --repeat 10 --output benchmark-results/2026-10-11/tool-arms
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import shutil
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
DEFAULT_REGISTRY = SCRIPT_DIR / "tool-arms" / "tools.json"
TIME_BINARY = Path("/usr/bin/time")
MINIMUM_REPEATS = 10


class ToolArmError(RuntimeError):
    """Anything that should stop the run with a message the operator can act on."""


def load_registry(path: Path) -> dict:
    if not path.is_file():
        raise ToolArmError(f"tool registry not found: {path}")
    registry = json.loads(path.read_text(encoding="utf-8"))
    if registry.get("schema_version") != "1":
        raise ToolArmError(f"unsupported registry schema_version in {path}")
    return registry


def resolve_command(command: list[str], input_path: Path) -> list[str]:
    """Substitute placeholders; never build a shell string."""
    return [
        argument.replace("{input}", str(input_path)).replace("{script_dir}", str(SCRIPT_DIR))
        for argument in command
    ]


def run_capture(argv: list[str], use_time: bool, stdout_path: Path | None = None) -> dict:
    """One execution with the same metric sources the harness uses."""
    started = time.monotonic()
    if use_time:
        completed = subprocess.run(
            [str(TIME_BINARY), "-v", *argv],
            capture_output=True,
            text=True,
            check=False,
        )
        stderr = completed.stderr
    else:
        completed = subprocess.run(argv, capture_output=True, text=True, check=False)
        stderr = completed.stderr
    instant_wall_ms = (time.monotonic() - started) * 1000.0
    metrics = parse_time_verbose(stderr) if use_time else None
    output_bytes = len(completed.stdout.encode("utf-8")) + len(completed.stderr.encode("utf-8"))
    if stdout_path is not None:
        stdout_path.write_text(completed.stdout, encoding="utf-8", newline="")
        output_bytes += stdout_path.stat().st_size if stdout_path.exists() else 0
    return {
        "wall_ms": metrics["wall_ms"] if metrics else instant_wall_ms,
        "cpu_ms": metrics["cpu_ms"] if metrics else None,
        "peak_rss_mb": metrics["peak_rss_mb"] if metrics else None,
        "disk_read_bytes": metrics.get("disk_read_bytes") if metrics else None,
        "disk_write_bytes": metrics.get("disk_write_bytes") if metrics else None,
        "output_bytes": output_bytes,
        "ok": completed.returncode == 0,
        "error_summary": None if completed.returncode == 0 else completed.stderr[-2000:],
        "stdout_tail": completed.stdout[-800:],
    }


def parse_time_verbose(stderr: str) -> dict | None:
    """Parse GNU `time -v` output.

    Field labels are matched explicitly rather than split on the first colon: the elapsed
    line's own label contains colons ("Elapsed (wall clock) time (h:mm:ss or m:ss):") and
    its value does too ("0:01.23"), so a naive split yields a None wall time. That bug was
    caught by the first real tool-arm run, which is why the parser is exercised by a test
    with verbatim GNU time output.
    """
    patterns = {
        "wall": re.compile(r"^\s*Elapsed \(wall clock\) time \(h:mm:ss or m:ss\):\s*(?P<value>.+?)\s*$", re.MULTILINE),
        "user": re.compile(r"^\s*User time \(seconds\):\s*(?P<value>[0-9.]+)\s*$", re.MULTILINE),
        "system": re.compile(r"^\s*System time \(seconds\):\s*(?P<value>[0-9.]+)\s*$", re.MULTILINE),
        "rss": re.compile(r"^\s*Maximum resident set size \(kbytes\):\s*(?P<value>[0-9]+)\s*$", re.MULTILINE),
        "reads": re.compile(r"^\s*File system inputs:\s*(?P<value>[0-9]+)\s*$", re.MULTILINE),
        "writes": re.compile(r"^\s*File system outputs:\s*(?P<value>[0-9]+)\s*$", re.MULTILINE),
    }
    matched = {key: pattern.search(stderr) for key, pattern in patterns.items()}
    if matched["rss"] is None:
        return None

    def number(key: str, cast):
        found = matched[key]
        if found is None:
            return None
        try:
            return cast(found.group("value"))
        except ValueError:
            return None

    def elapsed_ms() -> float | None:
        raw = number("wall", str)
        if raw is None:
            return None
        total = 0.0
        try:
            for part in raw.split(":"):
                total = total * 60.0 + float(part)
        except ValueError:
            return None
        return total * 1000.0

    user = number("user", float) or 0.0
    system = number("system", float) or 0.0
    rss_kb = number("rss", int)
    return {
        "wall_ms": elapsed_ms(),
        "cpu_ms": (user + system) * 1000.0,
        "peak_rss_mb": rss_kb / 1024.0 if rss_kb is not None else None,
        "disk_read_bytes": number("reads", int),
        "disk_write_bytes": number("writes", int),
    }


def median_of(values: list[float]) -> float:
    ordered = sorted(values)
    count = len(ordered)
    if count % 2:
        return ordered[count // 2]
    return statistics.fmean(ordered[count // 2 - 1 : count // 2 + 1])


def quartiles(values: list[float]) -> tuple[float, float]:
    """Same split-half rule the analysis tables use, so IQR means one thing repo-wide."""
    ordered = sorted(values)
    count = len(ordered)
    if count < 2:
        return ordered[0], ordered[0]
    middle = count // 2
    lower = ordered[:middle]
    upper = ordered[middle + 1 :] if count % 2 else ordered[middle:]
    return median_of(lower or ordered), median_of(upper or ordered)


def tool_version(version_command: list[str]) -> str:
    completed = subprocess.run(version_command, capture_output=True, text=True, check=False)
    text = (completed.stdout or completed.stderr).strip().splitlines()
    if completed.returncode != 0 or not text:
        raise ToolArmError(
            f"could not read the tool version with {version_command!r}: {completed.stderr.strip()}"
        )
    return text[0].strip()


def environment_block(code_revision: str | None) -> dict:
    def os_release(key: str) -> str | None:
        release = Path("/etc/os-release")
        if not release.is_file():
            return None
        for line in release.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith(f"{key}="):
                return line.split("=", 1)[1].strip().strip('"') or None
        return None

    total_memory_mb = None
    meminfo = Path("/proc/meminfo")
    if meminfo.is_file():
        for line in meminfo.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("MemTotal:"):
                total_memory_mb = int(int(line.split()[1]) / 1024)
                break

    return {
        "os": platform.system().lower(),
        "kernel": platform.release() or None,
        "cpu_model": next(
            (
                line.split(":", 1)[1].strip()
                for line in Path("/proc/cpuinfo").read_text(encoding="utf-8", errors="replace").splitlines()
                if line.startswith("model name")
            ),
            None,
        )
        if Path("/proc/cpuinfo").is_file()
        else None,
        "total_memory_mb": total_memory_mb,
        "distro": os_release("PRETTY_NAME"),
        "engine_version": "tool-arm-runner/1",
        "python_version": sys.version.split()[0],
        "r_version": None,
        "in_container": Path("/.dockerenv").exists(),
        "page_cache": "warm",
        "precision": "high" if TIME_BINARY.is_file() else "degraded",
        "code_revision": code_revision,
        "code_branch": git_label(["rev-parse", "--abbrev-ref", "HEAD"]),
        "code_describe": git_label(["describe", "--tags", "--always", "--dirty"]),
    }


def git_label(arguments: list[str]) -> str | None:
    """Run one git probe for the report's code label; None when git cannot answer."""
    completed = subprocess.run(
        ["git", *arguments],
        capture_output=True,
        text=True,
        check=False,
        cwd=SCRIPT_DIR,
    )
    value = completed.stdout.strip()
    return value or None


def git_revision() -> str | None:
    return git_label(["rev-parse", "HEAD"])


def validate_report(report: dict, schema_path: Path) -> None:
    try:
        import jsonschema  # type: ignore
    except ImportError:  # pragma: no cover - the CI venv has it
        return
    if not schema_path.is_file():
        raise ToolArmError(f"report schema not found: {schema_path}")
    schema = json.loads(schema_path.read_text(encoding="utf-8"))
    jsonschema.validate(instance=report, schema=schema)


def describe_input(path: Path | None) -> dict[str, object] | None:
    """Record the bytes a measurement ran on, so the number can be re-derived.

    The parity windows carry this identity at the archive level (descriptor.json plus
    inputs.sha256); a tool-arm report is standalone and lands on its own, so it holds
    its own copy of the input path, size and digest.
    """
    if path is None:
        return None
    digest = hashlib.sha256()
    size = 0
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
                size += len(chunk)
    except OSError:
        return {
            "path": str(path),
            "bytes": None,
            "sha256": None,
            "note": "input was not readable when the report was written",
        }
    return {"path": str(path), "bytes": size, "sha256": digest.hexdigest()}


def build_report(
    tool_key: str,
    entry: dict,
    arguments: argparse.Namespace,
    version: str,
    invocation: str,
    runs: list[dict],
    use_time: bool,
) -> dict:
    successful = [run for run in runs if run["ok"]]
    if not successful:
        last = runs[-1].get("error_summary") or "<no stderr>"
        raise ToolArmError(f"every run failed; last stderr:\n{last.strip()}")
    wall = [run["wall_ms"] for run in successful]
    low, high = quartiles(wall)
    rss = [run["peak_rss_mb"] for run in successful if run["peak_rss_mb"] is not None]
    cpu = [run["cpu_ms"] for run in successful if run["cpu_ms"] is not None]
    runs_payload = [
        {
            "backend": tool_key,
            "run_index": index + 1,
            "wall_ms": run["wall_ms"],
            "cpu_ms": run["cpu_ms"],
            "peak_rss_mb": run["peak_rss_mb"],
            "disk_read_bytes": run["disk_read_bytes"],
            "disk_write_bytes": run["disk_write_bytes"],
            "output_bytes": run["output_bytes"],
            "ok": run["ok"],
            "error_summary": run["error_summary"],
        }
        for index, run in enumerate(runs)
    ]
    findings = [
        {
            "field": "arm_class",
            "detail": (
                "tool arm: an ecosystem tool measured on the same machine, input and "
                "protocol as the parity arms. No cross-implementation comparison is stored "
                "in this report (speedup/memory_saving are null) because ROADMAP 7.1 keeps "
                "the two arm classes in separate, labelled tables."
            ),
        },
        {
            "field": "engine_version",
            "detail": "names the tool-arm runner, not the analysis engine, for this report",
        },
        {
            "field": "parity_comparison",
            "detail": (
                f"the arm this tool stands in for is `{entry['capability']}`; "
                "the parity implementation of that capability is hand-written in each language"
            ),
        },
    ]
    if not use_time:
        findings.append(
            {
                "field": "precision",
                "detail": "GNU time is unavailable: wall time only, peak RSS is null",
            }
        )
    raw_input = getattr(arguments, "input", None)
    # Record the path the tool was actually invoked on (resolved), not the string as
    # typed: the invocation and the runs already use the resolved path.
    input_path = str(Path(raw_input).resolve()) if raw_input else None
    input_record = describe_input(Path(input_path) if input_path else None)
    return {
        "schema_version": "1",
        "capability": entry["capability"],
        "inputs": [input_record] if input_record else [],
        "dataset_class": entry.get("dataset_class", "other"),
        "arm_class": "tool",
        "tool": {"name": entry["tool_name"], "version": version, "invocation": invocation},
        "backends": [
            {
                "backend": tool_key,
                "repeats": len(successful),
                "median_wall_ms": median_of(wall),
                "min_wall_ms": min(wall),
                "max_wall_ms": max(wall),
                "iqr_wall_ms": high - low,
                "median_peak_rss_mb": median_of(rss) if rss else None,
                "median_cpu_ms": median_of(cpu) if cpu else None,
                "runs": runs_payload,
            }
        ],
        "speedup": None,
        "memory_saving": None,
        "consistency": "not-applicable",
        "findings": findings,
        "environment": environment_block(git_revision()),
        "sampled_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    }


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--tool", help="registry key of the tool to measure")
    parser.add_argument("--list", action="store_true", help="list registry entries and exit")
    parser.add_argument("--input", help="input file the tool runs on")
    parser.add_argument("--repeat", type=int, default=MINIMUM_REPEATS)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--output", help="directory for the report (default: benchmark-results)")
    parser.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    parser.add_argument(
        "--allow-short",
        action="store_true",
        help=f"permit fewer than {MINIMUM_REPEATS} repeats (smoke checks only)",
    )
    parser.add_argument("--dataset-class", help="override the registry's dataset_class")
    arguments = parser.parse_args(argv)

    registry = load_registry(arguments.registry)
    tools = registry.get("tools", {})
    if arguments.list:
        for key, entry in sorted(tools.items()):
            print(f"{key}\t{entry['tool_name']}\t{entry['capability']}")
        return 0
    if not arguments.tool:
        raise SystemExit("either --list or --tool <key> is required")
    if arguments.tool not in tools:
        raise SystemExit(f"unknown tool {arguments.tool!r}; run --list")
    if not arguments.input:
        raise SystemExit("--input is required")
    if arguments.repeat < MINIMUM_REPEATS and not arguments.allow_short:
        raise SystemExit(
            f"refusing {arguments.repeat} repeats: the methodology requires at least "
            f"{MINIMUM_REPEATS}; pass --allow-short only for a smoke check"
        )

    entry = dict(tools[arguments.tool])
    if arguments.dataset_class:
        entry["dataset_class"] = arguments.dataset_class
    input_path = Path(arguments.input).resolve()
    if not input_path.is_file():
        raise SystemExit(f"input not found: {input_path}")

    executable = resolve_command(entry["run_command"], input_path)[0]
    if shutil.which(executable) is None and not Path(executable).is_file():
        raise SystemExit(
            f"{executable!r} is not on PATH: install the tool (and record its version) "
            "before measuring; the registry entry names the exact argv"
        )

    version = tool_version(entry["version_command"])
    use_time = TIME_BINARY.is_file()
    invocation = " ".join(resolve_command(entry["run_command"], input_path))

    for _ in range(max(arguments.warmup, 0)):
        run_capture(resolve_command(entry["run_command"], input_path), use_time)

    runs = [
        run_capture(resolve_command(entry["run_command"], input_path), use_time)
        for _ in range(arguments.repeat)
    ]
    report = build_report(arguments.tool, entry, arguments, version, invocation, runs, use_time)

    schema_path = SCRIPT_DIR.parent.parent.parent / "schemas" / "benchmark-report.schema.json"
    validate_report(report, schema_path)

    output_dir = Path(arguments.output or "benchmark-results")
    output_dir.mkdir(parents=True, exist_ok=True)
    report_path = output_dir / f"{entry['capability']}.{arguments.tool}.tool-arm.json"
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")

    backend = report["backends"][0]
    print(f"report\t{report_path}")
    print(f"tool\t{report['tool']['name']}\t{report['tool']['version']}")
    print(
        f"median_wall_ms\t{backend['median_wall_ms']:.1f}\t"
        f"repeats\t{backend['repeats']}\t"
        f"peak_rss_mb\t{backend['median_peak_rss_mb']}"
    )
    # The report drops stdout (the schema forbids unknown keys); the operator still wants
    # to see what the tool printed, so read it from the in-memory run list.
    if runs and runs[-1]["stdout_tail"]:
        print(f"tool_output\t{runs[-1]['stdout_tail'].strip()[:200]}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except ToolArmError as error:
        print(f"tool-arm runner: {error}", file=sys.stderr)
        sys.exit(1)
