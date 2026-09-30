# Read Simulation

## Purpose

Synthesize deterministic single-end or paired FASTQ reads from a reference
FASTA with seeded generation — reproducible inputs for aligner benchmarks,
variant-calling dry runs, and GPU consistency references.

## Inputs

One reference FASTA (plain or gzipped). Sequences shorter than the read
length are skipped by the sampler.

## Parameters

`--read-length N` (default 150); coverage or count via `--coverage C`
(percent, default 30) or `--read-count N`; `--paired` emits mate pairs one
`--fragment-length` apart (default 300); `--error-rate` sets per-base
substitution probability (percent, default 0); `--seed` (default 42);
`--quality-char` one printable ASCII quality symbol (default `I`, i.e. Q40).

## Outputs

A FASTQ with one record per read (`/1`, `/2` suffixes in paired mode;
headers carry the source position, `_revpos` marking reverse-complement
reads) and a JSON summary: seed, pairing, reference counts, emitted read
count, requested and realized coverage, and the error rate.

## Examples

```bash
linxira-bio simulate reads reference.fa reads.fq --read-length 150 --coverage 30 --paired --error-rate 0.1 --seed 42 --json
```

## Interpretation

Realized coverage can deviate slightly from the request because reads are
counted, not base-balanced. The error model is substitutions only; reads
look cleaner than real instrument data, which matters when comparing QC
pipelines. The seed plus options is the reproducibility contract.

## Caveats

No indels, no quality decay, no bias models — perfect-uniform sampling.
Sequences shorter than the read length are excluded from sampling; a
reference of only short sequences is rejected. Fragment placement clamps at
sequence ends.

## Runtime Dependencies

None beyond the CLI — pure Rust with xoshiro256** seeded via SplitMix64.

## Citations

No external simulator is reimplemented (contrast with art/wgsim); cite this
capability by version when simulated reads underpin a published comparison.

## Troubleshooting

"no reference sequence is at least read_length long": lower `--read-length`
or provide a longer reference. Read count far from the coverage request:
the reference is small — pass `--read-count` explicitly for exact counts.
