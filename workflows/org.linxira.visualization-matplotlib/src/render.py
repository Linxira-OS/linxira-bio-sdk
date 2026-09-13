#!/usr/bin/env python3
"""Deterministic matplotlib renderer for Linxira PlotSpec (M1-T1).

Contract: a request JSON {"plot_spec": {...}, "output_path": "<relative>"}
produces the figure named by output_path (caller-composed filename, any of
svg/png/pdf) plus a render-result JSON with the PlotSpec hash, artifact
metadata, and a data summary (point counts / coordinate ranges) so the
ggplot2 backend can be asserted data-consistent without pixel comparison.

Every PlotSpec knob (title, labels, ranges, log axes, grid, theme, palette,
legend, figure size/dpi, font, format) is honored; defaults follow
schemas/plot-spec.schema.json. Output is deterministic: no timestamps, fixed
dpi, so identical specs render to identical bytes.
"""
from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

# SVG clip-path/marker ids get random suffixes unless a salt is pinned; a
# fixed salt is what makes identical specs render to identical bytes.
plt.rcParams["svg.hashsalt"] = "linxira-bio"

SCHEMA_VERSION = "1"
BACKEND = "matplotlib"
KINDS = ("scatter", "line", "bar", "box", "violin", "heatmap")
FORMATS = ("svg", "png", "pdf")
# matplotlib savefig metadata keys that suppress nondeterministic fields.
NO_DATE_METADATA = {
    "svg": {"Date": None, "Creator": None},
    "png": {"Software": None},
    "pdf": {"CreationDate": None, "ModDate": None, "Creator": None},
}


def canonical_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, ensure_ascii=False).encode("utf-8")
    ).hexdigest()


def finite(values: list[float]) -> list[float]:
    return [float(v) for v in values if isinstance(v, (int, float))]


def data_summary(plot_spec: dict) -> dict:
    data = plot_spec["data"]
    kind = data.get("kind", "")
    if kind in ("scatter", "line", "bar"):
        series = data.get("series", [])
        xs = [v for entry in series for v in finite(entry.get("x", []))]
        ys = [v for entry in series for v in finite(entry.get("y", []))]
        return {
            "kind": kind,
            "series_count": len(series),
            "point_counts": [
                min(len(finite(e.get("x", []))), len(finite(e.get("y", []))))
                for e in series
            ],
            "x_range": [min(xs), max(xs)] if xs else None,
            "y_range": [min(ys), max(ys)] if ys else None,
        }
    if kind in ("box", "violin"):
        groups = data.get("groups", [])
        values = [v for group in groups for v in finite(group.get("values", []))]
        return {
            "kind": kind,
            "series_count": len(groups),
            "point_counts": [len(finite(g.get("values", []))) for g in groups],
            "x_range": None,
            "y_range": [min(values), max(values)] if values else None,
        }
    if kind == "heatmap":
        matrix = data["matrix"]
        flat = [v for row in matrix["values"] for v in finite(row)]
        return {
            "kind": kind,
            "series_count": 1,
            "point_counts": [len(matrix["row_labels"]) * len(matrix["col_labels"])],
            "x_range": None,
            "y_range": [min(flat), max(flat)] if flat else None,
        }
    raise ValueError(f"unsupported plot kind {kind!r}; expected one of {KINDS}")


def apply_style(figure, axes, plot_spec: dict, warnings: list[str]) -> None:
    theme = plot_spec.get("theme", "light")
    if theme == "dark":
        figure.patch.set_facecolor("#1c1c1c")
        axes.set_facecolor("#242424")
        axes.tick_params(colors="white")
        for spine in axes.spines.values():
            spine.set_color("white")
        axes.xaxis.label.set_color("white")
        axes.yaxis.label.set_color("white")
        axes.title.set_color("white")
    elif theme == "publication":
        for spine in axes.spines.values():
            spine.set_linewidth(0.8)
        figure.subplots_adjust(left=0.14, right=0.95, top=0.88, bottom=0.12)
    if plot_spec.get("grid", True):
        axes.grid(True, alpha=0.35 if theme != "dark" else 0.25)
    else:
        axes.grid(False)


def palette_colors(plot_spec: dict, count: int, warnings: list[str]):
    name = plot_spec.get("palette", "set2")
    colormap = matplotlib.colormaps.get(name)
    if colormap is None:
        warnings.append(f"unknown palette {name!r}; falling back to Set2")
        colormap = matplotlib.colormaps["Set2"]
    slots = max(count, 1)
    return [colormap(i / max(slots - 1, 1)) for i in range(slots)]


def render(plot_spec: dict, output_path: Path) -> dict:
    warnings: list[str] = []
    data = plot_spec["data"]
    kind = data.get("kind", "")
    summary = data_summary(plot_spec)

    figure_block = plot_spec.get("figure", {}) or {}
    width = int(figure_block.get("width", 800))
    height = int(figure_block.get("height", 600))
    dpi = int(figure_block.get("dpi", 150))
    font_block = plot_spec.get("font", {}) or {}
    font_size = int(font_block.get("size", 12))
    family = font_block.get("family") or None
    if family:
        plt.rcParams["font.family"] = family
    plt.rcParams["font.size"] = font_size

    figure, axes = plt.subplots(figsize=(width / dpi, height / dpi), dpi=dpi)
    if kind in ("scatter", "line"):
        colors = palette_colors(plot_spec, len(data.get("series", [])), warnings)
        for index, entry in enumerate(data.get("series", [])):
            style = {} if kind == "scatter" else {"marker": "o"}
            axes.plot(
                entry.get("x", []),
                entry.get("y", []),
                linestyle="none" if kind == "scatter" else "-",
                color=colors[index],
                label=entry.get("name", f"series {index + 1}"),
                **style,
            )
        if plot_spec.get("legend", True) and data.get("series"):
            axes.legend()
    elif kind == "bar":
        colors = palette_colors(plot_spec, len(data.get("series", [])), warnings)
        for index, entry in enumerate(data.get("series", [])):
            axes.bar(
                [str(x) for x in entry.get("x", [])],
                entry.get("y", []),
                color=colors[index],
                label=entry.get("name", f"series {index + 1}"),
            )
        if plot_spec.get("legend", True) and len(data.get("series", [])) > 1:
            axes.legend()
    elif kind in ("box", "violin"):
        groups = data.get("groups", [])
        colors = palette_colors(plot_spec, len(groups), warnings)
        names = [g.get("name", f"group {i + 1}") for i, g in enumerate(groups)]
        values = [finite(g.get("values", [])) for g in groups]
        if kind == "box":
            axes.boxplot(values, patch_artist=True)
            for patch, color in zip(axes.patches, colors):
                patch.set_facecolor(color)
        else:
            axes.violinplot(values, showmedians=True)
        axes.set_xticks(range(1, len(names) + 1), names)
    elif kind == "heatmap":
        matrix = data["matrix"]
        image = axes.imshow(matrix["values"], aspect="auto", interpolation="nearest")
        figure.colorbar(image, ax=axes)
        axes.set_xticks(range(len(matrix["col_labels"])), matrix["col_labels"])
        axes.set_yticks(range(len(matrix["row_labels"])), matrix["row_labels"])

    title = plot_spec.get("title", "")
    subtitle = plot_spec.get("subtitle", "")
    axes.set_title(
        "\n".join(part for part in (title, subtitle) if part)
        or axes.get_title()
    )
    axes.set_xlabel(
        plot_spec.get("x_label") or data.get("x_label_hint") or "x"
    )
    axes.set_ylabel(plot_spec.get("y_label") or data.get("y_label_hint") or "y")
    if plot_spec.get("x_log"):
        axes.set_xscale("log")
    if plot_spec.get("y_log"):
        axes.set_yscale("log")
    if plot_spec.get("x_range"):
        axes.set_xlim(plot_spec["x_range"])
    if plot_spec.get("y_range"):
        axes.set_ylim(plot_spec["y_range"])
    apply_style(figure, axes, plot_spec, warnings)

    fmt = (plot_spec.get("output", {}) or {}).get("format", "svg")
    if fmt not in FORMATS:
        warnings.append(f"unsupported format {fmt!r}; rendered svg instead")
        fmt = "svg"
    output_path = output_path.with_suffix(f".{fmt}")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    metadata = NO_DATE_METADATA.get(fmt, {})
    figure.savefig(
        output_path,
        format=fmt,
        dpi=dpi,
        metadata=metadata,
    )
    plt.close(figure)
    return {
        "schema_version": SCHEMA_VERSION,
        "backend": BACKEND,
        "plot_spec_sha256": canonical_hash(plot_spec),
        "artifact": {
            "path": output_path.name,
            "format": fmt,
            "width": width,
            "height": height,
            "dpi": dpi,
            "bytes": output_path.stat().st_size,
        },
        "data_summary": summary,
        "warnings": warnings,
    }


def main(argv: list[str]) -> int:
    request_path = result_path = None
    arguments = argv[1:]
    for index, argument in enumerate(arguments):
        if argument == "--request":
            request_path = Path(arguments[index + 1])
        elif argument == "--result":
            result_path = Path(arguments[index + 1])
    if request_path is None or result_path is None:
        print("usage: render.py --request REQUEST --result RESULT", file=sys.stderr)
        return 2
    request = json.loads(request_path.read_text(encoding="utf-8"))
    plot_spec = request["plot_spec"]
    output_path = request_path.parent / request.get("output_path", "figure.svg")
    try:
        result = render(plot_spec, output_path)
    except Exception as error:  # surfaced as harness failure envelope
        result = {
            "schema_version": SCHEMA_VERSION,
            "backend": BACKEND,
            "status": "error",
            "message": f"{type(error).__name__}: {error}",
        }
        result_path.write_text(
            json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
        return 1
    result_path.write_text(
        json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
