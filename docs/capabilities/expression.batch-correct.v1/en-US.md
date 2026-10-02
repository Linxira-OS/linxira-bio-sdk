# Batch Effect Correction

## Purpose

Remove technical batch effects (processing date, lab, library-prep lot) from
expression matrices with ComBat parametric empirical Bayes while preserving
biological covariates, so that downstream PCA, clustering, and differential
comparisons reflect biology instead of batch structure.

## Inputs

Two files. A features × samples expression matrix (CSV/TSV, first column =
feature id) that is normalized or log-transformed — ComBat on raw counts is
statistically invalid. A sample table (CSV/TSV, first column = sample id)
whose ids match the matrix columns exactly, with a batch column and optional
numeric or two-level categorical covariates to preserve.

## Parameters

`--sample-table` (required) points at the sample table. `--batch-column`
selects the batch column (default `batch`). `--method combat` is the only
method (parametric empirical Bayes). `--backend auto|python|r` selects the
implementation: NumPy (default for `auto`) or base R. Both implement the
identical specification and emit byte-identical corrected matrices.

## Outputs

A corrected matrix TSV in the same orientation with the same feature and
sample ids, plus a JSON summary: method, feature/sample counts, batch counts,
covariate columns preserved, the maximum empirical-Bayes iteration count, and
the per-batch priors (gamma bar, tau squared, a, b).

## Examples

```bash
linxira-bio expression batch-correct expr.csv expr-corrected.tsv \
  --sample-table samples.csv --batch-column batch --json
linxira-bio expression batch-correct expr.csv expr-corrected.tsv \
  --sample-table samples.csv --backend r --json
```

## Interpretation

After correction, per-batch means align (residual spread ≲ 0.1 on log-scale
data) while per-feature covariate effects are preserved within noise. The
empirical-Bayes priors shrink per-batch, per-feature adjustments toward the
cross-feature distribution, which stabilizes estimates in small batches. A
surviving batch signature usually means a batch-by-covariate confound the
model could not separate; the pack rejects rank-deficient designs outright
instead of returning silently confounded output.

## Caveats

Each batch needs at least two samples. Features with zero residual variance
(constant across samples) are rejected, as are missing values and sample-id
mismatches. The corrected values are not counts; re-run normalization only
before, not after, correction. Covariates collinear with batch are rejected
by the rank check rather than partially absorbed.

## Runtime Dependencies

Python backend: the pinned NumPy environment (`LINXIRA_BIO_WORKFLOW_PYTHON`).
R backend: base R plus jsonlite from the project library
(`LINXIRA_BIO_WORKFLOW_R_LIBRARY`). No network access.

## Citations

Johnson, W. E., Li, C., & Rabinovic, A. (2007). Adjusting batch effects in
microarray expression data using empirical Bayes methods. Biostatistics
8(1), 118–127. doi:10.1093/biostatistics/kxj037

## Troubleshooting

"batch design matrix is rank deficient" means a covariate is collinear with
batch (e.g. every batch processed on one condition only) — either drop the
covariate or collect a balanced design. "degenerate empirical-Bayes prior"
or non-convergence indicates a batch with too few samples for its variance;
pool or drop it. Backend output mismatches are bugs, not tolerances to widen.
