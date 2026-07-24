---
name: intersect-genomic-intervals
description: Validate and measure overlap between two local BED interval sets with the executable interval.intersect.v1 capability. Use for region-set intersection counts, overlapped-feature counts, per-contig summaries, and total overlap bases when both inputs use compatible BED coordinates.
---

# Intersect Genomic Intervals

Compare two BED files locally with deterministic zero-based, half-open interval
semantics.

## Run

1. Inspect both inputs with `linxira-bio dataset inspect <input.bed> --json`.
2. Confirm both files are BED and refer to the same reference assembly and
   contig naming convention.
3. Run `linxira-bio interval intersect <left.bed> <right.bed> --json`.
4. Preserve input order because left- and right-overlapped counts differ.

For an artifact-aware agent job, invoke `interval.intersect.v1` with single
inputs whose roles are `left-bed` and `right-bed`, formats are `bed`, and
execution mode is `local-cpu`.

## Validate And Interpret

- Treat intervals as `[start, end)`; touching boundaries do not overlap.
- Verify reference assembly, coordinate system, and chromosome prefixes before
  interpreting a zero-overlap result.
- Distinguish overlap pairs from unique overlapped intervals. One interval may
  contribute to several pairs.
- Use `total_overlap_bases` as the sum across overlap pairs; it can double-count
  bases when intervals within an input overlap each other.
- Review per-contig counts for unexpected alternate contigs or naming splits.

Use a maintained `bedtools` workflow for joins, fractional-overlap thresholds,
strand rules, or emitted BED records not covered by this summary capability.
