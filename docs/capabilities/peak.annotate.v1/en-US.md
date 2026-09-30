# Peak Annotation

## Purpose

Assign each BED peak the nearest gene or feature from a GFF3/GTF annotation
with the local interval.closest index, producing deterministic peak-to-
feature tables for regulatory analyses.

## Inputs

One BED peak file (contig, start, end required; column 4 becomes the peak
name) and one GFF3/GTF annotation (plain or gzipped).

## Parameters

`--feature-type` selects the considered feature types (repeatable; default
`gene`). Output paths must not already exist.

## Outputs

A TSV with one row per peak: peak coordinates and name (1-based inclusive),
nearest feature id/name/type/span/strand, distance (0 for overlaps), and
direction (`overlapping`/`upstream`/`downstream` relative to the feature's
stranded anchor). JSON summarizes peak, annotated, and unmatched counts plus
per-feature-type feature counts and warnings.

## Examples

```bash
linxira-bio peak annotate peaks.bed genes.gff3.gz annotated.tsv --feature-type gene --json
```

## Interpretation

Direction is strand-aware: for a '-' strand feature, a peak lying after the
feature end is `upstream` (toward the TSS). Unmatched peaks mean no feature
of the requested type shares the contig — usually a naming mismatch, not
absent biology. Distance is a genomic gap, not regulatory evidence.

## Caveats

Only the single nearest feature is reported; ties resolve by the index order
of equal-distance spans. Contig naming must match exactly between inputs.
BED rows with fewer than three columns or invalid coordinates are rejected.

## Runtime Dependencies

None beyond the CLI — the scan is pure Rust over parsed text.

## Citations

When reporting peak annotations, cite the BED and GFF3 format specifications
(NIC/UCSC; Stein 2013, GFF3) and the upstream peak caller used to produce
the peaks.

## Troubleshooting

All peaks unmatched: compare contig names on both sides (`dataset inspect`
shows contigs for both formats). Missing feature ids: the annotation may use
non-standard attribute keys; normalize it with `annotation normalize` first.
