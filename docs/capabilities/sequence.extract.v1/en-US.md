# FASTA Sequence Extraction

## Purpose

Extract FASTA records by identifier and extract 1-based inclusive coordinate regions from local FASTA files.

## Inputs

One readable FASTA file. Plain text and gzip streams are supported. Headers must contain a non-empty first identifier token.

## Parameters

The command requires an input path and an output FASTA path. Use `--id ID` for full-record extraction and `--region ID:START-END[:+|-]` for coordinate extraction. `--strict` fails if any requested identifier or region target is missing. `--json` returns the standard result envelope.

## Outputs

Writes a new FASTA file and refuses to overwrite an existing output. JSON reports input and output record counts, residue counts, requested and matched identifiers, requested and emitted regions, and missing selectors.

## Examples

```bash
linxira-bio sequence extract genome.fa selected.fa --id chr1 --region chr2:100-250:- --strict --json
```

## Interpretation

Full-record outputs keep the original FASTA header. Region outputs use `ID:START-END:+` or `ID:START-END:-` headers. Reverse-strand regions are reverse-complemented against the extracted slice.

## Caveats

Coordinates are 1-based and inclusive. This capability does not interpret BED, GFF, GTF, CDS phase, exons, introns, or transcript models; annotation-guided extraction remains a separate capability.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

FASTA parsing and reverse-complement behavior follow conventional IUPAC nucleotide notation.

## Troubleshooting

If extraction fails with a missing selector, confirm the requested identifier matches the first whitespace-delimited token after `>` and that region coordinates do not exceed the record length.
