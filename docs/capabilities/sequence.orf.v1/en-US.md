# FASTA ORF Prediction

## Purpose

Find complete and optional 3-prime partial open reading frames in nucleotide FASTA records locally.

## Inputs

One readable DNA or RNA FASTA file. Plain text and gzip streams are supported.

## Parameters

The command requires input and output FASTA paths. `--min-amino-acids N` sets the minimum protein length. Use `--forward-only` to skip reverse-strand search and `--include-partial-3prime` to emit ORFs that start with ATG but reach the sequence end without a stop codon. `--json` returns the standard result envelope.

## Outputs

Writes predicted ORF protein sequences as FASTA. JSON reports input/output record and residue counts, records with ORFs, complete and partial ORF counts, longest ORF length, minimum length, and whether reverse-strand search was enabled.

## Examples

```bash
linxira-bio sequence orf contigs.fa orfs.faa --min-amino-acids 30 --include-partial-3prime --json
```

## Interpretation

Output headers include ordinal, strand, frame, 1-based start, inclusive end, and complete or partial status. Proteins omit the terminal stop codon for complete ORFs.

## Caveats

This is a deterministic ORF finder, not a gene predictor. It does not model introns, codon bias, non-standard genetic codes, start-codon alternatives, or annotation evidence.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

ORFs are identified with ATG starts and TAA/TAG/TGA stops under the NCBI standard genetic code.

## Troubleshooting

If too few ORFs are returned, lower `--min-amino-acids`, enable partial ORFs, and confirm the sequence orientation.
