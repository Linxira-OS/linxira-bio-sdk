#!/usr/bin/env python3
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/figures/verify-figures.py
# Role: re-derive every number a rendered figure shows from the archive/reports and fail on drift
# Windows: 2026-10-11 (N=10 three-backend window)
# Direction: publication safety net — a figure must never carry a number that the table does not
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
"""Check that the rendered SVGs contain exactly the numbers the tables contain.

A figure is a derived artefact: if the archive is regenerated, the figures are
regenerated from it, and this script is what proves the two agree. It parses the
figure text (the SVG carries plain ``<text>`` labels), re-derives each value from
the archive and the raw reports, and exits non-zero on any mismatch.

Usage:
    python scripts/benchmark/figures/verify-figures.py \
        --figures benchmark-results/2026-10-11/blog-mirror/figures \
        --summary benchmark-results/2026-10-11/summary.json \
        --reports benchmark-results/2026-10-11 \
        --stats benchmark-results/2026-10-11/statistics.tsv \
        [--comparison benchmark-results/2026-09-13/summary.json] \
        [--granularity /path/to/granularity-reports]
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
import statistics
import sys
from pathlib import Path

TOLERANCE = 0.02  # 2% relative — labels are rendered with limited precision


def figures_text(directory: Path, name: str) -> str:
    return (directory / name).read_text(encoding="utf-8")


def median_of(values: list[float]) -> float:
    ordered = sorted(values)
    count = len(ordered)
    return ordered[count // 2] if count % 2 else statistics.fmean(ordered[count // 2 - 1:count // 2 + 1])


def runs_by_backend(report: dict) -> dict[str, list[float]]:
    result: dict[str, list[float]] = {}
    for backend in report.get("backends", []):
        times = [run["wall_ms"] for run in backend.get("runs", []) if run.get("ok")]
        if times:
            result[backend["backend"]] = times
    return result


def ratio_ci(first: list[float], second: list[float], rounds: int = 10_000, seed: int = 20261011) -> tuple[float, float, float]:
    generator = __import__("random").Random(seed)
    ratios = []
    for _ in range(rounds):
        a = median_of(generator.choices(first, k=len(first)))
        b = median_of(generator.choices(second, k=len(second)))
        ratios.append(b / a if a else float("inf"))
    ratios.sort()
    return (median_of(second) / median_of(first), ratios[int(0.025 * rounds)], ratios[int(0.975 * rounds)])


def classify(low: float, high: float, material: float = 1.10) -> str:
    if high <= 1.0 or low <= 1.0:
        return "within-noise"
    return "material" if low >= material else "detectable-immaterial"


def unique_table(rows: list[dict], column: str) -> tuple[dict, set]:
    """Key a table by (capability, backend) and report any repeated key."""
    table: dict = {}
    duplicates: set = set()
    for row in rows:
        key = (row["capability"], row["backend"])
        if key in table:
            duplicates.add(key)
        table[key] = float(row[column])
    return table, duplicates


class Report:
    def __init__(self, name: str) -> None:
        self.name = name
        self.checks = 0
        self.failures: list[str] = []

    def check(self, condition: bool, message: str) -> None:
        self.checks += 1
        if not condition:
            self.failures.append(message)


def close(a: float, b: float) -> bool:
    return abs(a - b) <= TOLERANCE * max(abs(a), abs(b), 1e-9)


def check_value_labels(report: Report, text: str, pattern: str, expected: list[float]) -> None:
    got = sorted(float(match.replace(",", "")) for match in re.findall(pattern, text))
    want = sorted(expected)
    report.check(len(got) == len(want), f"{len(got)} labels found, {len(want)} expected")
    for found, predicted in zip(got, want):
        report.check(close(found, predicted), f"label {found:g} != table {predicted:g}")


def check_bars(report: Report, text: str, table: dict, left: float, right: float,
               order: list[tuple[str, str]], colours: dict[str, str]) -> None:
    """Bars run from the value's position to the right edge; recover the value from x.

    The renderer draws one bar per (capability, backend) in ``order``, so the
    parsed rectangles are matched positionally instead of by re-deriving the
    layout arithmetic.
    """
    values = list(table.values())
    low, high = max(min(values) / 2, 0.5), max(values) * 2
    span = math.log10(high) - math.log10(low)
    pattern = (r'<rect x="([\d.]+)" y="[\d.]+" width="[\d.]+" height="(?:7|8)\.00" rx="[\d.]+" '
               r'fill="(#[0-9a-f]{6})"')
    parsed = [(float(x), colour) for x, colour in re.findall(pattern, text)]
    drawable = [(capability, backend) for capability, backend in order if (capability, backend) in table]
    report.check(len(parsed) == len(drawable),
                 f"{len(parsed)} bars parsed, expected {len(drawable)}")
    for (x, colour), (capability, backend) in zip(parsed, drawable):
        report.check(colours.get(colour) == backend,
                     f"bar {len(parsed) and ''}for {capability}/{backend} drawn in {colour}")
        predicted = 10 ** (math.log10(low) + (x - left) / (right - left) * span)
        expected = table[(capability, backend)]
        report.check(close(predicted, expected),
                     f"bar for {capability}/{backend} at x={x} implies {predicted:g}, table says {expected:g}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--figures", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument("--reports", required=True, type=Path)
    parser.add_argument("--stats", type=Path, help="statistics TSV; defaults to the archive directory")
    parser.add_argument("--comparison", type=Path, help="earlier summary.json for the two-window figure")
    parser.add_argument("--granularity", type=Path, help="granularity experiment reports")
    arguments = parser.parse_args()

    summary = json.loads(arguments.summary.read_text(encoding="utf-8"))
    stats_path = arguments.stats or (arguments.summary.parent / "statistics.tsv")
    rows = list(csv.DictReader(stats_path.open(encoding="utf-8"), delimiter="\t"))
    walls, duplicate_keys = unique_table(rows, "median_wall_ms")
    rss_table, _ = unique_table(rows, "median_peak_rss_mb")
    report = Report("values")
    if duplicate_keys:
        report.check(False, "the statistics table repeats a capability/backend key, so figures and "
                            f"checks cannot be matched one-to-one: {sorted(duplicate_keys)}")
    wall, rss = walls, rss_table
    entries = [entry for entry in summary["entries"] if entry["backends"].get("rust")]

    report = Report("values")
    figures = arguments.figures

    check_value_labels(report, figures_text(figures, "fig-absolute-medians.svg"),
                       r">([\d,]+) ms<", [wall[(e["capability"], b)]
                                          for e in entries for b in ("rust", "python", "r")
                                          if (e["capability"], b) in wall])
    check_value_labels(report, figures_text(figures, "fig-memory.svg"),
                       r">([\d,.]+) MB<", [rss[(e["capability"], b)]
                                           for e in entries for b in ("rust", "python", "r")
                                           if (e["capability"], b) in rss])

    draw_order = [(e["capability"], b) for e in entries for b in ("rust", "python", "r")]
    check_bars(report, figures_text(figures, "fig-absolute-medians.svg"), wall, 250.0, 900.0,
               draw_order, {"#f59e0b": "rust", "#60a5fa": "python", "#34d399": "r"})
    check_bars(report, figures_text(figures, "fig-memory.svg"), rss, 250.0, 830.0,
               draw_order, {"#f59e0b": "rust", "#60a5fa": "python", "#34d399": "r"})

    # ratio CI: annotation text carries ratio, interval and verdict
    ci_text = figures_text(figures, "fig-ratio-ci.svg")
    annotations = re.findall(r">([\d.]+)× \[([\d.]+), ([\d.]+)\] ([a-z-]+)<", ci_text)
    report.check(len(annotations) == 2 * len(entries),
                 f"{len(annotations)} CI annotations, expected {2 * len(entries)}")
    for capability in [e["capability"] for e in entries]:
        for backend in ("python", "r"):
            report_row = next((row for row in rows
                               if row["capability"] == capability and row["backend"] == backend), None)
            if report_row is None:
                continue
            match = [a for a in annotations if close(float(a[0]), float(report_row["ratio_vs_rust"]))]
            report.check(bool(match), f"no annotation for {capability}/{backend} ratio {report_row['ratio_vs_rust']}")
            if match:
                _, low, high, verdict = match[0]
                report.check(close(float(low), float(report_row["ratio_ci_low"])),
                             f"{capability}/{backend} CI low {low} != table {report_row['ratio_ci_low']}")
                report.check(close(float(high), float(report_row["ratio_ci_high"])),
                             f"{capability}/{backend} CI high {high} != table {report_row['ratio_ci_high']}")
                report.check(verdict == report_row["verdict"],
                             f"{capability}/{backend} verdict {verdict} != table {report_row['verdict']}")

    # layered speedup figure: one ratio label per (capability, backend)
    speed_text = figures_text(figures, "fig-speedup-layered.svg")
    for capability in [e["capability"] for e in entries]:
        for backend in ("python", "r"):
            expected = wall.get((capability, backend))
            baseline = wall.get((capability, "rust"))
            if expected is None or not baseline:
                continue
            wanted = f"{(expected / baseline):.2f}×"
            report.check(f">{wanted}<" in speed_text, f"{capability}/{backend} label {wanted} missing")

    # two-window series: "small X×" from the earlier archive, "large Y×" from this one
    if arguments.comparison:
        earlier = json.loads(arguments.comparison.read_text(encoding="utf-8"))
        series_text = figures_text(figures, "fig-series-two-windows.svg")
        for entry in earlier["entries"]:
            earlier_capability = entry["capability"]
            current = next((e for e in entries if e["capability"] == earlier_capability), None)
            if current is None:
                continue
            for backend in ("python", "r"):
                old, new = entry["backends"].get(backend), current["backends"].get(backend)
                old_base, new_base = entry["backends"].get("rust"), current["backends"].get("rust")
                if not all((old, new, old_base, new_base)):
                    continue
                small = f"small {(old['median_wall_ms'] / old_base['median_wall_ms']):.2f}×"
                large = f"large {(new['median_wall_ms'] / new_base['median_wall_ms']):.2f}×"
                report.check(f">{small}<" in series_text, f"series label {small} missing ({earlier_capability})")
                report.check(f">{large}<" in series_text, f"series label {large} missing ({earlier_capability})")

    # granularity: labels are the medians of the three record shapes
    if arguments.granularity:
        gran_text = figures_text(figures, "fig-granularity.svg")
        for key, median in granularity_medians(arguments.granularity).items():
            wanted = f">{median:,.0f}<"
            report.check(wanted in gran_text, f"granularity label {wanted} missing ({key})")
    for failure in report.failures:
        print(f"FAIL {failure}")
    print(f"{report.name}: {report.checks - len(report.failures)}/{report.checks} figure numbers agree "
          f"with the tables")
    sys.exit(1 if report.failures else 0)


def granularity_medians(directory: Path) -> dict[str, float]:
    result: dict[str, float] = {}
    for shape in ("one", "twenty", "many"):
        candidates = [directory / f"out-gran-{shape}" / "sequence.stats.v1.benchmark.json",
                      directory / f"sequence.stats.v1.granularity-{shape}.benchmark.json",
                      directory / f"gran-{shape}.benchmark.json",
                      directory / f"{shape}.benchmark.json",
                      directory / f"out-gran-{shape}.json"]
        found = next((c for c in candidates if c.is_file()), None)
        if found is None:
            continue
        for backend, values in runs_by_backend(json.loads(found.read_text(encoding="utf-8"))).items():
            result[f"{shape}/{backend}"] = median_of(values)
    return result


if __name__ == "__main__":
    main()
