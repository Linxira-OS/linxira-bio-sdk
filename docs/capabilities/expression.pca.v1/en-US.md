# Expression PCA

## Purpose

Run deterministic principal component analysis with samples as observations and
expression features as variables.

## Inputs

A complete local CSV or TSV matrix with unique feature identifiers, at least
two samples, and one non-constant feature.

## Parameters

Use `--components N` to request components. Features are centered; `--scale`
also divides non-constant features by sample standard deviation.

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

The centered covariance operator and eigensolver are implemented in local Rust.

## Citations

Cite PCA and any upstream normalization method used to create the analyzed matrix.

## Troubleshooting

Remove constant features if all requested components cannot be resolved. Scale
features when their numeric ranges are not directly comparable.
