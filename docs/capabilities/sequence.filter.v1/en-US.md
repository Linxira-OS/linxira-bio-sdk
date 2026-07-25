# FASTA Sequence Filtering

## Purpose

Filter FASTA records locally by length, GC percentage, and ambiguous-N percentage.

## Inputs

One readable FASTA file. Plain text and gzip streams are supported.

## Parameters

The command requires input and output FASTA paths. Optional filters are `--min-length`, `--max-length`, `--min-gc-percent`, `--max-gc-percent`, and `--max-n-percent`. `--json` returns the standard result envelope.

## Outputs

Writes a new FASTA containing records that pass every requested filter. JSON reports input/output record and residue counts plus rejection counts for length, GC, and N filters.

## Examples

```bash
linxira-bio sequence filter contigs.fa kept.fa --min-length 1000 --max-n-percent 5 --json
```

## Interpretation

Each record is evaluated independently. A record rejected by an earlier filter is counted under that first failing reason.

## Caveats

GC percentage uses canonical A/C/G/T/U bases as the denominator. The filter does not remove contamination or validate assembly correctness.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

GC and ambiguous-base summaries follow conventional FASTA sequence QC definitions.

## Troubleshooting

If no records are emitted, relax thresholds and inspect the input with `linxira-bio sequence stats INPUT --json` first.
