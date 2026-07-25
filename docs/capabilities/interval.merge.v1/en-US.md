# BED Interval Merge

## Purpose

Merge overlapping, bookended, or nearby BED intervals within each contig and
write a deterministic BED3 output file.

## Inputs

One readable BED file. Plain text and gzip streams are detected by magic bytes.
At least three tab-separated columns are required.

## Parameters

`<input.bed>` and `<output.bed>` are required. `--max-gap N` also merges
intervals separated by at most `N` bases; the default is `0`, which merges
overlapping and bookended intervals only. `--json` returns the standard result
envelope.

## Outputs

Writes a new BED3 file containing `contig`, `start`, and `end`. JSON reports
input and output interval counts, merged interval count, input/output bases,
`max_gap`, per-contig summaries, and warnings.

## Examples

```bash
linxira-bio interval merge regions.bed merged.bed --max-gap 10 --json
```

## Interpretation

Intervals are `[start, end)`. The output is sorted by contig and start position.
`merged_interval_count` is the number of input intervals absorbed into larger
output intervals rather than emitted unchanged.

## Caveats

Version 1 emits BED3 only; name, score, strand, and extra BED columns are not
preserved. Use `bedtools merge` or a later record-preserving capability when
attribute handling is required.

## Runtime Dependencies

This is a local Rust capability with no Python, R, Java, bedtools, or external
command-line dependency.

## Citations

Coordinate semantics follow the UCSC BED specification. The merge behavior is
compatible with standard interval algebra used by tools such as bedtools.

## Troubleshooting

If the command refuses to overwrite an output, choose a new output path or remove
the stale file deliberately. Check line numbers in malformed BED errors for
missing columns, invalid coordinates, or non-positive interval lengths.
