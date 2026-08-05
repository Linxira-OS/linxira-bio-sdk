# Sequence Shuffle

## Purpose

Randomize the order of sequences in a FASTA file using Fisher-Yates shuffle
with a user-specified seed for reproducibility.

## Inputs

A plain or gzip FASTA file with one or more sequences.

## Parameters

`--seed` sets the random seed for deterministic shuffling (required).

## Outputs

A FASTA file with the same sequences in randomized order. JSON result
includes input sequence count and the seed used.

## Examples

```bash
linxira-bio sequence shuffle input.fa shuffled.fa --seed 42 --json
```

## Interpretation

Verify that the output contains the same number of sequences as the input.
The shuffle is deterministic for a given seed.

## Caveats

The shuffle randomizes sequence order only; sequence content is unchanged.
Memory usage scales with the total sequence count.

## Runtime Dependencies

Local Rust only; no Python, R, Java, or external executable is required.

## Citations

Fisher RA, Yates F. Statistical tables for biological, agricultural and
medical research. 1938.

## Troubleshooting

Ensure the input is valid FASTA format. Existing output files are never overwritten.