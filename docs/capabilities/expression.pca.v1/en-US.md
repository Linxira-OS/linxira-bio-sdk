# Expression PCA

## Purpose

Run deterministic principal component analysis with samples as observations and
expression features as variables.

## Inputs

A complete local CSV or TSV matrix with unique feature identifiers, at least
two samples, and one non-constant feature.

## Parameters

- `--components N`: number of components to request (default 2).
- `--scale`: also divide non-constant features by their sample standard
  deviation (parameter name `scale_features` on the worker contract).
- `--backend auto|rust|python|r`: the implementation backend. `rust` (and
  omission) runs the native engine; `python` and `r` route through the worker
  to the benchmark packs (`org.linxira.benchmark-python` /
  `org.linxira.benchmark-r`), which re-implement the same deterministic
  eigensolver (largest-magnitude component positive) so results match within
  the 1e-6 consistency tolerance. `auto` consults
  `runtime-preferences.json`; a non-rust hit emits a
  `backend_from_preferences` warning.
- `--json`: print the full result envelope.

## Outputs

JSON contains sample scores, eigenvalues, explained-variance percentages, and
the strongest positive and negative feature loadings for each component.

## Examples

```bash
linxira-bio expression pca matrix.tsv --components 2 --scale --json
```

## Interpretation

Use score plots to inspect major sample variation and loadings to identify
features contributing to each resolved axis.

## Caveats

PCA is exploratory and does not establish biological groups or significance.
Missing, duplicate-feature, and non-finite values are rejected. Local numeric
analysis is capped at 10 million matrix cells.

## Runtime Dependencies

- `rust` (default): the centered covariance operator and eigensolver are
  implemented in local Rust.
- `python` / `r`: the matching benchmark pack — Python needs `numpy`, R needs
  no extra packages (base `sin`/`cos` power iteration). No network access.

## Citations

Cite PCA and any upstream normalization method used to create the analyzed matrix.

## Troubleshooting

- Remove constant features if all requested components cannot be resolved. Scale
  features when their numeric ranges are not directly comparable.
- `unknown --backend value`: the flag accepts `auto`, `rust`, `python`, and
  `r` only.
