---
name: find-sequence-repeats
description: Find simple sequence repeats (SSR / microsatellite / microsatellites, mono to hexa-nucleotide motifs) in FASTA sequences with MISA-aligned semantics and per-motif-length repeat thresholds. Use for SSR marker mining, repeat density surveys, and compound SSR (cSSR) identification; supports rust, python (pytrf C kernel), and r (Biostrings C kernel) backends.
---

# Find Sequence Repeats

Detect perfect simple sequence repeats with the native Rust scanner or the
ecosystem C-kernel backends selected by `--backend`.

## Run
1. Inspect the input with `linxira-bio dataset inspect <input.fasta> --json`;
   require detected format `fasta`.
2. Run `linxira-bio sequence ssr <input.fasta[.gz]> <output.tsv> --min-repeats
   1:10,2:6,3:5,4:5,5:5,6:5 --compound-distance 100 --backend rust --json`.
3. Preserve the capability version, input hash, command, warnings, and result.

Semantics (identical across backends, MISA-aligned): at each position the
shortest motif whose tandem repeat meets the threshold wins, and the whole
repeat interval is consumed, so records never overlap. Bases outside ACGT
(interrupted by N) cannot extend. Records within `--compound-distance` bp
(default 100) in one sequence are marked as one compound SSR group.

Backends: `rust` (native scan, default), `python` (pytrf C kernel), `r`
(Biostrings C kernel). All three emit identical records and summaries; the
benchmark harness diffs them field by field. `auto` consults
runtime-preferences.json.

## Validate And Interpret

- Report SSR counts with motif-length breakdown and compound group count;
  a high mono-nucleotide share often masks assembly artefacts.
- Positions are 1-based inclusive on the forward strand; the motif is the
  repeat unit as it occurs (a TA-repeat starting on T is reported as TA,
  not AT).
- SSR density (SSR count per kb, derivable from total_bases) is the
  comparable metric across sequences; raw counts scale with length.
- Backends must agree exactly; a diff failure between rust and an ecosystem
  kernel is a bug to report, not a tolerance to widen.

Stop on malformed FASTA (headerless data, empty identifiers) and on
invalid `--min-repeats` specs (motif lengths outside 1-6, repeat counts
below 2). Do not treat SSR presence as functional annotation.

## Notes

- The python/r backends require the pinned benchmark-pack environments;
  the rust backend has no runtime dependency.
- Benchmark comparisons follow the measurement ledger rules: median of at
  least three runs, Linux as the primary environment.
