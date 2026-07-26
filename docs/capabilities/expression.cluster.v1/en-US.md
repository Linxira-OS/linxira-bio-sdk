# Expression Clustering

## Purpose

Cluster expression samples and features independently with deterministic k-means.

## Inputs

A complete local CSV or TSV matrix with unique feature identifiers and finite values.

## Parameters

Set `--sample-clusters`, `--feature-clusters`, and `--max-iterations`.
Feature-wise z-score scaling is enabled by default; disable it with `--no-scale`.

## Outputs

JSON contains assignments, centroid distances, cluster sizes, convergence state,
iterations, and within-cluster sums of squares for samples and features.

## Examples

```bash
linxira-bio expression cluster matrix.tsv --sample-clusters 2 --feature-clusters 4 --json
```

## Interpretation

Compare assignments with independent experimental metadata and inspect cluster
sizes and distances before describing patterns.

## Caveats

Clusters are exploratory. Requested counts are reduced when an axis has fewer
items. Results depend on preprocessing and the chosen cluster counts. Local
numeric analysis is capped at 10 million matrix cells.

## Runtime Dependencies

Deterministic farthest-point initialization and k-means run in local Rust.

## Citations

Cite k-means and the normalization or transformation applied to the matrix.

## Troubleshooting

Reduce cluster counts for small matrices. If the iteration limit is reached,
increase `--max-iterations` or review scaling and outliers.
