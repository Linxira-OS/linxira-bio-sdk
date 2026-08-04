# Research Cohort Pathway Analysis

## Purpose

Perform local over-representation analysis on a research cohort gene set and a supplied pathway association table. It is research use only and does not diagnose disease or guide treatment.

## Inputs

A query gene list and CSV/TSV associations containing `gene_id` and `term_id`.

## Parameters

Use `--min-overlap`, `--max-terms`, and `--include-genes` to control reporting.

## Outputs

Mapped query counts, exact one-sided hypergeometric p-values, BH-adjusted p-values, fold enrichment, and ranked pathway terms.

## Examples

```text
linxira-bio medical pathway genes.txt pathways.tsv --include-genes --json
```

## Interpretation

Reported associations describe enrichment under the supplied background, not causal mechanisms or clinical significance.

## Caveats

Results depend on identifier mapping, the association universe, and multiple-testing assumptions.

## Runtime Dependencies

Local Rust only; data is not uploaded.

## Citations

Cite the pathway association source, cohort definition, universe, and correction method.

## Troubleshooting

Use one identifier system across both files and inspect unmapped identifiers.
