# KEGG Over-Representation Analysis

## Purpose

Run local pathway over-representation analysis from explicit KEGG-associated identifiers.

## Inputs

A query identifier list and a CSV/TSV association table. KEGG namespace rows and conventional pathway or ortholog identifiers define the universe.

## Parameters

Set minimum overlap, maximum reported terms, and optional overlap-gene reporting.

## Outputs

JSON reports mapped queries, KEGG background size, hypergeometric and adjusted p-values, fold enrichment, and ranked pathways or ortholog terms.

## Examples

```bash
linxira-bio enrichment kegg genes.txt kegg-associations.tsv --json
```

## Interpretation

Interpret adjusted significance with pathway coverage, organism mapping, effect size, and the supplied background.

## Caveats

The capability does not download, redistribute, or update KEGG data. The user supplies an appropriately licensed association table.

## Runtime Dependencies

The calculation runs in local Rust without contacting a remote pathway service.

## Citations

Cite the association source, organism, release date, identifier conversion, universe, and correction method.

## Troubleshooting

Confirm the namespace or term IDs identify KEGG records and that query IDs match the association table.
