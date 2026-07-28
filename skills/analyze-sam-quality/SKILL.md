---
name: analyze-sam-quality
description: Validate local SAM alignment files and run controlled local BAM/CRAM quality, coverage, or short-read alignment workflows. Use for mapping-rate, flag, duplicate, MAPQ, reference-count, BAM/CRAM samtools reports, coverage summaries, or minimap2 short-read reference alignment.
---

# Analyze SAM Quality

Run deterministic SAM text QC or a controlled maintained native tool locally.

## Run

1. Inspect the input with `linxira-bio dataset inspect <input.sam> --json`.
2. For SAM, require detected format `sam`; do not pass BAM or CRAM to this capability.
3. Run `linxira-bio alignment qc <input.sam> --json`.
4. Preserve the capability version, input hash, command, warnings, and result.

For BAM or CRAM, run `linxira-bio alignment bam-cram-qc <input.bam|cram>
<output.tsv> --json`. The output retains the installed `samtools stats`
report. Supply `--reference <reference.fasta>` at the CLI when CRAM decoding
requires an external reference; the artifact-worker interface currently
supports only self-contained inputs.

For breadth and depth summary tables, run `linxira-bio alignment coverage
<input.bam|cram> <output.tsv> --json`.

For one local short-read FASTQ file, run `linxira-bio alignment short-read
<reference.fasta> <reads.fastq> <output.bam> --threads N --json`. This runs
fixed `minimap2 -x sr` arguments followed by `samtools sort`; it never shells
out or overwrites an existing result.

For an artifact-aware agent job, invoke `alignment.qc.v1` with one input whose
role is `sam`, format is `sam`, and execution mode is `local-cpu`.

## Validate And Interpret

- Confirm `record_count` equals mapped plus unmapped records.
- Interpret primary, secondary, and supplementary counts using SAM FLAG bits;
  do not treat all records as independent reads.
- Report mapping rate, zero-MAPQ count, duplicate count, QC-fail count, mean
  mapped-record MAPQ, and per-reference counts together.
- Treat MAPQ as aligner-specific confidence, not a universal probability.
- Report a missing SAM header as reduced provenance, not automatic corruption.

Stop on malformed columns, flags, coordinates, or unequal sequence and quality
lengths. Do not infer biological or clinical significance from alignment QC.
