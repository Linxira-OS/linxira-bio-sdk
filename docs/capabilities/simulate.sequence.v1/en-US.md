# Sequence Simulation

## Purpose

Synthesize deterministic random DNA FASTA with seeded generation — the fuel
for benchmark runs, pipeline dry runs, and GPU consistency references where
byte-identical regeneration matters.

## Inputs

None; options fully determine the output.

## Parameters

`--count N` sequences; `--length N` fixes one length, or `--min-length A
--max-length B` draws uniformly per sequence. `--gc` sets the GC target
percent (default 50), `--n-fraction` the N share (default 0), `--seed` the
generator seed (default 42), `--prefix` the identifier prefix (default
`sim`).

## Outputs

A FASTA with one record per requested sequence (headers carry the length)
and a JSON summary: seed, counts, length range, mean length, realized GC
percent, and N count.

## Examples

```bash
linxira-bio simulate sequence reference.fa --count 10 --length 100000 --gc 45 --seed 42 --json
```

## Interpretation

Realized GC content converges on the target as sequences grow; small
sequences deviate by sampling noise. The seed plus options is the
reproducibility contract: identical inputs must regenerate the file byte
for byte.

## Caveats

Sequences are i.i.d. draws — no repeats, isochores, or k-mer structure. For
structured benchmarks compose with `simulate reads` and explicit seed
differences per stage. The generator is integer-only; do not expect
floating-point statistical guarantees beyond the documented convergence.

## Runtime Dependencies

None beyond the CLI — pure Rust with xoshiro256** seeded via SplitMix64.

## Citations

No external method is reimplemented; cite this capability by version when
simulated data underpins a published comparison.

## Troubleshooting

Regenerated file differs: the seed or one option differs — compare the
JSON summary fields against the ledger entry. GC far from target: the
sequence length is too small for convergence; raise `--length`.
