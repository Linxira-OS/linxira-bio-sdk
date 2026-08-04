# Differential Expression Volcano Plot

## Purpose

Render a local SVG volcano plot from a differential-expression result table.

## Inputs

CSV containing finite `log2FoldChange` and `padj` columns.

## Parameters

`--padj`, `--log2-fold-change`, and `--max-points` set the significance, effect-size, and rendering limits.

## Examples

```text
linxira-bio expression volcano differential.csv volcano.svg --json
```

## Outputs

An SVG plot. Red and blue points meet the configured adjusted-p-value and fold-change thresholds.

## Interpretation

Points indicate effect size and adjusted significance; they do not establish biological causality.

## Caveats

The plot does not perform differential-expression statistics or replace experimental design review.

## Runtime Dependencies

Local Rust only. Statistical estimation remains the responsibility of the upstream differential-expression workflow.

## Citations

Cite the differential-expression method that produced the input table.

## Troubleshooting

Export CSV with the exact required column names and remove non-finite values.
