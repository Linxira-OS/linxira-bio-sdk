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
regenerated from it, and this script is what proves the two agree. Three kinds of
check run here:

1. **labels** — every printed number is re-derived from the archive (medians,
   memory, ratios, intervals, verdict words);
2. **geometry** — bar ends and dot positions are converted back through the same
   linear scale the renderer used, so a bar cannot silently disagree with its own
   label (stability dots are compared against the raw per-run wall times);
3. **presentation promises** — each figure must carry the "no smoothing, no axis
   truncation, no statistical transformation" disclosure, and every caption line
   must fit inside the canvas (a width-unaware wrapper clipped Chinese captions).

Usage:
    python scripts/benchmark/figures/verify-figures.py \
        --figures benchmark-results/2026-10-11/blog-mirror/figures/zh \
        --summary benchmark-results/2026-10-11/summary.json \
        --reports benchmark-results/2026-10-11 \
        [--comparison benchmark-results/2026-09-13/summary.json] \
        [--granularity /path/to/granularity-reports]
"""

from __future__ import annotations

import argparse
import csv
import html
import json
import math
import re
import statistics
import sys
import unicodedata
from pathlib import Path

TOLERANCE = 0.02  # 2% relative — the renderer positions at ~1e-3 px precision

# Geometry shared with render-svgs.py; keep in sync (the checker fails loudly if
# a bar cannot be matched positionally, which is what a mismatch looks like).
MEDIANS_RIGHT = 760.0
MEMORY_RIGHT = 780.0
STABILITY_RIGHT = 800.0
BARS_LEFT = 300.0
ROW_MAX_HEADROOM = 1.08
MATERIAL_RATIO = 1.10
LADDER_LEFT = 330.0       # fig-speedup-ladder.svg / fig-memory-ratio.svg
LADDER_RIGHT = 800.0
LADDER_HEADROOM = 1.05
LADDER_SPLIT = 10.0      # the two-panel split for the ratio ladder
DISPERSION_LEFT = 300.0   # fig-dispersion.svg
DISPERSION_RIGHT = 780.0
DISPERSION_HEADROOM = 1.12
CONDITION_LEFT = 340.0    # fig-condition.svg
CONDITION_RIGHT = 820.0

# Every data bar is a plain `<rect>`; decoration carries class="deco" so the
# geometry readers can skip panels, row bands and bar lanes explicitly.
# The lookahead cannot cross ">", so it only ever inspects the current tag.
DECO = r'(?![^>]*class="deco")'


def bar_pattern(height: str) -> str:
    return (r'<rect ' + DECO + r'x="([\d.]+)" y="[\d.]+" width="([\d.]+)" '
            r'height="' + height + r'" rx="[\d.]+" fill="(#[0-9a-fA-F]{6})"')


def coefficient_of_variation(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    mean = statistics.fmean(values)
    return statistics.stdev(values) / mean if mean else 0.0


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


def ratio_ci(first: list[float], second: list[float], rounds: int = 10_000,
             seed: int = 20261011) -> tuple[float, float, float]:
    generator = __import__("random").Random(seed)
    ratios = []
    for _ in range(rounds):
        a = median_of(generator.choices(first, k=len(first)))
        b = median_of(generator.choices(second, k=len(second)))
        ratios.append(b / a if a else float("inf"))
    ratios.sort()
    return (median_of(second) / median_of(first), ratios[int(0.025 * rounds)], ratios[int(0.975 * rounds)])


def unique_table(rows: list[dict], column: str) -> tuple[dict, set]:
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


def linear(value: float, low: float, high: float, left: float, right: float) -> float:
    if high <= low:
        return left
    return left + (value - low) / (high - low) * (right - left)


def recover(x: float, low: float, high: float, left: float, right: float) -> float:
    return low + (x - left) / (right - left) * (high - low)


def check_value_labels(report: Report, text: str, pattern: str, expected: list[float]) -> None:
    got = sorted(float(match.replace(",", "")) for match in re.findall(pattern, text))
    want = sorted(expected)
    report.check(len(got) == len(want), f"{len(got)} labels found, {len(want)} expected")
    for found, predicted in zip(got, want):
        report.check(close(found, predicted), f"label {found:g} != table {predicted:g}")


def check_row_bars(report: Report, text: str, rows: list[tuple[str, dict[str, float]]],
                   right: float, unit: str) -> None:
    """Medians figure: every row is its own truncated linear scale (0..max*1.08)."""
    parsed = re.findall(bar_pattern("14.00"), text)
    expected_sequence = [(capability, backend, values[backend])
                         for capability, values in rows for backend in ("rust", "python", "r")
                         if values.get(backend)]
    report.check(len(parsed) == len(expected_sequence),
                 f"{len(parsed)} bars parsed, expected {len(expected_sequence)}")
    colours = {"#f59e0b": "rust", "#60a5fa": "python", "#34d399": "r"}
    for (x, width, colour), (capability, backend, value) in zip(parsed, expected_sequence):
        report.check(colours.get(colour.lower()) == backend, f"bar drawn for {capability} in {colour}")
        row_max = max(v for v in rows[[c for c, _v in rows].index(capability)][1].values()) * ROW_MAX_HEADROOM
        predicted = recover(float(x) + float(width), 0.0, row_max, BARS_LEFT, right)
        report.check(close(predicted, value),
                     f"{capability}/{backend} bar implies {predicted:g} {unit}, table says {value:g}")


def check_global_bars(report: Report, text: str, table: dict, order: list[tuple[str, str]],
                      right: float, headroom: float = ROW_MAX_HEADROOM) -> None:
    """Memory figure: one shared truncated linear scale (0..max*headroom)."""
    parsed = re.findall(bar_pattern("14.00"), text)
    drawable = [(capability, backend) for capability, backend in order if (capability, backend) in table]
    report.check(len(parsed) == len(drawable), f"{len(parsed)} bars parsed, expected {len(drawable)}")
    high = max(table.values()) * headroom
    colours = {"#f59e0b": "rust", "#60a5fa": "python", "#34d399": "r"}
    for (x, width, colour), (capability, backend) in zip(parsed, drawable):
        report.check(colours.get(colour.lower()) == backend, f"bar drawn for {capability}/{backend} in {colour}")
        predicted = recover(float(x) + float(width), 0.0, high, BARS_LEFT, right)
        expected = table[(capability, backend)]
        report.check(close(predicted, expected),
                     f"{capability}/{backend} bar implies {predicted:g} MB, table says {expected:g}")


def check_stability_dots(report: Report, text: str, rows: list[tuple[str, dict[str, list[float]]]],
                         right: float) -> None:
    """Every dot is one timed repeat: recover each raw wall_ms from its position.

    This is the strictest geometry check in the set — 10 dots per backend per
    capability, compared against the runs recorded in the reports themselves.
    """
    parsed = re.findall(r'<circle cx="([\d.]+)" cy="[\d.]+" r="5\.00" fill="(#[0-9a-fA-F]{6})"',
                        text)
    colours = {"#f59e0b": "rust", "#60a5fa": "python", "#34d399": "r"}
    expected_sequence = []
    for capability, times in rows:
        # The renderer scales every row so that all plotted dots fit, i.e. by the
        # largest raw run in that row; the checker must use the same reference.
        row_max = max(v for values in times.values() for v in values) * ROW_MAX_HEADROOM
        for backend in ("rust", "python", "r"):
            for value in times.get(backend, []):
                expected_sequence.append((capability, backend, value, row_max))
    report.check(len(parsed) == len(expected_sequence),
                 f"{len(parsed)} dots parsed, expected {len(expected_sequence)}")
    for (x, colour), (capability, backend, value, row_max) in zip(parsed, expected_sequence):
        report.check(colours.get(colour.lower()) == backend, f"dot drawn for {capability}/{backend} in {colour}")
        predicted = recover(float(x), 0.0, row_max, BARS_LEFT, right)
        report.check(close(predicted, value),
                     f"{capability}/{backend} dot implies {predicted:g} ms, report says {value:g}")


def check_shared_bars(report: Report, text: str,
                      panels: list[tuple[list[tuple[str, str, float]], float, float]],
                      *, left: float, right: float, unit: str) -> None:
    """Bars that share one left edge, read against one scale per panel.

    fig-memory-ratio.svg passes a single panel (multipliers, 1.0 .. max);
    fig-speedup-ladder.svg passes two (the 1-10x panel that carries the decision
    line, then the panel that starts at 10x). Bars are parsed once, in document
    order, and sliced per panel.
    """
    parsed = re.findall(bar_pattern("16.00"), text)
    flat = [row for panel, _low, _high in panels for row in panel]
    report.check(len(parsed) == len(flat), f"{len(parsed)} bars parsed, expected {len(flat)}")
    colours = {"#f59e0b": "rust", "#60a5fa": "python", "#34d399": "r"}
    index = 0
    for panel, low, high in panels:
        for (x, width, colour), (capability, backend, value) in zip(parsed[index:index + len(panel)], panel):
            report.check(colours.get(colour.lower()) == backend,
                         f"bar drawn for {capability}/{backend} in {colour}")
            report.check(close(float(x), left), f"{capability}/{backend} bar does not start at the axis")
            predicted = recover(float(x) + float(width), low, high, left, right)
            report.check(close(predicted, value),
                         f"{capability}/{backend} bar implies {predicted:g}{unit}, table says {value:g}")
        index += len(panel)


def check_dispersion_bars(report: Report, text: str, expected: list[tuple[str, str, float]],
                          high: float) -> None:
    """fig-dispersion.svg: one shared 0..max linear scale, values are CV in percent."""
    parsed = re.findall(bar_pattern("14.00"), text)
    report.check(len(parsed) == len(expected),
                 f"{len(parsed)} bars parsed, expected {len(expected)}")
    colours = {"#f59e0b": "rust", "#60a5fa": "python", "#34d399": "r"}
    for (x, width, colour), (capability, backend, value) in zip(parsed, expected):
        report.check(colours.get(colour.lower()) == backend, f"bar drawn for {capability}/{backend} in {colour}")
        predicted = recover(float(x) + float(width), 0.0, high, DISPERSION_LEFT, DISPERSION_RIGHT)
        report.check(close(predicted, value),
                     f"{capability}/{backend} CV bar implies {predicted:g}%, reports say {value:g}")


def check_condition(report: Report, text: str, expected: list[tuple[str, str, float, float]]) -> None:
    """fig-condition.svg: zero-centred bars whose value is |Δ| ÷ that row's own band.

    The bar is read by its centre (signed ratio); the two ACCENT dashed lines are
    read back as the ±1.0 decision boundary. A true zero shift is drawn at the
    1.5 px minimum width, which the checker accepts as the drawing floor.
    """
    parsed = re.findall(bar_pattern("14.00"), text)
    report.check(len(parsed) == len(expected), f"{len(parsed)} bars parsed, expected {len(expected)}")
    ratios = [abs(delta) / band if band else 0.0 for _c, _b, delta, band in expected]
    span = max(max(ratios), 1.0) * 1.12
    half = (CONDITION_RIGHT - CONDITION_LEFT) / 2
    floor = 1.5 / half * span          # 1.5 px, expressed in axis units
    zero = (CONDITION_LEFT + CONDITION_RIGHT) / 2
    for (x, width, _colour), (capability, backend, delta, band) in zip(parsed, expected):
        want = abs(delta) / band if band else 0.0
        # Bars start at zero, so the value lives at the edge farther from zero.
        left_edge, right_edge = float(x), float(x) + float(width)
        outer = left_edge if abs(left_edge - zero) > abs(right_edge - zero) else right_edge
        predicted = recover(outer, -span, span, CONDITION_LEFT, CONDITION_RIGHT)
        if close(float(width), 1.5):
            # The drawn floor is a 1.5 px minimum; the check allows the same 2%
            # rounding room the rest of the geometry checks use.
            limit = floor * (1 + TOLERANCE)
            report.check(abs(predicted) <= limit and abs(want) <= limit,
                         f"{capability}/{backend} bar is at the drawing floor but the shift is "
                         f"{want:g} (={predicted:+g} read back)")
        else:
            report.check(close(abs(predicted), want),
                         f"{capability}/{backend} bar implies |Δ|/band={abs(predicted):g}, table says {want:g}")
            report.check(predicted * delta >= 0,
                         f"{capability}/{backend} bar points the wrong way ({predicted:+g} for Δ={delta:+g}%)")
    boundary = re.findall(r'<line x1="([\d.]+)" y1="[\d.]+" x2="[\d.]+" y2="[\d.]+" '
                          r'stroke="#20B8B0" stroke-width="1\.4"', text)
    report.check(len(boundary) == 2, f"{len(boundary)} decision lines parsed, expected 2")
    for x_one in boundary:
        predicted = recover(float(x_one), -span, span, CONDITION_LEFT, CONDITION_RIGHT)
        report.check(close(abs(predicted), 1.0), f"decision line implies {predicted:+g}, expected ±1.0")


def load_report(path: object) -> dict:
    target = Path(path)
    if not target.is_file():
        return {}
    return json.loads(target.read_text(encoding="utf-8"))


def require_arm_class(report: dict, path: object, expected: str) -> None:
    """Refuse to plot a report whose arm class is not the one this figure set declares."""
    arm_class = report.get("arm_class", "parity")
    if arm_class != expected:
        raise SystemExit(
            f"{path} is arm_class={arm_class!r} but this figure set declares "
            f"--expected-class {expected}: mixing arm classes in one figure is forbidden "
            "(ROADMAP 7.1)"
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--figures", required=True, type=Path)
    parser.add_argument("--summary", required=True, type=Path)
    parser.add_argument(
        "--expected-class",
        choices=["parity", "tool"],
        default="parity",
        help="arm class these figures are allowed to plot; a report of the other class "
             "would put two arm classes in one figure (ROADMAP 7.1), so it is an error",
    )
    parser.add_argument("--reports", required=True, type=Path)
    parser.add_argument("--stats", type=Path, help="statistics TSV; defaults to the archive directory")
    parser.add_argument("--comparison", type=Path, help="earlier summary.json for the two-window figure")
    parser.add_argument("--granularity", type=Path, help="granularity experiment reports")
    parser.add_argument("--contended", type=Path, help="the contended batch's reports (condition figure)")
    arguments = parser.parse_args()

    summary = json.loads(arguments.summary.read_text(encoding="utf-8"))
    stats_path = arguments.stats or (arguments.summary.parent / "statistics.tsv")
    rows = list(csv.DictReader(stats_path.open(encoding="utf-8"), delimiter="\t"))
    wall, duplicate_keys = unique_table(rows, "median_wall_ms")
    rss, _ = unique_table(rows, "median_peak_rss_mb")
    report = Report("values")
    if duplicate_keys:
        report.check(False, "the statistics table repeats a capability/backend key, so figures and "
                            f"checks cannot be matched one-to-one: {sorted(duplicate_keys)}")
    entries = [entry for entry in summary["entries"] if entry["backends"].get("rust")]
    figures = arguments.figures

    # ---- labels ----------------------------------------------------------
    check_value_labels(report, figures_text(figures, "fig-absolute-medians.svg"),
                       r">([\d,]+) ms<", [wall[(e["capability"], b)]
                                          for e in entries for b in ("rust", "python", "r")
                                          if (e["capability"], b) in wall])
    check_value_labels(report, figures_text(figures, "fig-memory.svg"),
                       r">([\d,.]+) MB<", [rss[(e["capability"], b)]
                                           for e in entries for b in ("rust", "python", "r")
                                           if (e["capability"], b) in rss])

    # ---- geometry --------------------------------------------------------
    median_rows = [(e["capability"],
                    {b: (e["backends"].get(b) or {}).get("median_wall_ms")
                     for b in ("rust", "python", "r")
                     if (e["backends"].get(b) or {}).get("median_wall_ms")})
                   for e in entries]
    check_row_bars(report, figures_text(figures, "fig-absolute-medians.svg"), median_rows,
                   MEDIANS_RIGHT, "ms")

    memory_rows = [e["capability"] for e in entries]
    check_global_bars(report, figures_text(figures, "fig-memory.svg"), rss,
                      [(capability, backend) for capability in memory_rows
                       for backend in ("rust", "python", "r")], MEMORY_RIGHT)

    stability_rows = []
    for entry in entries:
        report_file = arguments.reports / entry["report"]
        require_arm_class(load_report(report_file), report_file, arguments.expected_class)
        if report_file.is_file():
            stability_rows.append((entry["capability"],
                                   runs_by_backend(json.loads(report_file.read_text(encoding="utf-8")))))
    check_stability_dots(report, figures_text(figures, "fig-repeat-stability.svg"),
                         stability_rows, STABILITY_RIGHT)

    # ---- new views (2026-10-12): ladder, memory ratio, dispersion, condition ----
    ladder_expected: list[tuple[str, str, float]] = []
    for entry in entries:
        capability = entry["capability"]
        base = (entry["backends"].get("rust") or {}).get("median_wall_ms")
        if not base:
            continue
        for backend in ("python", "r"):
            value = (entry["backends"].get(backend) or {}).get("median_wall_ms")
            if value:
                ladder_expected.append((capability, backend, value / base))
    ladder_expected.sort(key=lambda row: row[2], reverse=True)
    ladder_upper = [row for row in ladder_expected if row[2] >= LADDER_SPLIT]
    ladder_lower = [row for row in ladder_expected if row[2] < LADDER_SPLIT]
    check_shared_bars(report, figures_text(figures, "fig-speedup-ladder.svg"),
                      [(ladder_upper, LADDER_SPLIT, max(row[2] for row in ladder_upper) * LADDER_HEADROOM),
                       (ladder_lower, 1.0, LADDER_SPLIT)],
                      left=LADDER_LEFT, right=LADDER_RIGHT, unit="×")
    ladder_text = figures_text(figures, "fig-speedup-ladder.svg")
    for capability, backend, ratio in ladder_expected:
        verdict = "material" if ratio >= MATERIAL_RATIO else "below line"
        report.check(f">{ratio:.2f}× · {verdict}<" in ladder_text,
                     f"ladder label {ratio:.2f}× · {verdict} missing ({capability}/{backend})")

    memory_ratio: list[tuple[str, str, float]] = []
    for entry in entries:
        capability = entry["capability"]
        base = (entry["backends"].get("rust") or {}).get("median_peak_rss_mb")
        if not base:
            continue
        for backend in ("python", "r"):
            value = (entry["backends"].get(backend) or {}).get("median_peak_rss_mb")
            if value:
                memory_ratio.append((capability, backend, value / base))
    memory_ratio.sort(key=lambda row: row[2], reverse=True)
    memory_high = max(row[2] for row in memory_ratio) * LADDER_HEADROOM
    check_shared_bars(report, figures_text(figures, "fig-memory-ratio.svg"),
                      [(memory_ratio, 1.0, memory_high)],
                      left=LADDER_LEFT, right=LADDER_RIGHT, unit="×")
    ratio_text = figures_text(figures, "fig-memory-ratio.svg")
    for capability, backend, ratio in memory_ratio:
        report.check(f">{ratio:.1f}×<" in ratio_text,
                     f"memory-ratio label {ratio:.1f}× missing ({capability}/{backend})")

    dispersion, dispersion_values = [], []
    for entry in entries:
        report_file = arguments.reports / entry["report"]
        require_arm_class(load_report(report_file), report_file, arguments.expected_class)
        if not report_file.is_file():
            continue
        times = runs_by_backend(json.loads(report_file.read_text(encoding="utf-8")))
        for backend in ("rust", "python", "r"):
            if times.get(backend):
                value = coefficient_of_variation(times[backend]) * 100
                dispersion.append((entry["capability"], backend, value))
                dispersion_values.append(value)
    dispersion_high = max(dispersion_values) * DISPERSION_HEADROOM
    check_dispersion_bars(report, figures_text(figures, "fig-dispersion.svg"), dispersion, dispersion_high)
    dispersion_text = figures_text(figures, "fig-dispersion.svg")
    for capability, backend, value in dispersion:
        report.check(f">{value:.2f}%<" in dispersion_text,
                     f"dispersion label {value:.2f}% missing ({capability}/{backend})")

    if arguments.contended:
        condition = []
        for other_file in sorted(arguments.contended.glob("*.benchmark.json")):
            clean_file = arguments.reports / other_file.name
            if not clean_file.is_file():
                continue
            contended_runs = runs_by_backend(json.loads(other_file.read_text(encoding="utf-8")))
            clean_runs = runs_by_backend(json.loads(clean_file.read_text(encoding="utf-8")))
            for backend in ("rust", "python", "r"):
                if not contended_runs.get(backend) or not clean_runs.get(backend):
                    continue
                first, second = median_of(contended_runs[backend]), median_of(clean_runs[backend])
                delta = (second - first) / first * 100 if first else 0.0
                band = 2 * (coefficient_of_variation(contended_runs[backend])
                            + coefficient_of_variation(clean_runs[backend])) * 100
                if band == 0 and abs(delta) > 0:
                    report.check(False, f"{other_file.name}/{backend} has a zero band with a "
                                        f"{delta:+.2f}% shift; the figure must not be drawn")
                    continue
                condition.append((other_file.name, backend, delta, band))
        condition.sort(key=lambda row: (abs(row[2]) / row[3] if row[3] else 0.0), reverse=True)
        check_condition(report, figures_text(figures, "fig-condition.svg"), condition)
        condition_text = figures_text(figures, "fig-condition.svg")
        for key, backend, delta, band in condition:
            verdict = "beyond band" if (band > 0 and abs(delta) > band) else "inside band"
            zh_verdict = "超出带宽" if verdict == "beyond band" else "落在带宽内"
            report.check(any(f">{delta:+.2f}% · {word} ±{band:.2f}% · {local}<" in condition_text
                             for word, local in (("band", verdict), ("带", zh_verdict))),
                         f"condition label {delta:+.2f}% / ±{band:.2f}% / {verdict} missing ({key}/{backend})")

    # ---- presentation promises ------------------------------------------
    for name in sorted(p.name for p in figures.glob("*.svg")):
        svg = figures_text(figures, name)
        compact = re.sub(r"\s+", "", re.sub(r"<[^>]+>", "", svg))
        report.check("nosmoothing" in compact or "无平滑" in compact,
                     f"{name} is missing the raw-data disclosure line")
        canvas_height = int(re.search(r'viewBox="0 0 \d+ (\d+)"', svg).group(1))
        for y, content in re.findall(r'<text x="24" y="([\d.]+)"[^>]*>(.*?)</text>', svg):
            if float(y) <= canvas_height - 140:
                continue
            # `&lt;` renders as one glyph, so entity-escaped text must be unescaped
            # before its width is measured, or the check reports phantom columns.
            line = html.unescape(re.sub(r"<[^>]+>", "", content))
            columns = sum(2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1 for ch in line)
            report.check(columns <= 152, f"{name} caption line is {columns} columns wide (max 152)")

    # ratio CI: annotation text carries ratio, interval and verdict
    ci_text = figures_text(figures, "fig-ratio-ci.svg")
    annotations = re.findall(r">([\d.]+)× \[([\d.]+), ([\d.]+)\] ([a-z-]+)<", ci_text)
    report.check(len(annotations) == 2 * len(entries),
                 f"{len(annotations)} CI annotations, expected {2 * len(entries)}")
    for capability in [e["capability"] for e in entries]:
        for backend in ("python", "r"):
            row = next((r for r in rows if r["capability"] == capability and r["backend"] == backend), None)
            if row is None:
                continue
            match = [a for a in annotations if close(float(a[0]), float(row["ratio_vs_rust"]))]
            report.check(bool(match), f"no annotation for {capability}/{backend} ratio {row['ratio_vs_rust']}")
            if match:
                _, low, high, verdict = match[0]
                report.check(close(float(low), float(row["ratio_ci_low"])),
                             f"{capability}/{backend} CI low {low} != table {row['ratio_ci_low']}")
                report.check(close(float(high), float(row["ratio_ci_high"])),
                             f"{capability}/{backend} CI high {high} != table {row['ratio_ci_high']}")
                report.check(verdict == row["verdict"],
                             f"{capability}/{backend} verdict {verdict} != table {row['verdict']}")

    # layered speedup + series: ratio labels must exist for every plotted pair
    speed_text = figures_text(figures, "fig-speedup-layered.svg")
    for capability in [e["capability"] for e in entries]:
        for backend in ("python", "r"):
            expected, baseline = wall.get((capability, backend)), wall.get((capability, "rust"))
            if expected is None or not baseline:
                continue
            wanted = f"{(expected / baseline):.2f}×"
            report.check(f">{wanted}<" in speed_text, f"{capability}/{backend} label {wanted} missing")

    if arguments.comparison:
        earlier = json.loads(arguments.comparison.read_text(encoding="utf-8"))
        series_text = figures_text(figures, "fig-series-two-windows.svg")
        for entry in earlier["entries"]:
            current = next((e for e in entries if e["capability"] == entry["capability"]), None)
            if current is None:
                continue
            for backend in ("python", "r"):
                old, new = entry["backends"].get(backend), current["backends"].get(backend)
                old_base, new_base = entry["backends"].get("rust"), current["backends"].get("rust")
                if not all((old, new, old_base, new_base)):
                    continue
                small = f"{(old['median_wall_ms'] / old_base['median_wall_ms']):.2f}×"
                large = f"{(new['median_wall_ms'] / new_base['median_wall_ms']):.2f}×"
                report.check(any(f">{token} {small}<" in series_text for token in ("small", "小夹具")),
                             f"series label {small} missing ({entry['capability']})")
                report.check(any(f">{token} {large}<" in series_text for token in ("large", "大输入")),
                             f"series label {large} missing ({entry['capability']})")

    # granularity: bar-top labels are the medians of the three record shapes
    if arguments.granularity:
        gran_text = figures_text(figures, "fig-granularity.svg")
        for key, median in granularity_medians(arguments.granularity).items():
            report.check(f">{median:,.0f}<" in gran_text, f"granularity label {median:,.0f} missing ({key})")

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
