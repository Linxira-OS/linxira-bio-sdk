---
name: import-npz-matrix
description: Import the main 2-D matrix of a NumPy .npz archive (np.savez or np.savez_compressed) into a labeled CSV/TSV table with native Rust ZIP/npy parsing and no Python runtime. Use when an agent must turn an npz-only count or expression matrix into the delimited table consumed by differential-expression, normalization, PCA, or clustering capabilities. Not for writing npz, structured dtypes, or 3-D arrays.
---

# Import NPZ Matrix

## Steps

1. Inspect the archive members before importing when the layout is
   unknown; the import summary lists every array (name, shape, dtype).

```bash
linxira-bio matrix from-npz counts.npz counts.tsv --json
```

2. The 2-D array named `counts` or `matrix` is imported automatically, or
   the unique 2-D entry. `rows`/`row_labels` become row labels and
   `cols`/`col_labels`/`genes` become column labels; missing or
   length-mismatched labels fall back to positional `row{i}`/`col{j}`
   labels with a warning.
3. Override the automatic selection when the archive uses other names:

```bash
linxira-bio matrix from-npz export.npz matrix.csv \
  --matrix-name X --row-labels genes --col-labels barcodes --json
```

4. Review the warnings, then continue with `expression matrix-qc` or the
   differential-expression workflow on the emitted table.

## Contract Notes

- Only npy 1.x with little-endian `<f8`/`<f4`/`<i8`/`<i4` dtypes (labels
  also `|S`/`<U`) and ZIP STORED/DEFLATE entries are supported; ZIP64,
  big-endian, structured dtypes, and 3-D arrays are rejected explicitly.
- The delimiter follows the output extension (`.csv` comma, `.tsv` tab);
  any other extension fails with a usage error.
- The JSON summary carries input_arrays, matrix_array, rows, cols,
  cell_count, output_path, output_bytes, and warnings.
