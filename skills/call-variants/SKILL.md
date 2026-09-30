---
name: call-variants
description: Call small variants (SNV/indel) from an indexed, reference-sorted BAM or CRAM with variant.call.v1, a native bcftools mpileup + bcftools call orchestration. Use to close the FASTQ-to-VCF gap before variant statistics, filtering, normalization, comparison, or annotation.
---

# Call Variants

Run the native bcftools caller locally to produce a VCF from a coordinate-sorted
alignment.

## Run

1. Audit the tool with `linxira-bio environment audit --json` and confirm
   `bcftools` (category variant-files) is available; set
   `LINXIRA_BIO_BCFTOOLS` to override the PATH lookup.
2. Confirm the alignment is coordinate-sorted and indexed
   (`.bai`/`.crai` beside it) and the reference FASTA is indexed (`.fai`);
   `linxira-bio alignment short-read` already emits a sorted BAM.
3. Run `linxira-bio variant call <sorted.bam|cram> <reference.fa>
   <output.vcf> --min-mq 20 --min-bq 13 --threads N --json`.
4. Pass `--all-sites` only when reference positions without alternate alleles
   are required; default emits variant sites only.
5. Preserve the capability version, input hashes, options, warnings, and the
   two-command provenance in the result.

## Interpret

- Default thresholds (min mapping quality 20, min base quality 13) follow the
  bcftools calling convention; raise them for stricter calls, not lower for
  sensitivity.
- The multiallelic caller (`-m`) is always used; genotype quality downstream
  filtering belongs to `variant.filter.v1`, not this call.

## Contract Notes

- The result JSON reports tool `bcftools`, mode `call`, output path and size,
  thread count, `command_count: 2` (mpileup then call), and warnings; the
  intermediate BCF is staged in a scratch directory and removed.
- Execution mode is local-cpu; the native binary is invoked without a shell
  and is never bundled.

Route the produced VCF to `analyze-variant-statistics` for statistics,
filtering, normalization, comparison, and snpEff annotation.
