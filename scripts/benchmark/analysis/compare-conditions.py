#!/usr/bin/env python3
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/analysis/compare-conditions.py
# Role: compare the same capabilities measured under two conditions (contended vs clean host, later serial vs parallel) and say whether any row moved beyond run-to-run noise
# Windows: 2026-10-11 contended pass vs the 2026-10-12 clean re-run
# Direction: analysis only; consumes report JSONs, emits a TSV plus a markdown table
# Status: repository-side publication tooling, NOT a product capability. Cross-report comparison
#         is registered as ROADMAP M2-T11 (CLI `benchmark compare` + schema); this script is a
#         stopgap for the 2026-10-11 article and is marked superseded once that command lands.
#         Per-capability statistics (median/min/max/IQR/speedup) are already product-side in the
#         report itself (M2-T1); only cross-window/condition comparison lives here.
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
"""Compare two sets of benchmark reports, condition A against condition B.

Every row is the same capability × backend measured twice; the question is not
"which is bigger" but "did the difference exceed what run-to-run dispersion can
explain". The rule mirrors the window protocol: a shift counts as resolved only
when |Δ| exceeds 2×(CV_A + CV_B); otherwise it is reported as within-noise, and
the tool says so explicitly instead of implying a change.

Nothing is re-derived: medians and CVs come from each report's own runs, so the
output can be checked against the archived reports by hand.

Usage:
    python scripts/benchmark/analysis/compare-conditions.py \
        --a benchmark-results/2026-10-11/contended-window --a-label "contended (CI co-tenant)" \
        --b /path/to/clean-reports --b-label "clean (CI idle)" \
        --out benchmark-results/2026-10-11/condition-comparison.tsv
"""

from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path

NOISE_FACTOR = 2.0  # |Δ| counts as resolved beyond this multiple of the CV sum


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def median_of(values: list[float]) -> float:
    ordered = sorted(values)
    count = len(ordered)
    return ordered[count // 2] if count % 2 else statistics.fmean(ordered[count // 2 - 1:count // 2 + 1])


def cv_percent(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    mean = statistics.fmean(values)
    return statistics.stdev(values) / mean * 100 if mean else 0.0


def report_key(path: Path, report: dict) -> str:
    """Capability id plus any experiment marker carried in the filename.

    `sequence.stats.v1.granularity-one.benchmark.json` → `sequence.stats.v1#granularity-one`,
    so the three record shapes stay distinct instead of colliding on the id.
    """
    capability = report["capability"]
    marker = path.stem.removeprefix(capability).removeprefix(".")
    if marker == "benchmark":
        marker = ""
    elif marker.endswith(".benchmark"):
        marker = marker[: -len(".benchmark")]
    return f"{capability}#{marker}" if marker else capability


def summarise(directory: Path) -> dict[str, dict[str, dict]]:
    """key → backend → {median, cv, n}."""
    result: dict[str, dict[str, dict]] = {}
    for path in sorted(directory.glob("*.benchmark.json")):
        report = load(path)
        entry = result.setdefault(report_key(path, report), {})
        for backend in report.get("backends", []):
            runs = [run["wall_ms"] for run in backend.get("runs", []) if run.get("ok")]
            if not runs:
                continue
            entry[backend["backend"]] = {
                "median": median_of(runs),
                "cv": cv_percent(runs),
                "n": len(runs),
            }
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--a", required=True, type=Path, help="report directory for condition A")
    parser.add_argument("--b", required=True, type=Path, help="report directory for condition B")
    parser.add_argument("--a-label", default="A")
    parser.add_argument("--b-label", default="B")
    parser.add_argument("--out", required=True, type=Path, help="TSV path; a .md twin is written beside it")
    arguments = parser.parse_args()

    a = summarise(arguments.a)
    b = summarise(arguments.b)
    rows = []
    for key in sorted(set(a) | set(b)):
        for backend in ("rust", "python", "r"):
            left, right = a.get(key, {}).get(backend), b.get(key, {}).get(backend)
            if not left or not right:
                continue
            delta = (right["median"] - left["median"]) / left["median"] * 100
            band = NOISE_FACTOR * (left["cv"] + right["cv"])
            verdict = "resolved" if abs(delta) > band else "within-noise"
            rows.append({
                "key": key,
                "backend": backend,
                "median_a_ms": f"{left['median']:.3f}",
                "median_b_ms": f"{right['median']:.3f}",
                "delta_percent": f"{delta:+.2f}",
                "noise_band_percent": f"{band:.2f}",
                "verdict": verdict,
                "n_a": str(left["n"]),
                "n_b": str(right["n"]),
            })

    columns = ["key", "backend", "median_a_ms", "median_b_ms", "delta_percent",
               "noise_band_percent", "verdict", "n_a", "n_b"]
    arguments.out.parent.mkdir(parents=True, exist_ok=True)
    header = f"# {arguments.a_label} (A) vs {arguments.b_label} (B)\n"
    with arguments.out.open("w", encoding="utf-8", newline="") as handle:
        handle.write("\t".join(columns) + "\n")
        for row in rows:
            handle.write("\t".join(row[column] for column in columns) + "\n")

    resolved = [row for row in rows if row["verdict"] == "resolved"]
    markdown = [header,
                f"- rows compared: {len(rows)}; shifted beyond noise: {len(resolved)}",
                "",
                "| capability | backend | A median ms | B median ms | Δ | noise band | verdict |",
                "| --- | --- | --- | --- | --- | --- | --- |"]
    for row in rows:
        markdown.append("| `{key}` | {backend} | {median_a_ms} | {median_b_ms} | {delta_percent}% | "
                        "±{noise_band_percent}% | {verdict} |".format(**row))
    arguments.out.with_suffix(".md").write_text("\n".join(markdown) + "\n", encoding="utf-8")

    print(f"wrote {arguments.out} and {arguments.out.with_suffix('.md')}")
    print(f"rows={len(rows)} resolved_beyond_noise={len(resolved)} "
          f"within_noise={len(rows) - len(resolved)}")
    for row in resolved:
        print(f"  SHIFT {row['key']} {row['backend']}: {row['delta_percent']}% "
              f"(band ±{row['noise_band_percent']}%)")


if __name__ == "__main__":
    main()
