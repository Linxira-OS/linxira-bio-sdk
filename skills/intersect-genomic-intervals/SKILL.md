---
name: intersect-genomic-intervals
description: Validate and transform local BED interval sets with executable interval.intersect.v1, interval.merge.v1, interval.subtract.v1, and interval.closest.v1 capabilities. Use for region-set overlap summaries, BED3 interval merging or subtraction, deterministic nearest-feature lookup, per-contig summaries, and total base counts when inputs use compatible BED coordinates.
---

# Intersect Genomic Intervals

Compare or transform BED files locally with deterministic zero-based, half-open
interval semantics.

## Run

1. Inspect every input with `linxira-bio dataset inspect <input.bed> --json`.
2. Confirm both files are BED and refer to the same reference assembly and
   contig naming convention.
3. Select the operation:
   - Intersect: `linxira-bio interval intersect <left.bed> <right.bed> --json`.
   - Merge: `linxira-bio interval merge <input.bed> <output.bed> [--max-gap N] --json`.
   - Subtract: `linxira-bio interval subtract <left.bed> <right.bed> <output.bed> --json`.
   - Closest: `linxira-bio interval closest <query.bed> <target.bed> <output.tsv> --json`.
4. Preserve input order for intersect and subtract because left and right roles
   are semantically different.

For artifact-aware agent jobs:

- Invoke `interval.intersect.v1` with single inputs whose roles are `left-bed`
  and `right-bed`.
- Invoke `interval.merge.v1` with one single input role, `bed`, plus string
  parameter `output` and optional integer parameter `max_gap`.
- Invoke `interval.subtract.v1` with single inputs whose roles are `left-bed`
  and `right-bed`, plus string parameter `output`.
- Invoke `interval.closest.v1` with single roles `query-bed` and `target-bed`,
  plus string parameter `output`.
- Declare input format `bed` and execution mode `local-cpu`.

## Validate And Interpret

- Treat intervals as `[start, end)`; touching boundaries do not overlap.
- Verify reference assembly, coordinate system, and chromosome prefixes before
  interpreting a zero-overlap result.
- Distinguish overlap pairs from unique overlapped intervals. One interval may
  contribute to several pairs.
- Use `total_overlap_bases` as the sum across overlap pairs; it can double-count
  bases when intervals within an input overlap each other.
- `interval.merge.v1` emits BED3 only and merges overlapping, bookended, or
  `--max-gap`-separated intervals within each contig.
- `interval.subtract.v1` emits BED3 only and removes right-side bases from
  left-side intervals; unmapped BED fields are not preserved.
- `interval.closest.v1` emits one headered TSV row per query with a target on
  the same contig. Distance ties choose the target with the smallest start/end;
  bookended intervals have distance zero but remain upstream or downstream.
- Review per-contig counts for unexpected alternate contigs or naming splits.

Use a maintained `bedtools` workflow for joins, coverage, fractional-overlap
thresholds, strand-aware distance, all-ties reporting, or record-preserving
operations not covered by these v1 capabilities.
