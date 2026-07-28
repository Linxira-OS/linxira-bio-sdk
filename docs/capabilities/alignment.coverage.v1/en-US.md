# Alignment Coverage Summary

## Purpose

Create a local depth and breadth summary for BAM or CRAM alignments with
`samtools coverage`.

## Inputs

Provide one local BAM or CRAM file.

## Parameters

Use `--reference reference.fasta` at the CLI when a CRAM requires external
reference bases.

## Outputs

Write a tab-separated coverage table plus JSON execution metadata. Existing
outputs are never overwritten.

```bash
linxira-bio alignment coverage sample.bam coverage.tsv --json
```

## Examples

Use the command above to produce per-reference breadth and depth values.

## Interpretation

Compare coverage breadth and mean depth across references, samples, and
libraries only after confirming comparable filtering and alignment settings.

## Caveats

Coverage is affected by duplicates, filters, reference composition, and input
sorting. This capability reports native results and does not perform diagnosis.

## Runtime Dependencies

Requires `samtools` on `PATH` or `LINXIRA_BIO_SAMTOOLS`; invocation is direct
and shell-free.

## Citations

Cite samtools, its version, the reference assembly, and upstream alignment and
filtering methods.

## Troubleshooting

Confirm the BAM/CRAM is readable by samtools and provide the correct reference
for CRAM decoding.
