# Alignment Window Depth

## Purpose

Compute fixed-window depth and breadth metrics for a BAM/CRAM alignment by
streaming native `samtools depth` output and aggregating it in Rust, giving
coverage profiles that scale to whole genomes without materialising per-base
tables.

## Inputs

A BAM or CRAM alignment. Coordinate-sorted input with an index is
recommended; CRAM files whose reference is not embedded must resolve it via
the samtools reference path configuration.

## Parameters

`--window-size` sets the fixed window length in bases (default 10000, must be
positive). `--min-mq` and `--min-bq` filter reads by mapping and base quality
(defaults 0). `--threads` sets samtools worker threads (default 1). Input and
output paths must differ.

## Outputs

Writes a TSV with one row per window (reference, start, end, mean depth,
min, max, covered bases, breadth percent) using 1-based inclusive
coordinates; the final window of each reference is partial. JSON reports the
native-tool provenance plus whole-file and per-reference summaries (window,
base, and covered-base counts, breadth percent, mean depth, maximum depth).
Depth sums are exact 64-bit integers, so means carry no accumulation error.

## Examples

```bash
linxira-bio alignment window-depth aligned.sorted.bam windows.tsv --window-size 10000 --json
```

## Interpretation

Read mean depth together with breadth percent: a high mean with low breadth
indicates uneven, concentrated coverage. Windows are genomic bins, not
independent biological units; use them to localise coverage drops, not as
replicates. Quality-filtered bases count as uncovered at their positions.

## Caveats

Requires samtools 1.13 or newer (the unambiguous long `--min-MQ`/`--min-BQ`
flags). The per-base stream is parsed strictly: positions must be ascending
and grouped by reference, matching `samtools depth -aa` output; hand-edited
input is rejected. Output with no mapped reads yields an empty table and a
warning, not an error.

## Runtime Dependencies

The native `samtools` executable (probed by the environment audit under the
alignment-files category; overridable via `LINXIRA_BIO_SAMTOOLS`). No Python,
R, or Java runtime is used.

## Citations

When reporting window depth, cite the samtools suite: Li et al. 2009
(Bioinformatics 25:2078-2079) and the samtools-depth manual page.

## Troubleshooting

Run the environment audit first and check the samtools version (`samtools
--version`). Confirm the alignment contains mapped reads; an empty window
table with the no-records warning means the depth stream was empty. For CRAM
reference errors, configure the reference path or convert to BAM.
