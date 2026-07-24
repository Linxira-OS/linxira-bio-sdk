---
name: analyze-expression-matrix
description: Validate and summarize local rectangular CSV or TSV expression matrices with the executable expression.matrix.qc.v1 capability. Use for dimensions, missingness, sparsity, duplicate features, negative values, per-sample totals, means, and detected-feature counts before downstream bulk-expression analysis.
---

# Analyze Expression Matrix

Run dependency-free matrix QC locally before selecting an R, Python, or native
downstream expression workflow.

## Run

1. Inspect the input with `linxira-bio dataset inspect <matrix.csv|tsv> --json`.
2. Confirm the first column contains feature identifiers and all remaining
   columns represent samples.
3. Run `linxira-bio expression matrix-qc <matrix.csv|tsv> --json`.
4. Preserve the capability version, input hash, delimiter, warnings, and result.

For an artifact-aware agent job, invoke `expression.matrix.qc.v1` with one
input whose role is `matrix`, format is `csv` or `tsv`, and execution mode is
`local-cpu`.

## Validate And Interpret

- Check feature and sample counts against the experimental design.
- Report missing and zero values separately; zeros can be biological or caused
  by limited detection, while missing values indicate absent cells.
- Treat duplicate feature identifiers as an ambiguity requiring aggregation or
  disambiguation before model fitting.
- Treat negative values as evidence of a transformed matrix; do not pass them
  to raw-count methods such as DESeq2.
- Compare per-sample totals and detected-feature counts for library-size or
  sample-quality outliers without assigning significance thresholds silently.

This capability performs QC only. Use a locked and validated R workflow for
differential expression and preserve the design formula, contrasts, package
versions, normalization, and multiple-testing method.
