#!/usr/bin/env python3
"""Write benchmark results back into the runtime preference table.

Usage:
    python scripts/update-runtime-preferences.py <capability.benchmark.json> [...]
    python scripts/update-runtime-preferences.py --check <report.json> [...]
    python scripts/update-runtime-preferences.py --file PATH <report.json> [...]

For every report with verdict `consistent`, the fastest backend (lowest
median wall time among backends with successful runs; ties go to `rust`)
becomes the `default_backend` for that capability. Entries for the same
(capability, dataset_class) pair are replaced, other entries are kept, and
the result is validated against `schemas/runtime-preferences.schema.json`
before an atomic write. Inconsistent or failed reports are skipped with a
message so a batch run never poisons the table.

This is the write-back step of ROADMAP M2-T7: `run-benchmark-linux.sh` calls
it after each capability so the table always reflects the newest consistent
server measurement, and the worker/CLI consult it for backend selection
(`auto`).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "schemas" / "runtime-preferences.schema.json"
DEFAULT_TABLE = ROOT / "runtime-preferences.json"
BACKEND_ORDER = {"rust": 0, "python": 1, "r": 2}


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def entry_from_report(report: dict) -> tuple[dict | None, str]:
    capability = report.get("capability")
    dataset_class = report.get("dataset_class", "other")
    consistency = report.get("consistency")
    if consistency != "consistent":
        return None, (
            f"skip {capability} [{dataset_class}]: verdict {consistency!r} must not drive defaults"
        )
    measured = {}
    candidates = []
    rust_wall = None
    for backend in report.get("backends", []):
        name = backend.get("backend")
        runs_ok = any(run.get("ok") for run in backend.get("runs", []))
        if not name or not runs_ok:
            continue
        measured[name] = {
            "wall_ms": backend.get("median_wall_ms"),
            "peak_rss_mb": backend.get("median_peak_rss_mb"),
        }
        candidates.append(name)
        if name == "rust":
            rust_wall = backend.get("median_wall_ms")
    if not candidates:
        return None, f"skip {capability} [{dataset_class}]: no backend produced a successful run"

    default = min(candidates, key=lambda name: (BACKEND_ORDER.get(name, 99), measured[name]["wall_ms"]))
    speedup = None
    if default != "rust" and rust_wall:
        speedup = measured[default]["wall_ms"] / rust_wall
    entry = {
        "capability": capability,
        "dataset_class": dataset_class,
        "default_backend": default,
        "measured": measured,
        "sampled_on": report.get("sampled_at"),
        "speedup": speedup,
        "consistency": consistency,
    }
    detail = ", ".join(
        f"{name}={measured[name]['wall_ms']:.1f}ms" for name in sorted(measured, key=lambda n: BACKEND_ORDER.get(n, 99))
    )
    return entry, f"{capability} [{dataset_class}]: default_backend={default} ({detail})"


def merge(table: dict, entry: dict) -> bool:
    preferences = table.setdefault("preferences", [])
    for index, existing in enumerate(preferences):
        if (
            existing.get("capability") == entry["capability"]
            and existing.get("dataset_class") == entry.get("dataset_class")
        ):
            preferences[index] = entry
            return False
    preferences.append(entry)
    return True


def write_atomic(path: Path, document: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(document, handle, indent=2, ensure_ascii=False)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("reports", nargs="+", type=Path, help="benchmark report JSON files")
    parser.add_argument(
        "--file", type=Path, default=DEFAULT_TABLE, help=f"preference table (default {DEFAULT_TABLE})"
    )
    parser.add_argument(
        "--check", action="store_true", help="print the resulting entries without writing"
    )
    options = parser.parse_args(argv)

    try:
        from jsonschema import Draft202012Validator
    except ImportError:
        Draft202012Validator = None  # type: ignore[assignment]

    schema = load_json(SCHEMA_PATH)
    validator = Draft202012Validator(schema) if Draft202012Validator else None

    if options.file.is_file():
        table = load_json(options.file)
    else:
        print(f"{options.file} does not exist yet; starting an empty table")
        table = {"schema_version": 1, "preferences": []}
    written = 0
    for report_path in options.reports:
        entry, message = entry_from_report(load_json(report_path))
        if entry is None:
            print(message)
            continue
        if options.check:
            print(f"[check] {message}")
        else:
            appended = merge(table, entry)
            print(("added " if appended else "replaced ") + message)
            written += 1

    if validator is not None:
        errors = sorted(validator.iter_errors(table), key=str)
        if errors:
            for error in errors:
                print(f"schema violation: {error.message}", file=sys.stderr)
            return 1

    if options.check:
        return 0
    if written:
        write_atomic(options.file, table)
        print(f"wrote {options.file} ({len(table['preferences'])} entries)")
    else:
        print("nothing to write")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
