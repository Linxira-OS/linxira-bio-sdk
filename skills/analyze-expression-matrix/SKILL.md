---
name: analyze-expression-matrix
description: Validate, normalize, reduce, cluster, prepare native heatmaps, and run WGCNA co-expression network analysis for local rectangular CSV or TSV bulk-expression matrices with expression.matrix.qc.v1, expression.normalize.v1, expression.pca.v1, expression.cluster.v1, expression.heatmap.v1, and expression.wgcna.v1. Use for matrix QC, CPM or median-ratio normalization, exploratory PCA, deterministic sample or feature clustering, clustered heatmap preparation, and weighted gene co-expression network analysis.
---

# Analyze Expression Matrix

Run deterministic Rust matrix analysis locally before selecting a statistical
differential-expression workflow or co-expression network analysis.

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
     --json`;
   - WGCNA: `linxira-bio expression wgcna <matrix.csv|tsv> <output.json>
     --min-module-size 30 --merge-cut-height 0.25 --network-type signed
     --threads 4 --json`.
6. Preserve the capability version, input hash, options, warnings, and result.

For an artifact-aware agent job, invoke the selected capability with one input
whose role is `matrix` (for QC, PCA, clustering, heatmap) or `expression` (for
WGCNA), format is `csv` or `tsv`, and execution mode is `local-cpu`.
Normalization also requires `parameters.output` and emits a TSV artifact.

### WGCNA Co-Expression Network

WGCNA requires R and the WGCNA package. Set `LINXIRA_BIO_WORKFLOW_R_LIBRARY`
to the project-isolated R package library. Key parameters:

- `--min-expression X`: minimum expression threshold per gene (default 1)
- `--min-samples N`: minimum samples meeting threshold (default 3)
- `--min-module-size N`: minimum module size (default 30)
- `--merge-cut-height X`: module merge threshold (default 0.25)
- `--network-type`: `signed` (default), `unsigned`, or `signed hybrid`
- `--power N`: soft-thresholding power (0 = auto-detect, default)
- `--no-log-transform`: skip log2(x+1) transformation
- `--threads N`: number of threads (default 1)

Output artifacts:
- `module-assignments.csv`: gene-to-module mapping
- `module-eigengenes.csv`: sample eigengene values
- `module-summary.csv`: module sizes
- `scale-free-fit.csv`: power selection fit indices

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
- For WGCNA, validate the scale-free topology fit (R^2 > 0.8 recommended) and
  review module sizes before interpreting biological relevance.

Differential expression remains separate. Route raw integer counts and sample
metadata to `analyze-differential-expression`, and preserve the design,
contrast, package versions, normalization, and multiple-testing method.
