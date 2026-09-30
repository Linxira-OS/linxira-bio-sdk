# Alignment Duplicate Marking

## Purpose

Flag PCR and optical duplicates in a BAM alignment with the native samtools
chain, preparing the file for duplicate-aware variant calling.

## Inputs

A BAM alignment (coordinate order is not required; the chain re-sorts
internally). Paired-end data is required — fixmate metadata is what markdup
consumes.

## Parameters

`--threads` sets samtools worker threads (default 1). `--stats` emits
duplicate statistics while keeping duplicate-flagged reads. Input and output
paths must differ.

## Outputs

Writes a BAM whose duplicate records carry the 0x400 flag. JSON reports tool
`samtools`, mode `markdup`, output path and byte size, thread count,
`command_count` of 4, and warnings. Intermediate files are staged in a scratch
directory and removed.

## Examples

```bash
linxira-bio alignment markdup aligned.bam marked.bam --stats --json
```

## Interpretation

Duplicates are flagged, never removed; downstream tools decide how to use the
flag. The four internal steps are collate, fixmate -m, sort, markdup — exactly
the preprocessing samtools markdup documents.

## Caveats

Single-end libraries produce meaningless fixmate metadata and unreliable
marking. Requires `samtools` on PATH or via `LINXIRA_BIO_SAMTOOLS`; the binary
is invoked, never bundled.

## Runtime Dependencies

The native `samtools` executable (probed by the environment audit under the
alignment-files category). No Python, R, or Java runtime is used.

## Citations

When reporting duplicate marking, cite the samtools suite: Li et al. 2009
(Bioinformatics 25:2078-2079) and the markdup documentation in the samtools
manual.

## Troubleshooting

Run the environment audit first. Confirm the BAM is paired-end; samtools emits
fixmate warnings on single-end input that indicate the result should not be
trusted.
