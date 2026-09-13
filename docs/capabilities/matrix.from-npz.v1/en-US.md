# matrix.from-npz.v1

Import the main 2-D matrix of a NumPy `.npz` archive (produced by
`np.savez` or `np.savez_compressed`) into a plain CSV/TSV table with row
and column labels. The engine reads the ZIP container and each `.npy` v1.0
member natively in Rust — no Python runtime is involved — which unlocks the
delimited-matrix capabilities (differential expression, normalization, PCA,
clustering) for count matrices that only exist as npz artifacts.

## Purpose

Turn npz-only expression/count matrices into the delimited table format the
rest of the SDK consumes, preserving labels when the archive carries them
and generating positional labels (with a warning) when it does not.

## Inputs

- One `.npz` archive: a ZIP of `.npy` members written by `np.savez`
  (ZIP_STORED) or `np.savez_compressed` (DEFLATE).
- The main matrix must be a 2-D array with dtype `<f8`, `<f4`, `<i8`, or
  `<i4`; it is picked automatically when named `counts` or `matrix`, or
  when it is the only 2-D entry in the archive.
- Optional 1-D label arrays: `rows` or `row_labels` for row labels, and
  `cols`, `col_labels`, or `genes` for column labels. Label dtypes may be
  the numeric ones above, byte strings (`|S`), or unicode (`<U`). Array
  names are matched after stripping the `.npy` suffix.

## Parameters

- `--matrix-name NAME`: import this 2-D array instead of the automatic
  selection (fails with the available array list when absent).
- `--row-labels NAME`: use this 1-D array as row labels.
- `--col-labels NAME`: use this 1-D array as column labels.
- `--json`: emit the standard result envelope.
- The output delimiter follows the output extension: `.csv` comma, `.tsv`
  tab; any other extension is rejected.

## Outputs

- `--output counts.csv|counts.tsv`: the matrix table. The header is
  `row` followed by the column labels; each data row is the row label
  followed by the row's values in shortest round-trip form.
- Summary fields: input_arrays (name, shape, dtype of every member),
  matrix_array, rows, cols, cell_count, output_path, output_bytes,
  warnings.

## Examples

```bash
linxira-bio matrix from-npz counts.npz counts.tsv --json
linxira-bio matrix from-npz scanpy_export.npz matrix.csv --matrix-name X --row-labels genes --col-labels barcodes
```

## Interpretation

- `matrix_array` names the entry that was imported; cross-check it and
  `input_arrays` against the exporting script to confirm the intended
  matrix was selected.
- Missing or length-mismatched label arrays fall back to positional
  `row0..rowN-1` / `col0..colM-1` labels and are always disclosed as
  warnings — never silently.
- Fortran-ordered (`fortran_order: True`) members are transposed to C
  (row-major) order on import, so rows and columns keep their numpy
  semantics.

## Caveats

- Only npy format 1.x, little-endian dtypes `<f8`/`<f4`/`<i8`/`<i4`
  (plus `|S`/`<U` labels), and ZIP STORED/DEFLATE entries are supported;
  structured dtypes, big-endian payloads, 3-D arrays, ZIP64 archives, and
  non-npy members are rejected with an explicit error.
- Values are written through `f64`; `i64` counts beyond 2^53 lose exact
  integer precision, and `<f4` values may print with f64-expanding
  decimals.
- Duplicate or empty labels are reported as warnings (empties are replaced
  by positional labels) because downstream matrix consumers reject them.
- This is a format import, not a QC step; run `expression matrix-qc` on the
  emitted table before differential analysis.

## Runtime Dependencies

- None beyond the SDK binary; the ZIP and npy parsing are implemented in
  Rust (csv, flate2) with no Python, numpy, or shell involvement.

## Citations

- Harris, C. R. et al. (2020). Array programming with NumPy. Nature, 585,
  357–362.
- Collette, A. (2013). Python and HDF5 / NumPy binary format (`.npy`
  version 1.0 specification). NumPy documentation.

## Troubleshooting

- `no 2-D matrix array found` / `multiple 2-D arrays found`: name the
  matrix entry `counts` or `matrix`, or pass `--matrix-name`.
- `unsupported dtype`: convert the array in numpy first (for example
  `X.astype('<f8')`) and re-export; structured or big-endian dtypes are
  not importable.
- Label warnings mentioning length mismatch: the label array does not span
  the matrix axis (common when `genes` labels rows of a genes-by-samples
  matrix but is matched against columns); re-export with explicit names or
  pass `--row-labels`/`--col-labels`.
- `output must end in .csv or .tsv`: rename the output file; the delimiter
  is chosen from the extension.
