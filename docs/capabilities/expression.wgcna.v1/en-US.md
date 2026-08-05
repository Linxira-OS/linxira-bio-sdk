# WGCNA Co-Expression Network

## Purpose

Construct a weighted gene co-expression network using the WGCNA R package
to identify modules of co-expressed genes from an expression matrix.

## Inputs

A CSV or TSV expression matrix with genes as rows and samples as columns.
Values must be non-negative and finite.

## Parameters

`--min-expression` threshold per gene (default 1). `--min-samples` minimum
samples meeting threshold (default 3). `--min-module-size` minimum module
size (default 30). `--merge-cut-height` module merge threshold (default 0.25).
`--network-type` signed, unsigned, or signed hybrid (default signed).
`--power` soft-thresholding power (0 = auto-detect). `--no-log-transform`
skip log2(x+1) transformation. `--threads` number of threads.

## Outputs

JSON result with artifact paths: `module-assignments.csv` (gene-to-module),
`module-eigengenes.csv` (sample eigengenes), `module-summary.csv` (module
sizes), and `scale-free-fit.csv` (power selection fit indices).

## Examples

```bash
linxira-bio expression wgcna expression.tsv results.json --min-module-size 30 --threads 4 --json
```

## Interpretation

Validate scale-free topology fit (R² > 0.8 recommended). Review module sizes
and eigengene profiles before interpreting biological relevance.

## Caveats

Requires R and the WGCNA package installed. Large matrices consume significant
memory. The analysis is exploratory and does not establish causality.

## Runtime Dependencies

R 4.3+ with WGCNA, dynamicTreeCut, and fastcluster packages.

## Citations

Langfelder P, Horvath S. WGCNA: an R package for weighted correlation network
analysis. BMC Bioinformatics. 2008;9:559.

## Troubleshooting

If auto power detection fails, manually specify `--power`. Reduce
`--min-module-size` if no modules are detected. Ensure the expression matrix
has no missing or negative values.