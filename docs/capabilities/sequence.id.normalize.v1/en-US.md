# FASTA Identifier Normalization

## Purpose

Rewrite FASTA record identifiers deterministically for downstream tools that require short, stable, or uniformly numbered IDs.

## Inputs

One readable FASTA file. Plain text and gzip streams are supported.

## Parameters

The command requires input and output FASTA paths. Optional parameters are `--prefix`, `--start`, `--width`, `--no-padding`, and `--drop-description`. `--json` returns the standard result envelope.

## Outputs

Writes a new FASTA file with rewritten identifiers. JSON reports input/output records, residues, prefix, first and last numeric index, width, and whether descriptions were preserved.

## Examples

```bash
linxira-bio sequence normalize-ids input.fa renamed.fa --prefix seq --width 6 --json
```

## Interpretation

IDs are assigned in input order. With prefix `seq`, start `1`, and width `6`, the first identifier is `seq000001`.

## Caveats

This capability does not infer biological gene names or maintain an external mapping table. Keep the original FASTA when a reversible audit trail is required.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

Identifier normalization follows conventional FASTA record naming practice.

## Troubleshooting

If a downstream tool still rejects the file, use a prefix without whitespace or punctuation and inspect the output headers manually.
