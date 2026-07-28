# Short-Read Reference Alignment

## Purpose

Align one local short-read FASTQ file to a local FASTA reference with fixed
`minimap2 -x sr` settings, then create a coordinate-sorted BAM with samtools.

## Inputs

Provide one reference FASTA and one single-end FASTQ file. Pair-aware alignment
is not part of this v1 contract.

## Parameters

Set `--threads N` from 1 to 1024.

## Outputs

Write one new coordinate-sorted BAM and JSON execution metadata. The workflow
never overwrites inputs or an existing output.

```bash
linxira-bio alignment short-read reference.fa reads.fastq aligned.bam --threads 4 --json
```

## Examples

The command above creates an alignment BAM suitable for follow-up local quality
or coverage reporting.

## Interpretation

Review mapping quality, alignment rates, and coverage before drawing biological
conclusions.

## Caveats

Choose an aligner and preset appropriate to the assay. This v1 workflow is
single-end and is not a replacement for specialized paired-end or variant
calling pipelines.

## Runtime Dependencies

Requires `minimap2` and `samtools` on `PATH`, or `LINXIRA_BIO_MINIMAP2` and
`LINXIRA_BIO_SAMTOOLS`. Both processes are invoked directly without a shell.

## Citations

Cite minimap2, samtools, their versions, the reference assembly, and the input
read-processing method.

## Troubleshooting

Audit `minimap2` and `samtools`, ensure sufficient local disk space, and verify
the FASTQ and FASTA files are readable.
