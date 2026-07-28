# BAM and CRAM Quality Report

## Purpose

Create a local alignment-quality report from a BAM or CRAM file using the
maintained `samtools stats` implementation.

## Inputs

Provide one local BAM or CRAM alignment file. Supply a local FASTA reference
when decoding a CRAM that does not contain its required reference bases.

## Parameters

Use `--reference reference.fasta` only when the native decoder needs it.

## Outputs

Write the complete tab-separated native report and JSON execution metadata.
The capability refuses to overwrite an existing output.

```bash
linxira-bio alignment bam-cram-qc sample.bam alignment-stats.tsv --json
```

## Examples

Run the command above to retain all reported alignment statistics for later
inspection or downstream table processing.

## Interpretation

Inspect mapped-read counts, duplication, insert-size, and quality sections in
the report in the context of the library and alignment method.

## Caveats

This is research analysis, not a clinical diagnostic. CRAM decoding can require
the exact reference used during encoding. The artifact-worker interface accepts
self-contained alignment inputs only; use the CLI when an external reference is
needed.

## Runtime Dependencies

Requires `samtools` on `PATH` or `LINXIRA_BIO_SAMTOOLS`. It is invoked directly
without a shell.

## Citations

Cite samtools, its version, the reference assembly, and the aligner used to
create the input.

## Troubleshooting

Run the environment audit for `samtools`; verify the CRAM reference and its
contig names when decoding fails.
