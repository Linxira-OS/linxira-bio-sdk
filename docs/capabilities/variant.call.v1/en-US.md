# Variant Calling

## Purpose

Call small variants (SNV and indel) from a coordinate-sorted, indexed BAM or
CRAM alignment against a reference FASTA, closing the FASTQ-to-VCF pipeline
gap.

## Inputs

An indexed coordinate-sorted alignment (BAM or CRAM) and an indexed reference
FASTA. Short-read alignments produced by the alignment capability are already
in the required form.

## Parameters

`--min-mq` (default 20) filters pileup reads by mapping quality; `--min-bq`
(default 13) filters bases by quality; both accept 0-255. `--threads` sets
bcftools worker threads. `--all-sites` emits reference positions without
alternate alleles; the default emits variant sites only. Input and output
paths must differ.

## Outputs

Writes a VCF at the requested path. JSON reports tool `bcftools`, mode `call`,
output path and byte size, thread count, `command_count` of 2, and warnings.
The intermediate BCF is staged in a scratch directory beside the output and
removed afterwards.

## Examples

```bash
linxira-bio variant call sample.sorted.bam reference.fa calls.vcf --min-mq 20 --min-bq 13 --json
```

## Interpretation

The multiallelic caller model is always used. Default thresholds follow the
bcftools calling convention; tighten rather than loosen them for sensitivity
trades. Genotype-quality and cohort filtering belongs to the variant filter
capability, not this call.

## Caveats

Requires `bcftools` on PATH or via `LINXIRA_BIO_BCFTOOLS`; the binary is
detected and invoked, never bundled. Joint genotyping across cohorts and
structural-variant calling are out of scope.

## Runtime Dependencies

The native `bcftools` executable (probed by the environment audit under the
variant-files category). No Python, R, or Java runtime is used.

## Citations

When reporting calls, cite Li et al. 2009 (Bioinformatics 25:2078-2079) for
the SAMTools/bcftools pileup model and the bcftools call multiallelic model
documented in its manual.

## Troubleshooting

Run the environment audit first. Confirm the alignment is coordinate-sorted
with an index beside it, and that the reference carries a `.fai` index;
unsorted inputs are rejected by bcftools with an ordering error.
