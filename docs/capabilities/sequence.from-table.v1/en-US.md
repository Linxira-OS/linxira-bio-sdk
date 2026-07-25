# Table To FASTA

## Purpose

Rebuild FASTA records from CSV or TSV sequence tables.

## Inputs

One readable CSV or TSV table with a header row. Plain text and gzip streams are supported.

## Parameters

The command requires input table and output FASTA paths. Optional parameters are `--delimiter csv|tsv`, `--id-column`, `--sequence-column`, `--description-column`, and `--no-description-column`. `--json` returns the standard result envelope.

## Outputs

Writes a new FASTA file. JSON reports input rows, output records, output residues, delimiter, and the column mapping used.

## Examples

```bash
linxira-bio sequence from-table records.tsv output.fa --delimiter tsv --json
```

## Interpretation

By default, the table must contain `id` and `sequence` columns. A `description` column is used when present and configured.

## Caveats

Identifiers must be non-empty and cannot contain whitespace. Whitespace inside sequence cells is removed before writing FASTA lines.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

The conversion follows conventional FASTA header and sequence-line construction.

## Troubleshooting

If conversion fails on a missing column, pass `--id-column`, `--sequence-column`, or `--description-column` to match the table header names.
