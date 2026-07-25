# FASTA Merge

## Purpose

Concatenate one or more FASTA files into a single FASTA while preserving input order.

## Inputs

One or more readable FASTA files. Plain text and gzip streams are supported.

## Parameters

The command takes the output FASTA first, followed by input FASTA paths. `--allow-duplicate-ids` keeps duplicate identifiers; duplicates are rejected by default. `--json` returns the standard result envelope.

## Outputs

Writes a new merged FASTA file. JSON reports input file count, record and residue counts, duplicate identifier count, and whether duplicates were allowed.

## Examples

```bash
linxira-bio sequence merge merged.fa sample1.fa sample2.fa --json
```

## Interpretation

Records are emitted in the order of the input paths and the order within each file.

## Caveats

Duplicate identifiers are usually unsafe for downstream extraction and indexing, so they are rejected unless explicitly allowed.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

FASTA concatenation follows conventional text FASTA processing behavior.

## Troubleshooting

If the merge fails on duplicate IDs, either normalize IDs first with `linxira-bio sequence normalize-ids` or rerun with `--allow-duplicate-ids` when duplicates are intentional.
