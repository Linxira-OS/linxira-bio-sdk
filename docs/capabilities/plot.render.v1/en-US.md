# plot.render.v1

Render a deterministic, fully parameterized scientific figure from a PlotSpec
JSON via a native plotting pack (matplotlib or ggplot2). Plotting here is a
data product, not an image-generation task: every visual knob is an explicit,
versioned parameter, and the same spec renders to the same bytes every run.

## Purpose

Turn analysis results into publication figures whose every property (axes,
theme, palette, size, dpi, format, output filename) is an explicit parameter
with a stable contract, so figures are reproducible, diffable, and provably
data-consistent across two independent plotting backends. This is the
opposite of asking an image model to "draw a chart": no parameters are
hidden, no bytes are arbitrary.

## Inputs

- A request JSON: `{"plot_spec": {...}, "output_path": "<filename>"}`.
- `plot_spec` must satisfy `schemas/plot-spec.schema.json`. Supported data
  kinds: `scatter`, `line`, `bar` (field `series` with `x`/`y` per series),
  `box`, `violin` (field `groups` with `values`), and `heatmap` (field
  `matrix` with `row_labels`/`col_labels`/`values`).

## Parameters

Every field below is honored by both backends (defaults per the PlotSpec
schema):

| Field | Default | Notes |
|---|---|---|
| `title` / `subtitle` | empty | rendered above the axes |
| `x_label` / `y_label` | from `x_label_hint`/`y_label_hint`, else `x`/`y` | caller or input-file-derived |
| `x_range` / `y_range` | auto | two-number [min, max] |
| `x_log` / `y_log` | false | log-scaled axes |
| `grid` | true | |
| `theme` | `light` | `dark`, `light`, `publication` |
| `palette` | `set2` | mapped per backend; unknown names fall back with a warning |
| `legend` | true | |
| `figure.width/height/dpi` | 800 / 600 / 150 | |
| `font.family` / `font.size` | platform default / 12 | |
| `output.format` | `svg` | `svg`, `png`, `pdf`; the artifact extension follows it |
| `output_path` (request) | `figure.svg` | caller-composed output filename |

## Outputs

- The figure file named by `output_path` (extension replaced by the
  requested format).
- A render-result JSON (`schemas/plot-render-result.schema.json`): `backend`,
  `plot_spec_sha256` (provenance), `artifact` (path, format, width, height,
  dpi, bytes), `data_summary` (kind, series count, per-series point counts,
  x/y ranges), `warnings`.

## Examples

```bash
linxira-bio workflow run org.linxira.visualization-matplotlib --request request.json --json
```

```bash
python workflows/org.linxira.visualization-matplotlib/src/render.py \
  --request request.json --result result.json
Rscript workflows/org.linxira.visualization-ggplot2/src/render.R \
  --request request.json --result result.json
```

Request example:

```json
{"plot_spec": {"title": "PCA scores", "x_label": "PC1 (38%)",
  "y_label": "PC2 (21%)", "theme": "publication", "output": {"format": "png"},
  "data": {"kind": "scatter", "series": [
    {"name": "control", "x": [1, 2, 3], "y": [2, 1.5, 3.5]},
    {"name": "treated", "x": [3.5, 4, 5], "y": [4, 5.5, 6]}]}},
 "output_path": "pca-scores.png"}
```

## Interpretation

- `data_summary` describes what the figure actually plots: series count,
  per-series point counts, and coordinate ranges. When both backends render
  the same spec, these must be equal — that is the cross-backend data
  consistency guarantee (asserted in both packs' tests); pixel-level identity
  is not claimed.
- `plot_spec_sha256` is per-backend provenance for rerun stability checks;
  the two languages canonicalize JSON differently, so it is not expected to
  match across backends.

## Caveats

- svg and png are byte-stable across reruns: matplotlib pins the svg hash
  salt and suppresses date metadata; ggplot2 renders svg via svglite and png
  via cairo. ggplot2 `pdf` embeds a creation date, disclosed in `warnings`.
- Six chart kinds in v1; the dedicated Rust SVG capabilities cover
  annotation/enrichment/domain families, and the interactive structure
  viewer covers molecules.
- `interactive` output is accepted by the PlotSpec schema but not yet
  honored by the packs.

## Runtime Dependencies

- matplotlib pack: CPython 3.12+ with matplotlib >= 3.8 (pinned in the pack
  `requirements.lock`).
- ggplot2 pack: R >= 4.4 with ggplot2, svglite, jsonlite, digest (pinned in
  the pack `dependencies.lock.json`).

## Citations

- Hunter, J. D. (2007). Matplotlib: A 2D graphics environment. Computing in
  Science & Engineering, 9(3), 90–95.
- Wickham, H. (2016). ggplot2: Elegant Graphics for Data Analysis.
  Springer-Verlag New York.

## Troubleshooting

- `status: "error"` with `unsupported plot kind`: the `data.kind` value is
  not one of the six supported kinds; fix the spec.
- Unknown palette warning: the palette name is not mapped in this backend;
  it fell back to Set2 — set a supported name to remove the warning.
- R renderer fails on missing packages: install per the pack
  `dependencies.lock.json` (ggplot2, svglite, jsonlite, digest).
