# FASTA To Table

## Purpose

Convert FASTA records into CSV or TSV rows for spreadsheet inspection, joins, and agent-readable tabular workflows.

## Inputs

One readable FASTA file. Plain text and gzip streams are supported.

## Parameters

The command requires input FASTA and output CSV/TSV paths. Optional parameters are `--delimiter csv|tsv` and `--no-header`. `--json` returns the standard result envelope.

## Outputs

Writes a CSV or TSV table with columns `id`, `description`, `length`, and `sequence`. JSON reports row count, residue count, delimiter, header state, and columns.

## Examples

```bash
linxira-bio sequence to-table input.fa records.tsv --delimiter tsv --json
```

## Interpretation

Each FASTA record becomes one table row. The description is the header text after the first whitespace-delimited identifier.

## Caveats

The sequence column is intended for text sequence data. Non-UTF-8 sequence bytes are rejected rather than silently rewritten.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

The table schema follows conventional FASTA identifier, description, length, and sequence fields.

## Troubleshooting

If a downstream table reader mis-detects the delimiter, pass `--delimiter csv` or `--delimiter tsv` explicitly.
