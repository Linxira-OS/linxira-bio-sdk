# Expression Matrix Quality Control

## Purpose

Validate a rectangular CSV or TSV expression matrix and summarize dimensions,
missingness, sparsity, feature identifiers, and sample-level values locally.

## Inputs

One CSV or TSV matrix whose first row is a header, first column contains feature
identifiers, and remaining columns contain numeric sample values. Plain text and
gzip streams are detected by magic bytes.

## Parameters

The input path is required. Delimiter detection is automatic. `--json` returns
the standard analysis result envelope. Missing values are empty cells, `.`,
`NA`, or `NaN`.

## Outputs

Returns delimiter, feature identifier column, dimensions, total and numeric
cell counts, missing, zero, negative, and duplicate-feature counts, zero
percentage, per-sample totals, means, detected-feature counts, and warnings.

## Examples

```bash
linxira-bio expression matrix-qc tests/fixtures/expression-matrix/counts.tsv --json
```

## Interpretation

Zeros and missing cells are reported separately. Negative values usually mean
the matrix is transformed rather than raw counts. Per-sample totals and detected
features can reveal scale or quality outliers but do not establish significance.

## Caveats

The capability performs descriptive QC only. It does not normalize counts,
read experimental design metadata, fit statistical models, correct batch
effects, or calculate differential expression.

## Runtime Dependencies

This is a streaming local Rust capability with no Python, R, Java, pandas, or
Bioconductor dependency.

## Citations

Downstream raw-count analysis should follow the assumptions and citation
requirements of the selected method, such as DESeq2, edgeR, or limma-voom.

## Troubleshooting

Ensure the header contains unique sample names, every row has the same number
of columns, and all non-missing sample cells are finite numbers. Resolve
duplicate feature identifiers before model fitting.
