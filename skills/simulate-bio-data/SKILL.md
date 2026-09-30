---
name: simulate-bio-data
description: Generate deterministic synthetic biological data with a fixed seed - random DNA FASTA sequences with target GC content and N fraction, and single-end or paired FASTQ reads from a reference with configurable read length, coverage, fragment size, and substitution error rate. Use for benchmark fuel, pipeline dry runs, and GPU consistency references.
---

# Simulate Bio Data

Create reproducible synthetic sequences and reads; identical seeds and
options always produce byte-identical outputs.

## Run
1. Synthesize a reference: `linxira-bio simulate sequence <output.fasta>
   --count N --length N|--min-length A --max-length B --gc 50
   --n-fraction 0 --seed 42 --json`.
2. Synthesize reads from it: `linxira-bio simulate reads <reference.fasta>
   <output.fastq> --read-length 150 --coverage 30|--read-count N [--paired
   --fragment-length 300] --error-rate 0.1 --seed 42 --json`.
3. Preserve the seed and options with the artifacts; they are the
   reproducibility contract.

Semantics: integer-only random draws from a xoshiro256** generator seeded
via SplitMix64. Sequence bases draw N first, then the GC/AT split, then the
G/C and A/T coin flips, so realized GC content converges on the target as
lengths grow. Reads pick sequences and start positions uniformly, reverse-
complement half the single-end reads, and mate starts sit one fragment apart
in paired mode; substitutions replace bases at the configured error rate and
quality strings are flat.

## Validate And Interpret

- Re-running with the same seed must reproduce the file byte for byte;
  treat any difference as a bug.
- Realized coverage is reported in the summary and can deviate slightly from
  the requested coverage because reads are counted, not base-balanced.
- Simulated error models are substitutions only; indels and quality decay
  are not modeled, so downstream QC results will look cleaner than real
  data.

Stop on options violating their ranges (length 0, GC outside 0-100,
fragment shorter than the read). Do not publish simulated results as
experimental evidence.

## Notes

- Pure Rust; no runtime dependencies.
- These outputs are the fuel for benchmark runs and GPU consistency checks;
  keep the seed alongside every artifact for the measurement ledgers.
