# Simple Sequence Repeat Mining

## Purpose

Detect perfect simple sequence repeats (SSR / microsatellites, mono- to
hexa-nucleotide motifs) in FASTA sequences for marker development and genome
repeat surveys, with MISA-aligned semantics and selectable rust / python /
r backends.

## Inputs

One FASTA file, optionally gzipped. Identifiers are the first
whitespace-delimited token of each header.

## Parameters

`--min-repeats` takes a `length:count` list (default
`1:10,2:6,3:5,4:5,5:5,6:5`, the MISA standard; unspecified lengths keep the
default). `--compound-distance` sets the maximum interruption in bp for
compound SSR grouping (default 100, MISA's interruption_max_distance).
`--backend auto|rust|python|r` selects the implementation: the native Rust
scanner, the pytrf C kernel via the python benchmark pack, or Biostrings via
the r benchmark pack. `auto` consults runtime-preferences.json.

## Outputs

A TSV with one row per repeat (sequence_id, motif_length, motif, repeats,
size, start, end, compound) and a JSON summary: sequence and SSR counts,
total bases, compound group count, per-motif-length counts, and per-sequence
statistics.

## Examples

```bash
linxira-bio sequence ssr genome.fa ssr.tsv --min-repeats 2:8 --compound-distance 100 --json
linxira-bio benchmark run sequence.ssr.v1 --fasta genome.fa   # three-backend comparison
```

## Interpretation

At each position the shortest motif whose tandem repeat meets the threshold
is reported and the interval consumed, so records never overlap — identical
to MISA. The motif is reported as it occurs in the sequence. Compare SSR
density (count per kb) rather than raw counts across sequences. Compound
groups (records within the interruption distance) approximate cSSR counts.

## Caveats

Only perfect repeats are detected; interrupted repeats are reported only
through compound grouping. Bases outside ACGT (N) break repeats and are
never part of a motif. The python/r backends require their pinned pack
environments; a backend diff mismatch is a bug, not noise.

## Runtime Dependencies

Rust backend: none. Python backend: the pinned pytrf environment from the
benchmark pack lock. R backend: Biostrings from the benchmark pack lock.

## Citations

When reporting SSR surveys, cite Thiel et al. 2003 (Theor. Appl. Genet.
106:1231-1238, MISA) for the semantics and thresholds; cite Benson 1999
(Nucleic Acids Res. 27:573-580) when contrasting with TRF-derived repeats.

## Troubleshooting

Empty output with a non-empty FASTA usually means thresholds are stricter
than the data (raise `--min-repeats` counts or check motif lengths 1-6).
Backend route failures report the missing pack environment; run the
environment audit to install the pinned python/r dependency sets.
