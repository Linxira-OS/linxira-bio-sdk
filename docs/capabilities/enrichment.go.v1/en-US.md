# GO Over-Representation Analysis

## Purpose

Run local Gene Ontology over-representation analysis from explicit gene-to-GO associations.

## Inputs

A query identifier list and a CSV/TSV association table. Only syntactically valid `GO:` term IDs enter the GO universe.

## Parameters

Set minimum overlap, maximum reported terms, and optional overlap-gene reporting.

## Outputs

JSON reports mapping coverage, GO background size, hypergeometric p-values, Benjamini-Hochberg values, fold enrichment, and ranked GO terms.

## Examples

```bash
linxira-bio enrichment go genes.txt go-associations.tsv --json
```

## Interpretation

Interpret terms together with ontology namespace, study design, background coverage, effect size, and adjusted p-value.

## Caveats

No ontology graph expansion or semantic redundancy reduction is performed. Use a versioned association table.

## Runtime Dependencies

The calculation runs in local Rust without network access.

## Citations

Cite the GO release, annotation source, evidence filters, universe, and statistical correction.

## Troubleshooting

Normalize source GO columns first and confirm query identifiers occur in the association table.
