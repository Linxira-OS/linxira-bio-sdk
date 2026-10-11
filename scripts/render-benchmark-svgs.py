#!/usr/bin/env python3
"""Render self-contained SVG figures from archived benchmark reports.

The figures are generated from the archived JSON, never hand-drawn: every
number in a figure comes from the same files that the summary links to, so a
reader can regenerate them from the repository. Output is deterministic (fixed
resampling seed, sorted capability order) so re-running produces identical
bytes.

Usage:
    python3 scripts/render-benchmark-svgs.py \
        --summary benchmark-results/<date>/summary.json \
        --reports /path/to/reports-with-raw-runs \
        --comparison benchmark-results/2026-09-13/summary.json \
        --granularity /path/to/granularity-reports \
        --output benchmark-results/<date>/blog-mirror/figures

Every figure targets the dark site theme (slate card, light ink); the palette
is fixed here so the SVGs stay self-contained and theme-independent.
"""
from __future__ import annotations

import argparse
import json
import math
import random
import statistics
from pathlib import Path

# --- palette (self-contained; matches the site's dark blog theme) ----------
# Site template: card/pre background #0a0f1a, ink #e2e8f0, muted #94a3b8,
# accent teal #20B8B0, hairline rgba(51,65,85,0.3).
CARD = "#0a0f1a"
INK = "#e2e8f0"
MUTED = "#94a3b8"
GRID = "#223046"
RUST = "#f59e0b"
PYTHON = "#60a5fa"
R_LANG = "#34d399"
ACCENT = "#20B8B0"
WARN = "#fb7185"
FONT = "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,'Helvetica Neue',Arial,'Noto Sans SC',sans-serif"

BOOTSTRAP_SEED = 20261011
BOOTSTRAP_ROUNDS = 10_000
# Same pre-registered rule as scripts/benchmark-statistics.py: detectable when
# the ratio CI excludes 1.0, material only at/above this floor.
MATERIAL_RATIO = 1.10


def esc(text: str) -> str:
    return (
        str(text)
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
    )


class Canvas:
    """Minimal SVG writer: log-scaled bar and whisker figures."""

    def __init__(self, width: int, height: int, title: str, caption: str):
        self.width = width
        self.height = height
        self.parts = [
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
            f'width="{width}" height="{height}" role="img" '
            f'aria-label="{esc(title)}">',
            f'<rect width="{width}" height="{height}" fill="{CARD}"/>',
            f'<text x="24" y="34" fill="{INK}" font-family="{FONT}" '
            f'font-size="19" font-weight="600">{esc(title)}</text>',
        ]
        self.caption = caption

    def text(self, x: float, y: float, content: str, *, size: int = 13,
             fill: str = INK, anchor: str = "start", weight: str = "400") -> None:
        self.parts.append(
            f'<text x="{x:.1f}" y="{y:.1f}" fill="{fill}" font-family="{FONT}" '
            f'font-size="{size}" font-weight="{weight}" text-anchor="{anchor}">{esc(content)}</text>'
        )

    def rect(self, x: float, y: float, w: float, h: float, fill: str,
             *, opacity: float = 1.0, radius: float = 2.0) -> None:
        self.parts.append(
            f'<rect x="{x:.2f}" y="{y:.2f}" width="{max(w, 0):.2f}" height="{max(h, 0):.2f}" '
            f'rx="{radius}" fill="{fill}" opacity="{opacity}"/>'
        )

    def line(self, x1: float, y1: float, x2: float, y2: float, stroke: str,
             *, width: float = 1.0, dash: str | None = None, opacity: float = 1.0) -> None:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        self.parts.append(
            f'<line x1="{x1:.1f}" y1="{y1:.1f}" x2="{x2:.1f}" y2="{y2:.1f}" '
            f'stroke="{stroke}" stroke-width="{width}" opacity="{opacity}"{dash_attr}/>'
        )

    def save(self, path: Path) -> None:
        self.parts.append(
            f'<text x="24" y="{self.height - 14}" fill="{MUTED}" font-family="{FONT}" '
            f'font-size="11">{esc(self.caption)}</text>'
        )
        self.parts.append("</svg>")
        path.write_text("\n".join(self.parts) + "\n", encoding="utf-8")


def log_scale(value: float, low: float, high: float, start: float, end: float) -> float:
    if value <= 0:
        value = low
    span = math.log10(high) - math.log10(low)
    return start + (math.log10(value) - math.log10(low)) / span * (end - start)


def bootstrap_median_ci(values: list[float], rounds: int = BOOTSTRAP_ROUNDS,
                        seed: int = BOOTSTRAP_SEED) -> tuple[float, float]:
    generator = random.Random(seed)
    count = len(values)
    medians = []
    for _ in range(rounds):
        sample = sorted(generator.choices(values, k=count))
        medians.append(sample[count // 2] if count % 2 else statistics.fmean(sample[count // 2 - 1:count // 2 + 1]))
    medians.sort()
    return medians[int(0.025 * rounds)], medians[int(0.975 * rounds)]


def ratio_ci(first: list[float], second: list[float]) -> tuple[float, float, float]:
    """Median ratio plus the 2.5/97.5 percentiles of resampled medians."""
    generator = random.Random(BOOTSTRAP_SEED)
    def median_of(sample: list[float]) -> float:
        ordered = sorted(sample)
        count = len(ordered)
        return ordered[count // 2] if count % 2 else statistics.fmean(ordered[count // 2 - 1:count // 2 + 1])
    ratios = []
    for _ in range(BOOTSTRAP_ROUNDS):
        a = median_of(generator.choices(first, k=len(first)))
        b = median_of(generator.choices(second, k=len(second)))
        ratios.append(b / a if a else float("inf"))
    ratios.sort()
    return (median_of(second) / median_of(first), ratios[int(0.025 * BOOTSTRAP_ROUNDS)],
            ratios[int(0.975 * BOOTSTRAP_ROUNDS)])


def coefficient_of_variation(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    mean = statistics.fmean(values)
    return statistics.stdev(values) / mean if mean else 0.0


def runs_by_backend(report: dict) -> dict[str, list[float]]:
    result: dict[str, list[float]] = {}
    for backend in report.get("backends", []):
        times = [run["wall_ms"] for run in backend.get("runs", []) if run.get("ok")]
        if times:
            result[backend["backend"]] = times
    return result


def figure_speedup(summary: dict, path: Path) -> None:
    entries = summary["entries"]
    rows = []
    for entry in entries:
        backends = entry["backends"]
        baseline = (backends.get("rust") or {}).get("median_wall_ms")
        if not baseline:
            continue
        rows.append((
            entry["capability"].replace(".v1", ""),
            (backends.get("python") or {}).get("median_wall_ms", 0) / baseline,
            ((backends.get("r") or {}).get("median_wall_ms", 0) / baseline) if backends.get("r") else None,
        ))
    rows.sort(key=lambda row: row[1])
    row_height = 34
    height = 120 + row_height * len(rows) + 30
    canvas = Canvas(940, height, "Rust vs interpreter backends — wall-time ratio (log scale)",
                    f"Source: {summary['report_id']} · median of N repeats per backend, same input, same machine · "
                    "left of the divider a C-level library already covers the loop, right of it the interpreter iterates per record")
    left, right = 250, 860
    high = max([row[2] or row[1] for row in rows] or [2]) * 1.6
    low = 0.8
    for decade in range(0, int(math.log10(high)) + 1):
        value = 10 ** decade
        if value < low:
            continue
        x = log_scale(value, low, high, left, right)
        canvas.line(x, 62, x, 62 + row_height * len(rows), GRID)
        canvas.text(x, 56, f"{value}×", size=11, fill=MUTED, anchor="middle")
    canvas.line(left, 62, left, 62 + row_height * len(rows), MUTED, width=1.5)
    for index, (name, python_ratio, r_ratio) in enumerate(rows):
        y = 62 + index * row_height
        canvas.text(left - 12, y + 20, name, size=12, fill=INK, anchor="end")
        canvas.rect(left, y + 6, log_scale(python_ratio, low, high, left, right) - left, 11, PYTHON)
        canvas.text(log_scale(python_ratio, low, high, left, right) + 6, y + 16,
                    f"{python_ratio:.2f}×", size=11, fill=PYTHON)
        if r_ratio is not None:
            canvas.rect(left, y + 19, log_scale(r_ratio, low, high, left, right) - left, 11, R_LANG)
            canvas.text(log_scale(r_ratio, low, high, left, right) + 6, y + 29,
                        f"{r_ratio:.2f}×", size=11, fill=R_LANG)
    canvas.text(right - 4, 56, "python", size=12, fill=PYTHON, anchor="end", weight="600")
    canvas.text(right - 4, 56, "", size=12, fill=R_LANG, anchor="end")
    canvas.text(right, 78 + row_height * len(rows), "r", size=12, fill=R_LANG, anchor="end", weight="600")
    canvas.save(path)


def figure_medians(summary: dict, path: Path) -> None:
    entries = [e for e in summary["entries"] if e["backends"].get("rust")]
    bar_group = 46
    height = 150 + bar_group * len(entries) + 40
    canvas = Canvas(940, height, "Absolute wall time per backend (log scale, ms)",
                    f"Source: {summary['report_id']} · median of N repeats · warm page cache")
    left = 250
    values = []
    for entry in entries:
        for backend in ("rust", "python", "r"):
            value = (entry["backends"].get(backend) or {}).get("median_wall_ms")
            if value:
                values.append(value)
    if not values:
        canvas.save(path)
        return
    low = max(min(values) / 2, 0.5)
    high = max(values) * 2
    right = 900
    for decade in range(0, int(math.log10(high)) + 2):
        value = 10 ** decade
        if value < low or value > high:
            continue
        x = log_scale(value, low, high, left, right)
        canvas.line(x, 62, x, 62 + bar_group * len(entries), GRID)
        canvas.text(x, 56, f"{value:,.0f}", size=11, fill=MUTED, anchor="middle")
    for index, entry in enumerate(entries):
        y = 62 + index * bar_group
        canvas.text(left - 12, y + 26, entry["capability"].replace(".v1", ""), size=12, fill=INK, anchor="end")
        for offset, (backend, colour) in enumerate((("rust", RUST), ("python", PYTHON), ("r", R_LANG))):
            value = (entry["backends"].get(backend) or {}).get("median_wall_ms")
            if not value:
                continue
            x = log_scale(value, low, high, left, right)
            canvas.rect(x, y + offset * 10 + 4, max(right - x, 1), 8, colour)
            canvas.text(x + 6, y + offset * 10 + 12, f"{value:,.0f} ms", size=10, fill=colour)
    canvas.save(path)


def figure_memory(summary: dict, path: Path) -> None:
    entries = [e for e in summary["entries"] if e["backends"].get("rust")]
    row_height = 34
    height = 130 + row_height * len(entries) + 30
    canvas = Canvas(940, height, "Peak resident memory per backend (log scale, MB)",
                    f"Source: {summary['report_id']} · /usr/bin/time -v VmHWM per timed run, median of N repeats")
    left, right = 250, 830
    values = [v for e in entries for b in ("rust", "python", "r")
              if (v := (e["backends"].get(b) or {}).get("median_peak_rss_mb"))]
    if not values:
        canvas.save(path)
        return
    low, high = max(min(values) / 2, 1), max(values) * 2
    for decade in range(0, int(math.log10(high)) + 2):
        value = 10 ** decade
        if value < low or value > high:
            continue
        x = log_scale(value, low, high, left, right)
        canvas.line(x, 62, x, 62 + row_height * len(entries), GRID)
        canvas.text(x, 56, f"{value:,.0f}", size=11, fill=MUTED, anchor="middle")
    for index, entry in enumerate(entries):
        y = 62 + index * row_height
        canvas.text(left - 12, y + 22, entry["capability"].replace(".v1", ""), size=12, fill=INK, anchor="end")
        for offset, (backend, colour) in enumerate((("rust", RUST), ("python", PYTHON), ("r", R_LANG))):
            value = (entry["backends"].get(backend) or {}).get("median_peak_rss_mb")
            if not value:
                continue
            x = log_scale(value, low, high, left, right)
            canvas.rect(x, y + offset * 9 + 4, max(right - x, 1), 7, colour)
            canvas.text(x + 6, y + offset * 9 + 11, f"{value:,.1f} MB", size=10, fill=colour)
        saving = entry.get("memory_saving")
        if saving is not None:
            canvas.text(right + 10, y + 22, f"−{saving * 100:.0f}%", size=11, fill=MUTED)
    canvas.save(path)


def figure_ratio_ci(summary: dict, reports: Path, path: Path) -> None:
    rows = []
    for entry in summary["entries"]:
        report_file = reports / entry["report"]
        if not report_file.is_file():
            continue
        times = runs_by_backend(json.loads(report_file.read_text(encoding="utf-8")))
        if "rust" not in times:
            continue
        for backend in ("python", "r"):
            if backend not in times:
                continue
            ratio, low, high = ratio_ci(times["rust"], times[backend])
            noise = 2 * (coefficient_of_variation(times["rust"]) + coefficient_of_variation(times[backend]))
            rows.append((f"{entry['capability'].replace('.v1', '')} · {backend}", ratio, low, high, noise))
    rows.sort(key=lambda row: row[1])
    row_height = 34
    height = 140 + row_height * len(rows) + 40
    canvas = Canvas(940, height, "Is the gap real? — ratio with bootstrap 95% CI (log scale)",
                    f"Source: {summary['report_id']} raw runs · {BOOTSTRAP_ROUNDS:,} resamples of the N recorded wall times, seed {BOOTSTRAP_SEED} · "
                    "shaded band = ±2×(CV_rust+CV_other): inside it the gap is not resolvable at this N")
    left, right = 300, 870
    ratios = [row[1] for row in rows] + [row[3] for row in rows]
    low = min(0.9, min(ratios) * 0.9) if ratios else 0.9
    high = max(ratios or [2]) * 1.3
    for decade in range(-1, int(math.log10(high)) + 2):
        value = 10 ** decade
        if value < low or value > high:
            continue
        x = log_scale(value, low, high, left, right)
        canvas.line(x, 66, x, 66 + row_height * len(rows), GRID)
        canvas.text(x, 60, f"{value:g}×", size=11, fill=MUTED, anchor="middle")
    one = log_scale(1.0, low, high, left, right)
    material_line = log_scale(MATERIAL_RATIO, low, high, left, right)
    canvas.line(one, 66, one, 66 + row_height * len(rows), WARN, width=1.5, dash="5 4")
    canvas.text(one, 60, "1.00× (no difference)", size=11, fill=WARN, anchor="middle")
    canvas.line(material_line, 66, material_line, 66 + row_height * len(rows), ACCENT, width=1.2, dash="3 4")
    canvas.text(material_line, 74 + row_height * len(rows), f"{MATERIAL_RATIO:.2f}× = materiality floor",
                size=11, fill=ACCENT, anchor="middle")
    for index, (label, ratio, low_ci, high_ci, noise) in enumerate(rows):
        y = 66 + index * row_height + 16
        canvas.text(left - 12, y + 5, label, size=12, fill=INK, anchor="end")
        x_low = log_scale(low_ci, low, high, left, right)
        x_high = log_scale(high_ci, low, high, left, right)
        x_mid = log_scale(ratio, low, high, left, right)
        # Run-to-run dispersion, reported for context only (not a threshold).
        canvas.rect(one, y - 1,
                    max(log_scale(min(1 + noise, high), low, high, left, right) - one, 0), 12,
                    WARN, opacity=0.14)
        canvas.line(x_low, y + 5, x_high, y + 5, MUTED, width=1.5)
        canvas.line(x_low, y + 1, x_low, y + 9, MUTED, width=1.5)
        canvas.line(x_high, y + 1, x_high, y + 9, MUTED, width=1.5)
        colour = PYTHON if label.endswith("python") else R_LANG
        if high_ci <= 1.0 or low_ci <= 1.0:
            verdict, marker = "within-noise", WARN
        elif low_ci >= MATERIAL_RATIO:
            verdict, marker = "material", colour
        else:
            verdict, marker = "detectable-immaterial", ACCENT
        canvas.rect(x_mid - 3.5, y, 7, 10, marker)
        canvas.text(x_high + 8, y + 9, f"{ratio:.2f}× [{low_ci:.2f}, {high_ci:.2f}] {verdict}",
                    size=10, fill=marker)
    canvas.save(path)


def figure_granularity(reports: Path, path: Path) -> None:
    shapes = [("one", "1 × 100 Mb"), ("twenty", "20 × 5 Mb"), ("many", "100,000 × 1 kb")]
    series = {"rust": (RUST, []), "python": (PYTHON, []), "r": (R_LANG, [])}
    for key, _label in shapes:
        report_file = reports / f"out-gran-{key}" / "sequence.stats.v1.benchmark.json"
        if not report_file.is_file():
            continue
        times = runs_by_backend(json.loads(report_file.read_text(encoding="utf-8")))
        for backend in series:
            series[backend][1].append(min(times.get(backend, [float("nan")])) if times.get(backend) else None)
    if not any(values for _c, values in series.values()):
        return
    canvas = Canvas(940, 420, "Why sequence.stats shows almost no gap — record granularity at fixed ~100 Mb",
                    "Source: repository reports (granularity experiment) · same total bases, same machine, N repeats "
                    "· python pays a fixed cost per record, the Rust kernel is one pass per line")
    left, top, right, bottom = 110, 80, 880, 330
    values = [v for _c, vals in series.values() for v in vals if v]
    low, high = max(min(values) / 2, 1), max(values) * 1.6
    for decade in range(0, int(math.log10(high)) + 2):
        value = 10 ** decade
        if value < low or value > high:
            continue
        y = bottom - (math.log10(value) - math.log10(low)) / (math.log10(high) - math.log10(low)) * (bottom - top)
        canvas.line(left, y, right, y, GRID)
        canvas.text(left - 8, y + 4, f"{value:,.0f} ms", size=11, fill=MUTED, anchor="end")
    step = (right - left) / (len(shapes) - 1) if len(shapes) > 1 else 1
    for index, (_key, label) in enumerate(shapes):
        x = left + index * step
        canvas.line(x, top, x, bottom, GRID)
        canvas.text(x, bottom + 22, label, size=12, fill=INK, anchor="middle")
    for backend, (colour, values_) in series.items():
        points = []
        for index, value in enumerate(values_):
            if value is None:
                continue
            x = left + index * step
            y = bottom - (math.log10(value) - math.log10(low)) / (math.log10(high) - math.log10(low)) * (bottom - top)
            points.append(f"{x:.1f},{y:.1f}")
            canvas.rect(x - 4, y - 4, 8, 8, colour, radius=4)
        if len(points) > 1:
            canvas.parts.append(
                f'<polyline points="{" ".join(points)}" fill="none" stroke="{colour}" stroke-width="2"/>'
            )
        canvas.text(right + 8, top + 14 + list(series).index(backend) * 18, backend, size=12, fill=colour)
    canvas.save(path)


def figure_stability(reports: Path, summary: dict, path: Path) -> None:
    picks = [e for e in summary["entries"] if e["backends"].get("r")][:4]
    rows = []
    for entry in picks:
        report_file = reports / entry["report"]
        if report_file.is_file():
            rows.append((entry["capability"].replace(".v1", ""),
                         runs_by_backend(json.loads(report_file.read_text(encoding="utf-8")))))
    if not rows:
        return
    row_height = 52
    canvas = Canvas(940, 130 + row_height * len(rows), "Raw repeats — what the medians are built from",
                    f"Source: {summary['report_id']} · every recorded wall_ms per backend (N repeats, warm page cache)")
    left, right = 250, 880
    values = [v for _n, times in rows for vals in times.values() for v in vals]
    low, high = max(min(values) / 1.5, 0.5), max(values) * 1.5
    for decade in range(0, int(math.log10(high)) + 2):
        value = 10 ** decade
        if value < low or value > high:
            continue
        x = log_scale(value, low, high, left, right)
        canvas.line(x, 60, x, 60 + row_height * len(rows), GRID)
        canvas.text(x, 54, f"{value:,.0f}", size=11, fill=MUTED, anchor="middle")
    for index, (name, times) in enumerate(rows):
        y = 60 + index * row_height
        canvas.text(left - 12, y + 22, name, size=12, fill=INK, anchor="end")
        for offset, (backend, colour) in enumerate((("rust", RUST), ("python", PYTHON), ("r", R_LANG))):
            values_ = times.get(backend)
            if not values_:
                continue
            for value in values_:
                x = log_scale(value, low, high, left, right)
                canvas.rect(x - 2.5, y + offset * 11 + 4, 5, 9, colour, opacity=0.75, radius=2.5)
            median = sorted(values_)[len(values_) // 2]
            x = log_scale(median, low, high, left, right)
            canvas.line(x, y + offset * 11 + 2, x, y + offset * 11 + 15, colour, width=2)
    canvas.save(path)


def figure_series(comparison: dict, summary: dict, path: Path) -> None:
    """Same four capabilities in the September small-fixture run and here."""
    old = {e["capability"]: e for e in comparison["entries"]}
    new = {e["capability"]: e for e in summary["entries"]}
    rows = []
    for capability in sorted(set(old) & set(new)):
        def ratio(entry: dict, backend: str) -> float | None:
            backends = entry["backends"]
            baseline = (backends.get("rust") or {}).get("median_wall_ms")
            other = (backends.get(backend) or {}).get("median_wall_ms")
            return other / baseline if baseline and other else None
        rows.append((capability.replace(".v1", ""),
                     ratio(old[capability], "python"), ratio(new[capability], "python"),
                     ratio(old[capability], "r"), ratio(new[capability], "r")))
    if not rows:
        return
    row_height = 60
    canvas = Canvas(940, 150 + row_height * len(rows),
                    "The same capability, two fixture sizes — ratios move with the input",
                    f"September: {comparison['report_id']} (small repository fixtures, laptop/WSL) · "
                    f"now: {summary['report_id']} (large inputs, NAS) · python vs rust wall-time ratio")
    left, right = 250, 860
    values = [v for row in rows for v in row[1:] if v]
    low, high = 0.8, max(values or [10]) * 1.5
    for decade in range(0, int(math.log10(high)) + 1):
        value = 10 ** decade
        if value < low:
            continue
        x = log_scale(value, low, high, left, right)
        canvas.line(x, 60, x, 60 + row_height * len(rows), GRID)
        canvas.text(x, 54, f"{value}×", size=11, fill=MUTED, anchor="middle")
    for index, (name, old_python, new_python, old_r, new_r) in enumerate(rows):
        y = 60 + index * row_height
        canvas.text(left - 12, y + 26, name, size=12, fill=INK, anchor="end")
        if old_python:
            canvas.rect(left, y + 8, log_scale(old_python, low, high, left, right) - left, 9, PYTHON, opacity=0.45)
            canvas.text(log_scale(old_python, low, high, left, right) + 6, y + 16, f"small {old_python:.2f}×", size=10, fill=PYTHON)
        if new_python:
            canvas.rect(left, y + 20, log_scale(new_python, low, high, left, right) - left, 9, PYTHON)
            canvas.text(log_scale(new_python, low, high, left, right) + 6, y + 28, f"large {new_python:.2f}×", size=10, fill=PYTHON)
        if old_r:
            canvas.rect(left, y + 32, log_scale(old_r, low, high, left, right) - left, 9, R_LANG, opacity=0.45)
            canvas.text(log_scale(old_r, low, high, left, right) + 6, y + 40, f"small {old_r:.2f}×", size=10, fill=R_LANG)
        if new_r:
            canvas.rect(left, y + 44, log_scale(new_r, low, high, left, right) - left, 9, R_LANG)
            canvas.text(log_scale(new_r, low, high, left, right) + 6, y + 52, f"large {new_r:.2f}×", size=10, fill=R_LANG)
    canvas.save(path)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--summary", required=True)
    parser.add_argument("--reports", required=True, help="directory with the raw per-capability reports")
    parser.add_argument("--comparison", help="an earlier summary.json to compare against")
    parser.add_argument("--granularity", help="directory with the granularity experiment reports")
    parser.add_argument("--output", required=True)
    arguments = parser.parse_args()

    summary = json.loads(Path(arguments.summary).read_text(encoding="utf-8"))
    reports = Path(arguments.reports)
    output = Path(arguments.output)
    output.mkdir(parents=True, exist_ok=True)

    figure_speedup(summary, output / "fig-speedup-layered.svg")
    figure_medians(summary, output / "fig-absolute-medians.svg")
    figure_memory(summary, output / "fig-memory.svg")
    figure_ratio_ci(summary, reports, output / "fig-ratio-ci.svg")
    figure_stability(reports, summary, output / "fig-repeat-stability.svg")
    if arguments.comparison:
        figure_series(json.loads(Path(arguments.comparison).read_text(encoding="utf-8")),
                      summary, output / "fig-series-two-windows.svg")
    if arguments.granularity:
        figure_granularity(Path(arguments.granularity), output / "fig-granularity.svg")
    print(f"wrote figures to {output}: " + ", ".join(sorted(p.name for p in output.glob("*.svg"))))


if __name__ == "__main__":
    main()
