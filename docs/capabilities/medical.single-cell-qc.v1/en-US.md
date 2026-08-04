# Single-Cell Count Matrix QC

## Purpose

Summarize a local cell-by-gene count matrix for research: per-cell totals, detected genes, zeros, missing values, and duplicate feature identifiers. This is not diagnosis or clinical decision support.

## Inputs

CSV or TSV counts with feature identifiers in the first column and one cell per remaining column.

## Parameters

The input path is required; delimiter detection is automatic.

## Outputs

Cell/sample totals, detected-feature counts, missingness, zero rate, negative-value count, and warnings.

## Examples

```text
linxira-bio medical single-cell-qc counts.tsv --json
```

## Interpretation

Library totals and detected genes help identify data-quality variation; they do not establish cell identity or clinical meaning.

## Caveats

This initial local QC does not calculate mitochondrial percentages, doublet scores, normalization, or clustering.

## Runtime Dependencies

Local streaming Rust only.

## Citations

Cite the single-cell protocol and preprocessing method.

## Troubleshooting

Use a rectangular numeric count matrix with unique cell and feature identifiers.
