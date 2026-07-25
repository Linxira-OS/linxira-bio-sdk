# FASTA Translation

## Purpose

Translate nucleotide FASTA records into protein FASTA using the NCBI standard genetic code.

## Inputs

One readable DNA or RNA FASTA file. Plain text and gzip streams are supported.

## Parameters

The command requires input and output FASTA paths. Use repeated `--frame FRAME` values from `-3`, `-2`, `-1`, `1`, `2`, or `3`; default is frame `1`. `--trim-terminal-stop` removes a final `*`; `--stop-at-first` stops at the first stop codon. `--json` returns the standard result envelope.

## Outputs

Writes one protein FASTA record per requested frame for each input record. JSON reports record and residue counts, selected frames, and genetic code.

## Examples

```bash
linxira-bio sequence translate cds.fa proteins.fa --frame 1 --trim-terminal-stop --json
```

## Interpretation

Output headers append `|frame=+N` or `|frame=-N`. Ambiguous codons translate to `X`; stop codons translate to `*` unless a stop option changes the output.

## Caveats

Only the NCBI standard code is implemented in this capability version. It does not validate CDS phase, transcript models, organellar codes, or biological completeness.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

Codon translation uses the NCBI standard genetic code, table 1.

## Troubleshooting

If translation fails, check for protein characters, mixed T/U alphabets, or frame choices that do not match the intended coding sequence.
