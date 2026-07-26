# GO Annotation Normalization

## Purpose

Construct a deterministic gene-to-GO association table from a local CSV or TSV annotation column.

## Inputs

A headered CSV/TSV table with a gene identifier column and a GO column containing comma, semicolon, or pipe-separated `GO:` identifiers.

## Parameters

Use `--gene-column` and `--go-column` only when automatic header aliases do not match. An output TSV path is required.

## Outputs

JSON reports row, gene, term, and deduplicated association counts. A normalized `gene_id`, `term_id`, `term_name`, `namespace` TSV is written without overwriting an existing file.

## Examples

```bash
linxira-bio annotation go annotations.tsv go-associations.tsv --json
```

## Interpretation

Each output row is one unique gene-to-GO association. Duplicate source rows do not increase the association count.

## Caveats

This capability validates GO identifier syntax but does not download an ontology, resolve obsolete terms, or infer parent terms.

## Runtime Dependencies

Parsing, validation, deduplication, and TSV writing run in local Rust.

## Citations

Cite the annotation source, ontology release, identifier mapping, and filters used to create the input.

## Troubleshooting

Confirm the table has a header and specify explicit column names when nonstandard labels are used.
