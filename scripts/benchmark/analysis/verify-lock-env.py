#!/usr/bin/env python3
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/analysis/verify-lock-env.py
# Role: fail when the interpreter environment differs from the workflow pack's hashed lock
# Windows: 2026-10-11 (archive bench-20261011-001)
# Direction: a measurement is only reproducible if the environment that produced it is the one the
#            lock describes; this makes "the lock did not govern the run" a hard error, not a note
# Provenance index: scripts/benchmark/README.md · REPRODUCE.md
# ---------------------------------------------------------------------------
"""Compare the installed interpreter environment against a workflow pack's hashed lock.

Why this exists: in the 2026-10-11 window the Python arm ran with Biopython 1.88 and
NumPy 2.5.4 while ``requirements.lock`` pins 1.85 and 2.2.4. The numbers are what they
are, but "reproduce this" was not a truthful instruction while the lock described an
environment nobody used. Any window whose environment differs from its lock is now
invalid by construction: the harness refuses to call such a run reproducible.

Distribution metadata is read with ``importlib.metadata``, so the comparison is by
distribution name (``biopython``) rather than import name (``Bio``).

Usage:
    python scripts/benchmark/analysis/verify-lock-env.py \
        --lock workflows/org.linxira.benchmark-python/requirements.lock
    python scripts/benchmark/analysis/verify-lock-env.py --lock … --lock … --json
"""

from __future__ import annotations

import argparse
import json
import sys
from importlib import metadata
from pathlib import Path

HEADER = ("# Lock/environment gate: a run whose interpreter packages differ from the pack "
          "lock is not reproducible.")


def parse_lock(lock: Path) -> dict[str, str]:
    """Return {distribution: pinned version} from a pip-compile style hashed lock."""
    pinned: dict[str, str] = {}
    for raw in lock.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or line.startswith("--"):
            continue
        # Lock lines look like `name==1.2.3 \` followed by indented --hash lines.
        if "==" not in line:
            continue
        name, _, version = line.partition("==")
        version = version.rstrip("\\").strip().split()[0] if version.strip() else ""
        name = name.strip().strip('"').strip("'")
        if name and version:
            pinned[name] = version
    return pinned


def installed_version(distribution: str) -> str | None:
    try:
        return metadata.version(distribution)
    except metadata.PackageNotFoundError:
        return None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", required=True, action="append", type=Path,
                        help="hashed requirements lock (repeatable)")
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    arguments = parser.parse_args()

    report: list[dict[str, str]] = []
    failures = 0
    for lock in arguments.lock:
        if not lock.is_file():
            print(f"lock not found: {lock}", file=sys.stderr)
            raise SystemExit(2)
        for distribution, pinned in sorted(parse_lock(lock).items()):
            found = installed_version(distribution)
            status = ("match" if found == pinned
                      else "absent" if found is None
                      else "differs")
            failures += 0 if status == "match" else 1
            report.append({"lock": str(lock), "distribution": distribution,
                           "pinned": pinned, "installed": found or "", "status": status})

    if arguments.json:
        print(json.dumps({"checks": report, "failures": failures}, indent=2))
    else:
        print(HEADER)
        print(f"{'distribution':22s} {'lock':12s} {'installed':12s} status")
        for row in report:
            print(f"{row['distribution']:22s} {row['pinned']:12s} {row['installed'] or '-':12s} {row['status']}")
    if failures:
        print(f"\n{failures} of {len(report)} packages do not match the lock. A measurement whose "
              "environment differs from its lock is not reproducible: either install from the lock "
              "(pip install --require-hashes -r <lock>) or re-pin the lock to the environment that "
              "actually ran, and re-run.", file=sys.stderr)
    raise SystemExit(1 if failures else 0)


if __name__ == "__main__":
    main()
