---
name: render-bio-plots
description: Render deterministic, fully parameterized scientific figures (scatter, line, bar, box, violin, heatmap) from a PlotSpec JSON via native matplotlib or ggplot2 packs. Use when an agent needs a publication figure with explicit axes, theme, palette, size, dpi, format, and caller-chosen output filename, or must prove two plotting backends show identical data. Not for interactive structure viewing or AI image generation.
---

# Render Bio Plots

## When to Use

- A figure is a **data product**: every knob (title, labels, axis ranges,
  log scales, grid, theme, palette, legend, figure size/dpi, font, output
  format, output filename) is an explicit parameter, and identical specs
  render to identical bytes. This is the opposite of asking an image model
  to "draw a chart".
- Use `plot.render.v1` with the workflow packs below; do not write ad hoc
  matplotlib/ggplot2 scripts for one-off figures.

## Steps

1. Assemble a PlotSpec per `schemas/plot-spec.schema.json`. The `data`
   payload uses one of six kinds: `scatter`/`line`/`bar` (with `series`),
   `box`/`violin` (with `groups`), `heatmap` (with `matrix`). Include
   `x_label_hint`/`y_label_hint` derived from the input file name when the
   caller has not set labels.
2. Write a request JSON `{"plot_spec": {...}, "output_path": "<filename>"}`
   — the output filename is caller-composed; the extension is replaced by
   the requested format (`svg`/`png`/`pdf`).
3. Run a pack renderer:
   - python: `workflows/org.linxira.visualization-matplotlib/src/render.py --request REQ --result RES`
   - r: `workflows/org.linxira.visualization-ggplot2/src/render.R --request REQ --result RES`
4. Read the render result: `plot_spec_sha256` (provenance), `artifact`
   (path/format/size/bytes), `data_summary` (point counts and coordinate
   ranges). When both backends ran, their `data_summary` must be equal.

## Contract Notes

- Determinism: svg and png are byte-stable across reruns (fixed svg hash
  salt; suppressed date metadata). ggplot2 pdf embeds a creation date and
  discloses that in `warnings`.
- Errors are structural: unsupported kinds/formats return
  `status: "error"` with a message, never a partial figure.
- Palettes map per backend (`set2` → matplotlib Set2 / RColorBrewer Set2);
  unknown names fall back to Set2 with a warning in the result.
