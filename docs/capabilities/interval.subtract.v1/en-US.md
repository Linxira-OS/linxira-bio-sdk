# BED Interval Subtraction

## Purpose

Subtract right-side BED intervals from left-side BED intervals and write the
remaining fragments as a deterministic BED3 output file.

## Inputs

Two readable BED files: the left intervals to retain and the right intervals to
remove. Plain text and gzip streams are detected by magic bytes. At least three
tab-separated columns are required in each record.

## Parameters

`<left.bed>`, `<right.bed>`, and `<output.bed>` are required. Input order is
significant. `--json` returns the standard result envelope.

## Outputs

Writes a new BED3 file containing retained left-side fragments. JSON reports
left/right interval counts, output interval count, affected left interval count,
removed bases, output bases, per-contig summaries, and warnings.

## Examples

```bash
linxira-bio interval subtract genes.bed repeats.bed genes-without-repeats.bed --json
```

## Interpretation

Intervals are `[start, end)`. Right-side intervals remove only overlapping bases
from left-side intervals. A single left interval may be split into several output
fragments.

## Caveats

Version 1 emits BED3 only; name, score, strand, and extra BED columns are not
preserved. It has no strand-specific, fractional-overlap, or reciprocal-overlap
rules.

## Runtime Dependencies

This is a local Rust capability with no Python, R, Java, bedtools, or external
command-line dependency.

## Citations

Coordinate semantics follow the UCSC BED specification. The subtraction behavior
uses standard interval algebra comparable to bedtools subtract for unstranded
base removal.

## Troubleshooting

Confirm left and right files use the same assembly and contig naming before
interpreting an empty output. If the command refuses to overwrite an output,
choose a new output path or remove the stale file deliberately.
