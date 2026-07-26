---
name: analyze-expression-matrix
description: Validate, normalize, reduce, cluster, and prepare native heatmaps for local rectangular CSV or TSV bulk-expression matrices with expression.matrix.qc.v1, expression.normalize.v1, expression.pca.v1, expression.cluster.v1, and expression.heatmap.v1. Use for matrix QC, CPM or median-ratio normalization, exploratory PCA, deterministic sample or feature clustering, and clustered heatmap preparation.
---

# Analyze Expression Matrix

Run deterministic Rust matrix analysis locally before selecting a statistical
differential-expression workflow.

## Run

1. Inspect the input with `linxira-bio dataset inspect <matrix.csv|tsv> --json`.
2. Confirm the first column contains feature identifiers and all remaining
   columns represent samples.
3. Run `linxira-bio expression matrix-qc <matrix.csv|tsv> --json`.
4. Reject or resolve missing values and duplicate feature identifiers before
   using downstream capabilities.
5. Select one operation:
   - normalize: `linxira-bio expression normalize <matrix> <output.tsv>
     --method cpm|log2-cpm|median-ratio --json`;
   - PCA: `linxira-bio expression pca <matrix> --components 2 [--scale]
     --json`;
   - clustering: `linxira-bio expression cluster <matrix>
     --sample-clusters N --feature-clusters N --json`;
   - heatmap: `linxira-bio expression heatmap <matrix> --top-features N
     --json`.
6. Preserve the capability version, input hash, options, warnings, and result.

For an artifact-aware agent job, invoke the selected capability with one input
whose role is `matrix`, format is `csv` or `tsv`, and execution mode is
`local-cpu`. Normalization also requires `parameters.output` and emits a TSV
artifact.

## Validate And Interpret

- Check feature and sample counts against the experimental design.
- Report missing and zero values separately; zeros can be biological or caused
  by limited detection, while missing values indicate absent cells.
- Treat duplicate feature identifiers as an ambiguity requiring aggregation or
  disambiguation before model fitting.
- Treat negative values as evidence of a transformed matrix; normalization
  requires non-negative input.
- Compare per-sample totals and detected-feature counts for library-size or
  sample-quality outliers without assigning significance thresholds silently.
- Use CPM only for library-size adjustment. Use log2-CPM for exploration and
  retain raw counts for count-based models.
- Treat PCA, clustering, and heatmaps as exploratory. Do not infer biological
  groups or statistical significance from them alone.

Differential expression remains separate. Use a locked and validated workflow
and preserve the design formula, contrasts, package versions, normalization,
and multiple-testing method.
