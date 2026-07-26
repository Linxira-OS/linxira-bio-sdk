# Custom Over-Representation Analysis

## Purpose

Test local gene-to-term associations for over-representation in a query identifier set.

## Inputs

A query identifier list and a CSV/TSV association table with `gene_id` and `term_id`; `term_name` and `namespace` are optional.

## Parameters

Set minimum overlap, maximum reported terms, and whether overlap identifiers are included. The association universe is the background.

## Outputs

JSON reports mapped and unmapped queries, background size, one-sided hypergeometric p-values, Benjamini-Hochberg adjusted p-values, fold enrichment, and ranked terms.

## Examples

```bash
linxira-bio enrichment custom genes.txt associations.tsv --include-genes --json
```

## Interpretation

Smaller adjusted p-values indicate stronger evidence against random overlap under the supplied universe; fold enrichment measures effect size.

## Caveats

Results depend on the association source and background. Association does not establish causality or clinical significance.

## Runtime Dependencies

Parsing, exact counting, hypergeometric tails, and multiple-testing correction run in local Rust.

## Citations

Cite the association source, universe definition, identifier mapping, filters, and correction method.

## Troubleshooting

Ensure both files use the same identifier system and review `query_unmapped_count` before interpretation.
