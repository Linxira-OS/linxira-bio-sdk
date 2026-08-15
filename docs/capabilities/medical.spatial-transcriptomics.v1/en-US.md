# Spatial Transcriptomics Summary

## Purpose

Parse a 10x Genomics sparse expression matrix (matrix market format) with its feature and barcode annotations and compute local quality and summary statistics, including per-barcode total counts and detected genes, plus a barcode rank table for the standard "knee plot" style assessment.

## Inputs

Three files from a 10x output directory (optionally gzip-compressed): the matrix (`matrix.mtx`), the feature annotation (`features.tsv` — id, name, feature type), and the barcode annotation (`barcodes.tsv`).

## Parameters

No parameters are required.

## Outputs

A TSV barcode rank table with columns `rank`, `barcode`, `total_counts`, `n_genes`. JSON output additionally reports `format`, `n_barcodes`, `n_features`, `n_nonzero`, `total_counts`, `mean_counts`, `median_genes`, `p90_genes`, and `barcode_rank`.

## Examples

```bash
linxira-bio medical spatial-transcriptomics matrix.mtx features.tsv barcodes.tsv barcode-rank.tsv --json
```

## Interpretation

`total_counts` and `n_genes` per barcode reflect sequencing depth and detection; the rank table sorted by total counts reproduces the standard barcode knee plot data used to separate high-content cell barcodes from background. `median_genes` and `p90_genes` summarize per-barcode detection across all barcodes.

## Caveats

Matrix values are treated as rounded integer counts. The matrix dimensions must match the annotation line counts exactly. This is a summary capability: it does not perform clustering, normalization, cell typing, or spatial coordinate analysis.

## Runtime Dependencies

None — pure local Rust capability (gzip support is built in).

## Citations

10x Genomics sparse matrix format specification (Matrix Market coordinate).

## Troubleshooting

If parsing fails, confirm the matrix header declares `coordinate` format, indices are 1-based, and the feature/barcode annotation line counts equal the declared matrix dimensions. gzip-compressed inputs are detected automatically.
