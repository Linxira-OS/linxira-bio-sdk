---
name: annotate-peaks
description: Annotate BED genomic peaks (ChIP-seq, ATAC-seq, CUT&RUN, clip peaks) with the nearest gene or feature from a GFF3/GTF annotation using the local interval.closest index. Use for peak-to-gene assignment with distance, direction relative to the feature's strand, and overlap detection.
---

# Annotate Peaks

Assign each BED peak the nearest feature from a local GFF3/GTF annotation.

## Run
1. Inspect the peak file with `linxira-bio dataset inspect <peaks.bed> --json`;
   require detected format `bed`.
2. Run `linxira-bio peak annotate <peaks.bed> <annotation.gff3|gtf[.gz]>
   <output.tsv> --feature-type gene --json`. Repeat `--feature-type` to
   consider several feature types (default: gene).
3. Preserve the capability version, input hash, command, warnings, and result.

Semantics: peaks are matched per contig against the interval.closest index.
Overlapping peaks get distance 0 and direction `overlapping`; otherwise the
distance is the bp gap and the direction is `upstream`/`downstream` relative
to the feature's stranded anchor ('+' start, '-' end). BED coordinates are
converted to 1-based inclusive in the output. Unmatched peaks (no feature on
the same contig) are emitted with `.` placeholders and counted in warnings.

## Validate And Interpret

- Report annotated and unmatched peak counts together; a high unmatched share
  usually means contig naming mismatches between the peak file and the
  annotation.
- Direction is strand-aware: for a '-' strand gene, a peak positioned after
  the gene end is reported as `upstream` (toward the TSS), and vice versa.
- Distances are genomic gaps, not regulatory evidence; combine with
  enhancer/promoter priors before any functional claim.

Stop on malformed BED rows (fewer than 3 columns, non-numeric coordinates,
end <= start). Do not treat proximity as causality.

## Notes

- Pure Rust implementation; no runtime dependency beyond the CLI.
- The annotation file may be gzipped; both GFF3 and GTF attribute styles are
  parsed for ID/Name.
