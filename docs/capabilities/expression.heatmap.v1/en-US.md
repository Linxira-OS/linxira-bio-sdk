# Clustered Expression Heatmap

## Purpose

Prepare an ordered expression heatmap payload for the native Rust desktop renderer.

## Inputs

A complete local CSV or TSV expression matrix with unique feature identifiers and finite values.

## Parameters

Use `--top-features N` to select up to 200 features by sample variance. Row
z-scores are enabled by default; use `--no-scale` to preserve input values.

## Outputs

JSON contains ordered row and column labels, the finite display matrix, value
range, dimensions, and warnings. The GUI renders this without a browser runtime.

## Examples

```bash
linxira-bio expression heatmap matrix.tsv --top-features 50 --json
```

## Interpretation

Inspect coordinated relative patterns after confirming the selected features,
sample labels, transformation, and clustering assumptions.

## Caveats

Feature selection and clustering are exploratory and do not perform differential
expression. More than 200 samples use deterministic projection ordering. Local
numeric analysis is capped at 10 million matrix cells.

## Runtime Dependencies

Variance selection, row scaling, average-linkage ordering, and rendering are local Rust.

## Citations

Cite hierarchical clustering and the normalization used to create the matrix.

## Troubleshooting

Lower `--top-features` for a clearer display. Disable row scaling only when the
input value magnitudes are intended to remain directly comparable.
