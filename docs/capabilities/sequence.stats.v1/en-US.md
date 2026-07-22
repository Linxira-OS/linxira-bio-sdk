# FASTA Sequence Statistics

## Purpose

Compute FASTA record count, lengths, N50/L50, auN, GC percentage, and N content locally.

## Inputs

One readable FASTA file. Multiline sequences are supported and headers must start with `>`.

## Parameters

The input path is required. `--json` returns the standard analysis result envelope.

## Outputs

Returns `sequence_count`, `total_bases`, minimum, maximum, and mean lengths,
`n50`, `l50`, `au_n`, `gc_percent`, `n_count`, and `n_percent`.

## Examples

```bash
linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json
```

## Interpretation

N50 is the sequence length at half of total length; L50 is the number of
sequences needed to reach that threshold. They describe contiguity, not assembly correctness.

## Caveats

GC percentage uses only A/C/G/T as its denominator. N percentage uses all
sequence characters. These statistics do not correct contamination, ploidy, or assembly errors.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

N50/L50 use their conventional definitions. auN is the length-weighted mean
`sum(length^2) / sum(length)`.

## Troubleshooting

If sequence data is reported before the first header, confirm the file is FASTA
and remove non-header content at its beginning.
