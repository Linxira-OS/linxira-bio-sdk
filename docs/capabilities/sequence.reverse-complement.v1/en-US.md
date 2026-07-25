# FASTA Reverse Complement

## Purpose

Generate reverse-complement FASTA records for DNA or RNA nucleotide sequences locally.

## Inputs

One readable FASTA file containing DNA or RNA nucleotide symbols. Plain text and gzip streams are supported.

## Parameters

The command requires input and output FASTA paths. `--json` returns the standard result envelope.

## Outputs

Writes a new FASTA with one reverse-complemented record per input record. JSON reports input/output record and residue counts.

## Examples

```bash
linxira-bio sequence reverse-complement transcripts.fa reverse.fa --json
```

## Interpretation

IUPAC ambiguous nucleotide symbols are complemented where defined. RNA inputs preserve U in complements; DNA inputs use T.

## Caveats

Records that mix T and U are rejected to avoid silently changing molecule type. Protein FASTA is not supported.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

Complement mapping follows standard IUPAC nucleotide ambiguity notation.

## Troubleshooting

If the command rejects a symbol, confirm the input is nucleotide FASTA and not protein FASTA or a mixed DNA/RNA export.
