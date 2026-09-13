---
name: quantify-expression
description: Quantify transcript abundance from single- or paired-end reads with the native salmon tool against a pre-built index, keeping the verbatim quant.sf table plus a comparable summary (row counts, TPM/read totals). Use when an agent needs expression quantification or must cross-check Rust-orchestrated runs against a legacy pipeline's quant.sf. Not for index building, small-RNA, or alignment-free tools other than salmon.
---

# Quantify Expression

## Steps

1. Confirm salmon is installed (`environment audit`); resolve via
   `LINXIRA_BIO_SALMON` or PATH, and locate the pre-built index — the
   capability never builds indices.
2. Run one command per sample:

```bash
linxira-bio expression quantify r1.fq.gz r2.fq.gz \
  --index salmon_idx --threads 8 --output <run>/quant.sf --json
```

Single-end: pass one read file. `--lib-type A` (auto) is the default;
`--no-validate-mappings` disables selective alignment.
3. Cross-check against a legacy pipeline's quant.sf: transcript_count vs
   row count, total_tpm/total_num_reads totals, then per-transcript values.

## Contract Notes

- The verbatim quant.sf (Name, Length, EffectiveLength, TPM, NumReads) is
  the artifact; the JSON summary carries tool/lib_type/threads, totals,
  expressed transcript count, and the top-5 transcripts by TPM.
- Arguments are controlled and shell-free; gzipped FASTQ passes through.
