# MUSCLE Multiple Sequence Alignment

## Purpose

Run MUSCLE 5 locally to produce a reusable FASTA multiple sequence alignment.

## Inputs

A local nucleotide or protein FASTA containing the sequences to align.

## Parameters

Choose standard `align` or large-dataset `super5` mode and set threads.

## Outputs

An aligned FASTA plus JSON execution metadata and Worker v2 input/output hashes.

## Examples

```bash
linxira-bio msa muscle sequences.fa alignment.fa --mode align --threads 4 --json
```

## Interpretation

Inspect alignment length, gap distribution, sequence coverage, and downstream model assumptions before inference.

## Caveats

Alignment is not a phylogeny. The wrapper does not trim columns or infer a tree, and it refuses to overwrite outputs.

## Runtime Dependencies

Requires a local MUSCLE 5 executable.

## Citations

Cite MUSCLE, its version and mode, input sequence source, and downstream trimming or inference methods.

## Troubleshooting

Audit `muscle`; configure `LINXIRA_BIO_MUSCLE` when the executable is outside `PATH`.
