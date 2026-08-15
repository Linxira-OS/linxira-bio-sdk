---
name: analyze-spatial-transcriptomics
description: Summarize 10x Genomics sparse expression matrices with per-barcode total counts and detected genes, plus barcode rank tables for quality assessment. Use for research-only spot/barcode-level QC of spatial transcriptomics or single-cell count matrices.
---

# Analyze Spatial Transcriptomics

Inspect imported files before execution. Use the Rust capability; do not
reimplement matrix-market parsing or count aggregation in Python or R.

## Choose a capability

- Use `medical.spatial-transcriptomics.v1` with the matrix (`matrix.mtx`),
  feature annotation (`features.tsv`), and barcode annotation
  (`barcodes.tsv`) to produce per-barcode totals, detected-gene summaries,
  and a barcode rank table.

## Execute

```bash
linxira-bio medical spatial-transcriptomics MATRIX.mtx FEATURES.tsv BARCODES.tsv RANK.tsv --json
```

## Interpret

Report `total_counts` and `n_genes` per barcode and the barcode rank table for
knee-plot style QC. `median_genes`/`p90_genes` summarize detection. This
capability does not cluster, normalize, or assign cell types; keep
interpretation at the quality-summary level. Keep controlled-access clinical
spatial data local.

## Caveats

Matrix dimensions must match annotation line counts; values are treated as
rounded integer counts. gzip-compressed inputs are auto-detected.
