# BED Interval Intersection

## Purpose

Measure overlap between two BED interval sets with deterministic zero-based,
half-open coordinate semantics.

## Inputs

Two readable BED files representing the left and right interval sets. Plain
text and gzip streams are detected by magic bytes. At least three tab-separated
columns are required.

## Parameters

Both input paths are required. Input order is significant. `--json` returns the
standard analysis result envelope. Version 1 has no strand or fraction filter.

## Outputs

Returns interval counts for each input, overlap-pair count, unique overlapped
interval counts for each side, summed overlap bases, per-contig summaries, and
warnings.

## Examples

```bash
linxira-bio interval intersect tests/fixtures/interval-intersect/left.bed tests/fixtures/interval-intersect/right.bed --json
```

## Interpretation

Intervals are `[start, end)`, so adjacent intervals do not overlap. One interval
can contribute to several overlap pairs. Summed overlap bases count each pair
and can therefore include the same genomic base more than once.

## Caveats

Both inputs must use the same reference assembly, coordinate convention, and
contig naming. Version 1 does not emit joined BED records or apply strand,
minimum-base, or fractional-overlap rules.

## Runtime Dependencies

This is a local Rust capability with no Python, R, Java, bedtools, or external
command-line dependency.

## Citations

Coordinate semantics follow the UCSC BED specification. Use bedtools for
advanced interval algebra beyond this versioned summary.

## Troubleshooting

Check assembly names and `chr` prefixes before interpreting zero overlaps.
Use the reported line number for missing columns, invalid coordinates, or
non-positive interval lengths.
