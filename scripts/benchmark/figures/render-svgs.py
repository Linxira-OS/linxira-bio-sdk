#!/usr/bin/env python3
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/figures/render-svgs.py
# Role: render the self-contained SVG figure set in zh and en (absolute medians, memory, ratio CI, repeat stability, layered speedup, two-window series, record granularity, method)
# Windows: 2026-10-11, comparing against the 2026-09-13 archive
# Direction: publication figures; reads the archive plus raw reports
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
"""Render self-contained SVG figures from archived benchmark reports.

Hard rule (do not weaken it): a figure is a **presentation** of the archived
numbers, never a re-derivation of them. The medians, ratios, intervals and
verdicts drawn here are byte-identical to `statistics.tsv` / `summary.json`;
`figures/verify-figures.py` re-derives every plotted number from the archive and
fails on any drift. What may change is presentation only — font sizes, labels,
bar direction, dot/bar choice, panel layout, language. There is **no smoothing,
no interpolation, no axis truncation, no dropped interval, no unequal N**. A
figure must never make a difference look larger or smaller than the raw
measurements say; it may only make the existing numbers easier to read.

Both languages are rendered from the same numbers in the same run, so the zh and
en figures cannot drift apart: `--lang zh --output .../figures/zh` and
`--lang en --output .../figures/en`.

Usage:
    python3 scripts/benchmark/figures/render-svgs.py --lang zh \
        --summary benchmark-results/<date>/summary.json \
        --reports /path/to/reports-with-raw-runs \
        --comparison benchmark-results/2026-09-13/summary.json \
        --granularity /path/to/granularity-reports \
        --output benchmark-results/<date>/blog-mirror/figures/zh

Every figure targets the dark site theme (slate card, light ink); the palette is
fixed here so the SVGs stay self-contained and theme-independent.
"""
from __future__ import annotations

import argparse
import json
import math
import random
import statistics
import unicodedata
from pathlib import Path

# --- palette (self-contained; matches the site's dark blog theme) ----------
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

# Caption layout: 150 display columns fits the 940 px canvas at 11.5 px; CJK
# glyphs count as two columns, so a Chinese caption is not one huge line.
CAPTION_COLUMNS = 150
CAPTION_SPACE = 96

BOOTSTRAP_SEED = 20261011
BOOTSTRAP_ROUNDS = 10_000
# Same pre-registered rule as scripts/benchmark/analysis/statistics-tables.py: detectable when
# the ratio CI excludes 1.0, material only at/above this floor.
MATERIAL_RATIO = 1.10

# --- value-label and direction conventions ---------------------------------
# Sizes below are deliberately large: the first review of these figures found the
# numeric labels unreadable at 10-11 px in a side-by-side preview.
VALUE_SIZE = 13
LEGEND_SIZE = 13
AXIS_SIZE = 12
CAPTION_SIZE = 11.5
DIRECTION_SIZE = 12.5


class Strings:
    """One place for every word that appears inside a figure."""

    def __init__(self, lang: str) -> None:
        self.lang = lang
        if lang == "zh":
            self._zh()
        else:
            self._en()

    # -- shared ------------------------------------------------------------
    def _common(self, lower: str, higher: str, ms_axis: str, mb_axis: str,
                ratio_axis: str, raw_note: str, unit_ms: str, unit_mb: str) -> None:
        self.lower_better = lower
        self.higher_better = higher
        self.ms_axis = ms_axis
        self.mb_axis = mb_axis
        self.ratio_axis = ratio_axis
        self.raw_note = raw_note
        self.unit_ms = unit_ms
        self.unit_mb = unit_mb

    def _zh(self) -> None:
        self._common(
            "↓ 越低越好", "↑ 越高越好（对 Rust 而言）",
            "墙钟时间（毫秒，线性刻度）", "峰值内存（MB，线性刻度）", "相对 Rust 的倍数（线性刻度）",
            "图中数字为 10 次计时重复的原始中位数，仅作呈现：无平滑、无坐标轴截断、无任何统计变换",
            "ms", "MB")
        self.backend = "后端"
        self.median = "中位数"
        self.method_title = "一次能力测量如何变成一条结论"
        self.method_caption = "输入固定并记录摘要 → 1 次不计时预热 → 10 次计时重复（报告只含这 10 条）→ 中位数/IQR/CV → bootstrap 置信区间 → 三段式判定。预热那次不进统计；裁剪口径只是稳健性检查。"
        self.medians_title = "各后端绝对墙钟时间"
        self.medians_caption_prefix = "每根条 = 该后端 10 次计时重复的中位数（同一输入、同一机器）。"
        self.memory_title = "各后端峰值内存"
        self.memory_caption_prefix = "每根条 = 该后端 10 次计时重复的峰值常驻内存（VmHWM）中位数。"
        self.ratio_title = "差距是真的吗？比值与 bootstrap 95% 置信区间"
        self.ratio_caption = ("每条 = 一个后端相对 Rust 的比值中位数与其 95% 区间（10,000 次重采样，种子 20261011）。"
                              "判定只看区间：区间完全 ≥ 1.10 记为 material；区间跨过 1.00 记为 within-noise（测不出可分辨差异）。"
                              "阴影带为 ±2×(两端 CV 之和)，仅作参考，不参与判定。")
        self.stability_title = "原始重复：中位数是由什么构成的"
        self.stability_caption = "每行 = 该能力 × 后端：每个点是 10 次计时中的一次原始 wall_ms，竖线为该后端中位数。"
        self.speedup_title = "Rust 相对解释器后端的墙钟比值"
        self.speedup_caption = "每条 = 相对 Rust 的中位数比值（同一输入、同一机器、每后端 1 次预热 + 10 次计时）。左侧是 C 库已覆盖循环的能力，右侧是解释器逐元素迭代的能力。"
        self.granularity_title = "记录粒度不是差距的来源——总量同为 ~100 Mb，三种记录形态"
        self.granularity_caption_prefix = "每根柱 = 该形态下该后端 10 次计时重复的中位数。"
        self.series_title = "同一能力、两个窗口：小夹具与大输入的比值"
        self.series_caption = "small = 2026-09-13 窗口的小夹具（N=5）；large = 本窗口的大输入（N=10）。不同输入规模之间的数字不互相替代，只用于说明比值随规模展开。"
        self.legend_small = "小夹具（九月窗口）"
        self.legend_large = "大输入（本窗口）"
        self.records_label = "记录"

    def _en(self) -> None:
        self._common(
            "↓ lower is better", "↑ higher is better for Rust",
            "wall time (ms, linear scale)", "peak RSS (MB, linear scale)", "ratio vs rust (linear scale)",
            "Every number is the raw median of the 10 timed repeats — presentation only: no smoothing, no axis truncation, no statistical transformation",
            "ms", "MB")
        self.backend = "backend"
        self.median = "median"
        self.method_title = "How one capability measurement becomes a verdict"
        self.method_caption = "Digest-checked inputs → 1 untimed warm-up → 10 timed repeats (the report holds only these 10) → median/IQR/CV → bootstrap confidence interval → three-way verdict. The warm-up never enters a statistic; the trimming bases are robustness checks only."
        self.medians_title = "Absolute wall time per backend"
        self.medians_caption_prefix = "Each bar is that backend's median of 10 timed repeats (same input, same machine)."
        self.memory_title = "Peak resident memory per backend"
        self.memory_caption_prefix = "Each bar is that backend's median peak RSS (VmHWM) over 10 timed repeats."
        self.ratio_title = "Is the gap real? Ratio with bootstrap 95% confidence interval"
        self.ratio_caption = ("Each row is one backend's median ratio against Rust with its 95% interval "
                              "(10,000 resamples, seed 20261011). The verdict reads the interval only: entirely ≥ 1.10 is "
                              "material; an interval crossing 1.00 is within-noise (no resolvable difference). "
                              "The shaded band is ±2×(CV sum), shown for context and not used as a threshold.")
        self.stability_title = "Raw repeats — what the medians are built from"
        self.stability_caption = "Each row is one capability × backend: every dot is one timed repeat's raw wall_ms, the vertical line is that backend's median."
        self.speedup_title = "Rust vs interpreter backends — wall-time ratio"
        self.speedup_caption = "Each bar is the median ratio against Rust (same input, same machine, one warm-up plus 10 timed repeats per backend). Left: a C library already covers the loop. Right: the interpreter iterates per element."
        self.granularity_title = "Record granularity is not the source of the gap — same ~100 Mb, three record shapes"
        self.granularity_caption_prefix = "Each bar is that backend's median of 10 timed repeats in that shape."
        self.series_title = "Same capability, two windows: ratios on small fixtures and on large inputs"
        self.series_caption = "small = the 2026-09-13 window's fixtures (N=5); large = this window's large inputs (N=10). Numbers from different input scales never replace one another; they show how ratios unfold with scale."
        self.legend_small = "small fixtures (September)"
        self.legend_large = "large inputs (this window)"
        self.records_label = "records"


STRINGS: Strings = Strings("en")  # replaced by --lang in main()


def esc(text: str) -> str:
    return str(text).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def _display_width(text: str) -> int:
    """Columns a string occupies: CJK glyphs are double-width."""
    return sum(2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1 for ch in text)


def _tokens(text: str) -> list[str]:
    """Split into break opportunities: Latin words, single CJK glyphs, spaces."""
    tokens: list[str] = []
    buffer = ""
    for ch in text:
        if ch == " ":
            if buffer:
                tokens.append(buffer)
                buffer = ""
            tokens.append(" ")
        elif _display_width(ch) == 2:
            if buffer:
                tokens.append(buffer)
                buffer = ""
            tokens.append(ch)
        else:
            buffer += ch
    if buffer:
        tokens.append(buffer)
    return tokens


def _wrap(text: str, limit: int) -> list[str]:
    """Greedy wrap by display width, so Chinese captions are not clipped.

    CJK text has no spaces, so a whitespace-only wrapper would emit one
    enormous line; here each CJK glyph is its own break opportunity and Latin
    words stay whole.
    """
    lines: list[str] = []
    current = ""
    width = 0
    for token in _tokens(str(text)):
        if token == " ":
            if not current:
                continue
            token = " "
        token_width = _display_width(token)
        if current and width + token_width > limit:
            lines.append(current.rstrip())
            current, width = "", 0
            if token == " ":
                continue
        current += token
        width += token_width
    if current.strip():
        lines.append(current.rstrip())
    return lines or [""]


class Canvas:
    """Minimal SVG writer for log-scaled figures."""

    def __init__(self, width: int, height: int, title: str, caption: str,
                 direction: str | None = None) -> None:
        self.width = width
        self.height = height
        self.parts = [
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
            f'width="{width}" height="{height}" role="img" aria-label="{esc(title)}">',
            f'<rect width="{width}" height="{height}" fill="{CARD}"/>',
            f'<text x="24" y="34" fill="{INK}" font-family="{FONT}" '
            f'font-size="20" font-weight="600">{esc(title)}</text>',
        ]
        if direction:
            # "lower is better" / "higher is better" is a property of the metric,
            # not a claim about the result, so it is stated on every data figure.
            self.parts.append(
                f'<text x="{width - 24}" y="34" fill="{ACCENT}" font-family="{FONT}" '
                f'font-size="{DIRECTION_SIZE}" font-weight="600" text-anchor="end">{esc(direction)}</text>'
            )
        self.caption = caption

    def text(self, x: float, y: float, content: str, *, size: int = AXIS_SIZE,
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

    def circle(self, x: float, y: float, r: float, fill: str, *, opacity: float = 1.0) -> None:
        # Two decimals: the figure checker converts positions back to values, and
        # 0.1 px quantisation was visible on rows with large scales.
        self.parts.append(f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{r:.2f}" fill="{fill}" '
                          f'opacity="{opacity}"/>')

    def line(self, x1: float, y1: float, x2: float, y2: float, stroke: str,
             *, width: float = 1.0, dash: str | None = None, opacity: float = 1.0) -> None:
        dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
        self.parts.append(
            f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}" '
            f'stroke="{stroke}" stroke-width="{width}" opacity="{opacity}"{dash_attr}/>'
        )

    def save(self, path: Path) -> None:
        # Captions carry the disclosure line; wrap them so nothing is clipped.
        for index, line in enumerate(_wrap(self.caption, CAPTION_COLUMNS)):
            total = len(_wrap(self.caption, CAPTION_COLUMNS))
            y = self.height - 14 - (total - 1 - index) * 15
            self.parts.append(
                f'<text x="24" y="{y}" fill="{MUTED}" font-family="{FONT}" '
                f'font-size="{CAPTION_SIZE}">{esc(line)}</text>'
            )
        self.parts.append("</svg>")
        path.write_text("\n".join(self.parts) + "\n", encoding="utf-8")


def lin_scale(value: float, low: float, high: float, start: float, end: float) -> float:
    """Uniform (linear) mapping of value onto [start, end].

    Review feedback: log axes make every grid cell a factor of ten (1 -> 10 ->
    100), which reads as an uneven stretch. Every figure now uses a linear axis
    over a range truncated to the data it shows; the range is stated on the axis.
    """
    if high <= low:
        return start
    return start + (value - low) / (high - low) * (end - start)


def nice_step(span: float, target_ticks: int = 6) -> float:
    if span <= 0:
        return 1.0
    raw = span / max(target_ticks, 1)
    magnitude = 10 ** math.floor(math.log10(raw))
    for factor in (1, 2, 2.5, 5, 10):
        if magnitude * factor >= raw:
            return magnitude * factor
    return magnitude * 10


def ticks_for(low: float, high: float, target_ticks: int = 6) -> tuple[list[float], float]:
    step = nice_step(high - low, target_ticks)
    start = math.ceil(low / step) * step
    values = []
    value = start
    while value <= high + step * 1e-6:
        values.append(value)
        value += step
    return values, step


def legend_row(canvas: Canvas, y: float, entries: list[tuple[str, str]], *, x: float = 24,
               gap: int = 118) -> None:
    """One legend line under the title.

    The previous legend was positioned relative to the plot's right edge; when
    that edge moved, the blue and green entries fell outside the canvas and only
    "rust" survived. The legend now has its own row and cannot be clipped.
    """
    for offset, (label, colour) in enumerate(entries):
        cursor = x + offset * gap
        canvas.rect(cursor, y - 10, 11, 11, colour, radius=2)
        canvas.text(cursor + 17, y, label, size=LEGEND_SIZE, fill=colour)


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


def median_of(values: list[float]) -> float:
    ordered = sorted(values)
    count = len(ordered)
    return ordered[count // 2] if count % 2 else statistics.fmean(ordered[count // 2 - 1:count // 2 + 1])


def format_number(value: float) -> str:
    if value >= 1000:
        return f"{value:,.0f}"
    if value >= 10:
        return f"{value:.0f}"
    if value >= 1:
        return f"{value:.2f}".rstrip("0").rstrip(".")
    return f"{value:g}"


def figure_speedup(summary: dict, path: Path) -> None:
    s = STRINGS
    rows = []
    for entry in summary["entries"]:
        backends = entry["backends"]
        baseline = (backends.get("rust") or {}).get("median_wall_ms")
        if not baseline:
            continue
        rows.append((
            entry["capability"].replace(".v1", ""),
            (backends.get("python") or {}).get("median_wall_ms", 0) / baseline,
            ((backends.get("r") or {}).get("median_wall_ms", 0) / baseline) if backends.get("r") else None,
        ))
    if not rows:
        return
    rows.sort(key=lambda row: row[1])
    row_height = 50
    canvas = Canvas(940, 190 + row_height * len(rows) + CAPTION_SPACE, s.speedup_title,
                    f"{s.speedup_caption} {s.raw_note}", s.higher_better)
    legend_row(canvas, 96, [("python", PYTHON), ("r", R_LANG)])
    left, right = 300, 770
    low = 1.0                      # ratios are relative to rust: truncated floor is 1x
    high = max([row[2] or row[1] for row in rows] or [2]) * 1.06
    ticks, _step = ticks_for(low, high, 6)
    for value in ticks:
        x = lin_scale(value, low, high, left, right)
        canvas.line(x, 116, x, 116 + row_height * len(rows), GRID)
        canvas.text(x, 108, f"{format_number(value)}×", size=AXIS_SIZE, fill=MUTED, anchor="middle")
    canvas.text(left, 108, "", size=AXIS_SIZE)
    canvas.text(24, 78, f"{s.ratio_axis}，显示范围 {format_number(low)}–{format_number(high)}×"
                if s.lang == "zh" else f"{s.ratio_axis}, range shown {format_number(low)}–{format_number(high)}×",
                size=AXIS_SIZE, fill=MUTED)
    for index, (name, python_ratio, r_ratio) in enumerate(rows):
        y = 116 + index * row_height
        canvas.text(left - 16, y + 32, name, size=14, fill=INK, anchor="end")
        for offset, (ratio, colour) in enumerate(((python_ratio, PYTHON), (r_ratio, R_LANG))):
            if ratio is None:
                continue
            x = lin_scale(ratio, low, high, left, right)
            canvas.rect(left, y + offset * 18 + 4, x - left, 14, colour, radius=2)
            canvas.text(x + 8, y + offset * 18 + 16, f"{ratio:.2f}×", size=VALUE_SIZE,
                        fill=colour, weight="600")
    canvas.save(path)


def figure_medians(summary: dict, path: Path) -> None:
    """Per-capability rows on a truncated linear scale (was: log axis)."""
    s = STRINGS
    entries = [e for e in summary["entries"] if e["backends"].get("rust")]
    if not entries:
        return
    group = 58
    left, right = 300, 760
    legend = 96
    canvas = Canvas(940, 200 + group * len(entries) + CAPTION_SPACE, s.medians_title,
                    f"{s.medians_caption_prefix} {s.raw_note}", s.lower_better)
    legend_row(canvas, legend, [("rust", RUST), ("python", PYTHON), ("r", R_LANG)])
    for index, entry in enumerate(entries):
        y = legend + 46 + index * group
        canvas.text(left - 16, y + 34, entry["capability"].replace(".v1", ""), size=14,
                    fill=INK, anchor="end")
        row_values = [(entry["backends"].get(b) or {}).get("median_wall_ms") or 0
                      for b in ("rust", "python", "r")]
        row_max = max(row_values) * 1.08
        canvas.text(right + 10, y + 34, f"0 – {format_number(row_max)} {s.unit_ms}",
                    size=AXIS_SIZE, fill=MUTED)
        for offset, (backend, colour) in enumerate((("rust", RUST), ("python", PYTHON), ("r", R_LANG))):
            value = (entry["backends"].get(backend) or {}).get("median_wall_ms")
            if not value:
                continue
            x = lin_scale(value, 0.0, row_max, left, right)
            canvas.rect(left, y + offset * 16 + 4, x - left, 14, colour, radius=2)
            canvas.text(x + 8, y + offset * 16 + 16, f"{format_number(value)} {s.unit_ms}",
                        size=VALUE_SIZE, fill=colour, weight="600")
    canvas.save(path)


def figure_memory(summary: dict, path: Path) -> None:
    s = STRINGS
    entries = [e for e in summary["entries"] if e["backends"].get("rust")]
    values = [v for entry in entries for backend in ("rust", "python", "r")
              if (v := (entry["backends"].get(backend) or {}).get("median_peak_rss_mb"))]
    if not values:
        return
    group = 58
    left, right = 300, 780
    legend = 96
    low, high = 0.0, max(values) * 1.08
    canvas = Canvas(940, 200 + group * len(entries) + CAPTION_SPACE, s.memory_title,
                    f"{s.memory_caption_prefix} {s.raw_note}", s.lower_better)
    legend_row(canvas, legend, [("rust", RUST), ("python", PYTHON), ("r", R_LANG)])
    ticks, _step = ticks_for(low, high, 6)
    for value in ticks:
        x = lin_scale(value, low, high, left, right)
        canvas.line(x, legend + 46, x, legend + 46 + group * len(entries), GRID)
        canvas.text(x, legend + 38, format_number(value), size=AXIS_SIZE, fill=MUTED, anchor="middle")
    canvas.text(24, legend + 38, s.mb_axis, size=AXIS_SIZE, fill=MUTED)
    for index, entry in enumerate(entries):
        y = legend + 46 + index * group
        canvas.text(left - 16, y + 34, entry["capability"].replace(".v1", ""), size=14,
                    fill=INK, anchor="end")
        for offset, (backend, colour) in enumerate((("rust", RUST), ("python", PYTHON), ("r", R_LANG))):
            value = (entry["backends"].get(backend) or {}).get("median_peak_rss_mb")
            if not value:
                continue
            x = lin_scale(value, low, high, left, right)
            canvas.rect(left, y + offset * 16 + 4, x - left, 14, colour, radius=2)
            canvas.text(x + 8, y + offset * 16 + 16, f"{value:.1f} {s.unit_mb}",
                        size=VALUE_SIZE, fill=colour, weight="600")
    canvas.save(path)


def figure_ratio_ci(summary: dict, reports: Path, path: Path) -> None:
    s = STRINGS
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
            ratio, low_ci, high_ci = ratio_ci(times["rust"], times[backend])
            noise = 2 * (coefficient_of_variation(times["rust"]) + coefficient_of_variation(times[backend]))
            rows.append((f"{entry['capability'].replace('.v1', '')} · {backend}", ratio, low_ci, high_ci, noise))
    if not rows:
        return
    rows.sort(key=lambda row: row[1])
    row_height = 46
    legend = 96
    canvas = Canvas(940, 210 + row_height * len(rows) + CAPTION_SPACE, s.ratio_title,
                    f"{s.ratio_caption} {s.raw_note}", s.higher_better)
    left, right = 330, 760
    low = min(1.0, min(row[2] for row in rows))
    low = math.floor(low * 20) / 20          # truncated floor, stated on the axis
    high = max(row[3] for row in rows) * 1.02
    ticks, _step = ticks_for(low, high, 6)
    for value in ticks:
        x = lin_scale(value, low, high, left, right)
        canvas.line(x, legend + 40, x, legend + 40 + row_height * len(rows), GRID)
        canvas.text(x, legend + 32, f"{format_number(value)}×", size=AXIS_SIZE, fill=MUTED,
                    anchor="middle")
    canvas.text(24, legend + 32, f"{s.ratio_axis}，显示范围 {format_number(low)}–{format_number(high)}×"
                if s.lang == "zh" else f"{s.ratio_axis}, range shown {format_number(low)}–{format_number(high)}×",
                size=AXIS_SIZE, fill=MUTED)
    one = lin_scale(1.0, low, high, left, right)
    floor = lin_scale(MATERIAL_RATIO, low, high, left, right)
    canvas.line(one, legend + 36, one, legend + 40 + row_height * len(rows), WARN, width=1.6, dash="6 4")
    canvas.line(floor, legend + 36, floor, legend + 40 + row_height * len(rows), ACCENT, width=1.4, dash="4 4")
    canvas.text(one, legend + 24, "1.00×", size=AXIS_SIZE, fill=WARN, anchor="middle")
    canvas.text(floor, legend + 24, f"{MATERIAL_RATIO:.2f}×", size=AXIS_SIZE, fill=ACCENT, anchor="middle")
    for index, (label, ratio, low_ci, high_ci, noise) in enumerate(rows):
        y = legend + 40 + index * row_height + 16
        canvas.text(left - 16, y + 5, label, size=13, fill=INK, anchor="end")
        x_low = lin_scale(low_ci, low, high, left, right)
        x_high = lin_scale(high_ci, low, high, left, right)
        x_mid = lin_scale(ratio, low, high, left, right)
        band_end = lin_scale(min(1 + noise, high), low, high, left, right)
        canvas.rect(one, y - 2, max(band_end - one, 0), 16, WARN, opacity=0.14)
        canvas.line(x_low, y + 6, x_high, y + 6, MUTED, width=2)
        canvas.line(x_low, y + 1, x_low, y + 11, MUTED, width=2)
        canvas.line(x_high, y + 1, x_high, y + 11, MUTED, width=2)
        colour = PYTHON if label.endswith("python") else R_LANG
        if high_ci <= 1.0 or low_ci <= 1.0:
            verdict, marker = "within-noise", WARN
        elif low_ci >= MATERIAL_RATIO:
            verdict, marker = "material", colour
        else:
            verdict, marker = "detectable-immaterial", ACCENT
        canvas.rect(x_mid - 4, y, 8, 13, marker, radius=2)
        annotation = f"{ratio:.2f}× [{low_ci:.2f}, {high_ci:.2f}] {verdict}"
        if x_high + 10 + 7.0 * len(annotation) > canvas.width - 10:
            canvas.text(x_low - 10, y + 11, annotation, size=VALUE_SIZE, fill=marker,
                        anchor="end", weight="600")
        else:
            canvas.text(x_high + 10, y + 11, annotation, size=VALUE_SIZE, fill=marker, weight="600")
    canvas.save(path)


def _box(canvas: Canvas, x: float, y: float, width: float, height: float,
         lines: list[str], colour: str, *, size: int = 12) -> None:
    canvas.rect(x, y, width, height, colour, opacity=0.12, radius=6)
    canvas.parts.append(
        f'<rect x="{x:.1f}" y="{y:.1f}" width="{width:.1f}" height="{height:.1f}" rx="6" '
        f'fill="none" stroke="{colour}" stroke-width="1.3"/>')
    step = 16
    start = y + height / 2 - (len(lines) - 1) * step / 2 + 5
    for index, line in enumerate(lines):
        canvas.text(x + width / 2, start + index * step, line, size=size, fill=INK, anchor="middle")


def _arrow(canvas: Canvas, x1: float, y1: float, x2: float, y2: float, colour: str = MUTED) -> None:
    canvas.line(x1, y1, x2, y2, colour, width=1.5)
    if abs(y2 - y1) < 1:
        canvas.parts.append(
            f'<polygon points="{x2:.1f},{y2:.1f} {x2 - 8:.1f},{y2 - 5:.1f} {x2 - 8:.1f},{y2 + 5:.1f}" '
            f'fill="{colour}"/>')
    else:
        canvas.parts.append(
            f'<polygon points="{x2:.1f},{y2:.1f} {x2 - 5:.1f},{y2 - 8:.1f} {x2 + 5:.1f},{y2 - 8:.1f}" '
            f'fill="{colour}"/>')


def figure_method(path: Path) -> None:
    """Protocol and decision rule — the only figure in the set that carries no data."""
    s = STRINGS
    zh = s.lang == "zh"
    canvas = Canvas(940, 470 + CAPTION_SPACE - 44, s.method_title, f"{s.method_caption} {s.raw_note}")
    margin, gap = 40, 22
    box_width = (940 - 2 * margin - 3 * gap) / 4
    row_one, row_two, row_height = 110, 214, 56
    stage_one = ([["输入", "固定 + 记录 sha256"], ["1 次预热", "不计时、不进报告"],
                  ["10 次计时重复", "run_index 1..10"], ["每次的 wall_ms", "+ 峰值 RSS"]]
                 if zh else
                 [["input", "fixed + sha256"], ["1 untimed warm-up", "excluded from report"],
                  ["10 timed repeats", "run_index 1..10"], ["per-run wall_ms", "+ peak RSS"]])
    stage_two = ([["10 次的中位数", "IQR、CV"], ["10,000 次 bootstrap", "重采样（种子 20261011）"],
                  ["比值的 95% 区间", "相对 rust"], ["判定", "三段式规则"]]
                 if zh else
                 [["median of the 10", "IQR, CV"], ["10,000 bootstrap", "resamples (seed 20261011)"],
                  ["95% CI of the", "median ratio vs rust"], ["verdict", "three-way rule"]])
    for row_y, stage, colour in ((row_one, stage_one, MUTED), (row_two, stage_two, ACCENT)):
        for index, lines in enumerate(stage):
            x = margin + index * (box_width + gap)
            _box(canvas, x, row_y, box_width, row_height, lines, colour)
            if index:
                _arrow(canvas, x - gap + 3, row_y + row_height / 2, x - 3, row_y + row_height / 2)
    _arrow(canvas, margin + box_width / 2, row_one + row_height,
           margin + box_width / 2, row_two - 3)
    rules = ([(["区间下界 ≥ 1.10"], "material", PYTHON),
              (["区间排除 1.00，", "下界 < 1.10"], "detectable-immaterial", ACCENT),
              (["区间跨过 1.00"], "within-noise", WARN)]
             if zh else
             [(["CI lower bound ≥ 1.10"], "material", PYTHON),
              (["CI excludes 1.00,", "lower bound < 1.10"], "detectable-immaterial", ACCENT),
              (["CI crosses 1.00"], "within-noise", WARN)])
    chip_width = (940 - 2 * margin - 2 * gap) / 3
    for index, (condition, label, colour) in enumerate(rules):
        x = margin + index * (chip_width + gap)
        _box(canvas, x, 306, chip_width, 62, condition, colour)
        canvas.text(x + chip_width / 2, 390, label, size=13, fill=colour, anchor="middle", weight="600")
    canvas.save(path)


def figure_granularity(reports: Path, path: Path) -> None:
    """Record-shape experiment as grouped bars on a truncated linear axis."""
    s = STRINGS
    zh = s.lang == "zh"
    shapes = [("one", ("1 条记录", "(1 × 100 Mb)") if zh else ("1 record", "(1 × 100 Mb)")),
              ("twenty", ("20 条记录", "(20 × 5 Mb)") if zh else ("20 records", "(20 × 5 Mb)")),
              ("many", ("10 万条记录", "(10 万 × 1 kb)") if zh else ("100,000 records", "(100k × 1 kb)"))]
    series: dict[str, list[float | None]] = {"rust": [], "python": [], "r": []}
    for key, _label in shapes:
        candidates = [reports / f"out-gran-{key}" / "sequence.stats.v1.benchmark.json",
                      reports / f"sequence.stats.v1.granularity-{key}.benchmark.json",
                      reports / f"{key}.benchmark.json",
                      reports / f"gran-{key}.benchmark.json"]
        found = next((c for c in candidates if c.is_file()), None)
        if found is None:
            continue
        runs = runs_by_backend(json.loads(found.read_text(encoding="utf-8")))
        for backend in series:
            values = runs.get(backend) or []
            series[backend].append(median_of(values) if values else None)
    values = [v for vals in series.values() for v in vals if v]
    if not values:
        return
    spread = 0.0
    for backend, vals in series.items():
        present = [v for v in vals if v]
        if len(present) > 1:
            spread = max(spread, (max(present) - min(present)) / min(present) * 100)
    if zh:
        caption = (f"{s.granularity_caption_prefix} 三种形态之间每个后端的变化 < {spread + 0.5:.0f}%，"
                   f"说明语言边界穿越次数不是差距来源。{s.raw_note}")
    else:
        caption = (f"{s.granularity_caption_prefix} Within each backend the three shapes stay within "
                   f"{spread + 0.5:.0f}% of one another, so boundary crossings are not what separates the "
                   f"backends. {s.raw_note}")
    low, high = 0.0, max(values) * 1.14
    left, right = 130, 860
    bottom, top = 330, 150
    canvas = Canvas(940, 500 + CAPTION_SPACE - 44, s.granularity_title, caption, s.lower_better)
    legend_row(canvas, 96, [("rust", RUST), ("python", PYTHON), ("r", R_LANG)])
    ticks, _step = ticks_for(low, high, 6)
    for value in ticks:
        y = bottom - lin_scale(value, low, high, 0, bottom - top)
        canvas.line(left, y, right, y, GRID)
        canvas.text(left - 12, y + 4, format_number(value), size=AXIS_SIZE, fill=MUTED, anchor="end")
    canvas.text(24, 120, s.ms_axis, size=AXIS_SIZE, fill=MUTED)
    group_width = (right - left) / len(shapes)
    bar_width = 52
    for index, (_key, labels) in enumerate(shapes):
        centre = left + group_width * (index + 0.5)
        for offset, (backend, colour) in enumerate((("rust", RUST), ("python", PYTHON), ("r", R_LANG))):
            value = series[backend][index] if index < len(series[backend]) else None
            if not value:
                continue
            x = centre - (bar_width * 3 + 18 * 2) / 2 + offset * (bar_width + 18)
            bar_height = lin_scale(value, low, high, 0, bottom - top)
            canvas.rect(x, bottom - bar_height, bar_width, bar_height, colour, radius=2)
            canvas.text(x + bar_width / 2, bottom - bar_height - 9, format_number(value),
                        size=VALUE_SIZE, fill=colour, anchor="middle", weight="600")
        for line_index, text in enumerate(labels):
            canvas.text(centre, bottom + 30 + line_index * 18, text, size=13, fill=INK, anchor="middle")
    canvas.save(path)


def figure_stability(reports: Path, summary: dict, path: Path) -> None:
    """Raw repeats per row, each row on its own truncated linear scale."""
    s = STRINGS
    picks = [e for e in summary["entries"]
             if all(e["backends"].get(b) for b in ("rust", "python", "r"))]
    rows = []
    for entry in picks:
        report_file = reports / entry["report"]
        if report_file.is_file():
            rows.append((entry["capability"].replace(".v1", ""),
                         runs_by_backend(json.loads(report_file.read_text(encoding="utf-8")))))
    if not rows:
        return
    group = 66
    legend = 96
    left, right = 300, 800
    canvas = Canvas(940, 200 + group * len(rows) + CAPTION_SPACE, s.stability_title,
                    f"{s.stability_caption} {s.raw_note}", s.lower_better)
    legend_row(canvas, legend, [("rust", RUST), ("python", PYTHON), ("r", R_LANG)])
    for index, (name, times) in enumerate(rows):
        y = legend + 46 + index * group
        canvas.text(left - 16, y + 34, name, size=14, fill=INK, anchor="end")
        row_values = [v for vals in times.values() for v in vals]
        row_max = max(row_values) * 1.08
        for offset, (backend, colour) in enumerate((("rust", RUST), ("python", PYTHON), ("r", R_LANG))):
            values_ = times.get(backend)
            if not values_:
                continue
            for value in values_:
                canvas.circle(lin_scale(value, 0.0, row_max, left, right), y + offset * 16 + 12, 5,
                              colour, opacity=0.85)
            canvas.line(lin_scale(median_of(values_), 0.0, row_max, left, right), y + offset * 16 + 4,
                        lin_scale(median_of(values_), 0.0, row_max, left, right), y + offset * 16 + 20,
                        colour, width=2.5)
        canvas.text(right + 12, y + 34, f"0 – {format_number(row_max)} {s.unit_ms}",
                    size=AXIS_SIZE, fill=MUTED)
    canvas.save(path)


def figure_series(comparison: dict, summary: dict, path: Path) -> None:
    """Same capabilities in the September fixture window and here (linear axis)."""
    s = STRINGS
    old = {e["capability"]: e for e in comparison["entries"]}
    rows = []
    for entry in summary["entries"]:
        earlier = old.get(entry["capability"])
        if not earlier:
            continue
        current_base = (entry["backends"].get("rust") or {}).get("median_wall_ms")
        old_base = (earlier["backends"].get("rust") or {}).get("median_wall_ms")
        if not current_base or not old_base:
            continue
        for backend in ("python", "r"):
            now = (entry["backends"].get(backend) or {}).get("median_wall_ms")
            then = (earlier["backends"].get(backend) or {}).get("median_wall_ms")
            if not now or not then:
                continue
            rows.append((f"{entry['capability'].replace('.v1', '')} · {backend}",
                         then / old_base, now / current_base))
    if not rows:
        return
    row_height = 50
    legend = 96
    canvas = Canvas(940, 210 + row_height * len(rows) + CAPTION_SPACE, s.series_title,
                    f"{s.series_caption} {s.raw_note}", s.higher_better)
    legend_row(canvas, legend, [(s.legend_small, RUST), (s.legend_large, ACCENT)], gap=200)
    left, right = 330, 780
    low = 1.0
    high = max(max(row[1], row[2]) for row in rows) * 1.06
    ticks, _step = ticks_for(low, high, 6)
    for value in ticks:
        x = lin_scale(value, low, high, left, right)
        canvas.line(x, legend + 40, x, legend + 40 + row_height * len(rows), GRID)
        canvas.text(x, legend + 32, f"{format_number(value)}×", size=AXIS_SIZE, fill=MUTED, anchor="middle")
    for index, (label, small, large) in enumerate(rows):
        y = legend + 40 + index * row_height
        canvas.text(left - 16, y + 32, label, size=13, fill=INK, anchor="end")
        for offset, (ratio, colour, token) in enumerate(((small, RUST, "小夹具" if s.lang == "zh" else "small"),
                                                         (large, ACCENT, "大输入" if s.lang == "zh" else "large"))):
            x = lin_scale(ratio, low, high, left, right)
            canvas.rect(left, y + offset * 18 + 4, x - left, 14, colour, radius=2)
            canvas.text(x + 8, y + offset * 18 + 16, f"{token} {ratio:.2f}×", size=VALUE_SIZE,
                        fill=colour, weight="600")
    canvas.save(path)


def main() -> None:
    global STRINGS
    parser = argparse.ArgumentParser()
    parser.add_argument("--summary", required=True)
    parser.add_argument("--reports", required=True, help="directory with the raw per-capability reports")
    parser.add_argument("--comparison", help="an earlier summary.json to compare against")
    parser.add_argument("--granularity", help="directory with the granularity experiment reports")
    parser.add_argument("--lang", default="en", choices=["en", "zh"])
    parser.add_argument("--output", required=True)
    arguments = parser.parse_args()

    STRINGS = Strings(arguments.lang)
    summary = json.loads(Path(arguments.summary).read_text(encoding="utf-8"))
    reports = Path(arguments.reports)
    output = Path(arguments.output)
    output.mkdir(parents=True, exist_ok=True)

    figure_speedup(summary, output / "fig-speedup-layered.svg")
    figure_method(output / "fig-method.svg")
    figure_medians(summary, output / "fig-absolute-medians.svg")
    figure_memory(summary, output / "fig-memory.svg")
    figure_ratio_ci(summary, reports, output / "fig-ratio-ci.svg")
    figure_stability(reports, summary, output / "fig-repeat-stability.svg")
    if arguments.comparison:
        figure_series(json.loads(Path(arguments.comparison).read_text(encoding="utf-8")),
                      summary, output / "fig-series-two-windows.svg")
    if arguments.granularity:
        figure_granularity(Path(arguments.granularity), output / "fig-granularity.svg")
    print(f"[{arguments.lang}] wrote figures to {output}: "
          + ", ".join(sorted(p.name for p in output.glob("*.svg"))))


if __name__ == "__main__":
    main()
