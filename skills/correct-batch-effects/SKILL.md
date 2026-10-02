---
name: correct-batch-effects
description: Remove batch effects from expression matrices with ComBat parametric empirical Bayes (Johnson 2007) while preserving biological covariates. Use before PCA, clustering, differential expression, or cross-batch comparisons when samples were processed in multiple batches, labs, or library-prep lots. Supports python (NumPy) and r (base R) backends with byte-identical output.
---

# Correct Batch Effects

Adjust a features × samples expression matrix so that samples cluster by
biology instead of processing batch, using the parametric ComBat algorithm
with an optional covariate model.

## Run

1. Inspect the matrix with `linxira-bio dataset inspect <matrix.csv> --json`;
   confirm the detected format and orientation (features as rows, samples as
   columns). The matrix must be normalized or log-transformed — ComBat on raw
   counts is statistically invalid.
2. Prepare a sample table whose first column holds sample ids exactly matching
   the matrix columns, plus a `batch` column and optional numeric or two-level
   categorical covariates (e.g. condition, library size) to preserve.
3. Run `linxira-bio expression batch-correct <matrix.csv|tsv> <output.tsv>
   --sample-table <samples.csv|tsv> [--batch-column batch] [--method combat]
   [--backend auto|python|r] --json`.
4. Preserve the capability version, input hashes, command, and result summary
   (including the reported empirical-Bayes priors).

Backends: `python` (NumPy, default for `auto`), `r` (base R + jsonlite). Both
implement the identical specification and emit byte-identical corrected
matrices; the cross-backend diff is part of CI.

## Validate And Interpret

- Batch means must align after correction: recompute per-batch means on the
  corrected matrix; a residual spread above ~0.1 on log-scale data signals a
  batch-by-covariate confound the model could not separate.
- The biological covariate (e.g. condition) must survive: per-feature
  case/control gaps should be unchanged within noise. If the group effect
  collapses, the covariate was omitted or is collinear with batch — the pack
  rejects rank-deficient designs outright.
- Each batch needs at least two samples; one-sample batches are rejected
  rather than silently passed through.
- Report the number of features, samples, batch counts, and the covariate
  columns preserved alongside the corrected matrix.

Stop on zero-variance features (a feature constant across samples cannot be
standardized), on missing values, on sample-id mismatches between the matrix
and the sample table, and on non-`GO`-style input errors. Do not feed raw
counts; do not interpret corrected values as counts.

## Notes

- The corrected matrix is a TSV in the same orientation and with the same
  feature/sample ids as the input; downstream PCA/clustering consume it
  directly (`expression pca`, `expression cluster`).
- The python backend requires `LINXIRA_BIO_WORKFLOW_PYTHON` pointing at an
  environment with the pinned NumPy; the r backend requires
  `LINXIRA_BIO_WORKFLOW_R_LIBRARY` with jsonlite.
- Algorithm: Johnson, Li & Rabinovic, Biostatistics 8(1), 2007,
  doi:10.1093/biostatistics/kxj037 (parametric posterior-mode variant).
