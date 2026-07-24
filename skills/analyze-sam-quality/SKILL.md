---
name: analyze-sam-quality
description: Validate and summarize local SAM alignment files with the executable alignment.qc.v1 capability. Use for mapping-rate, flag, duplicate, MAPQ, reference-count, and basic alignment-QC requests when the input is SAM text; use samtools through an approved native workflow for BAM or CRAM.
---

# Analyze SAM Quality

Run deterministic SAM text QC locally. Keep BAM and CRAM routed to maintained
`samtools` workflows until a verified adapter is available.

## Run

1. Inspect the input with `linxira-bio dataset inspect <input.sam> --json`.
2. Require detected format `sam`; do not pass BAM or CRAM to this capability.
3. Run `linxira-bio alignment qc <input.sam> --json`.
4. Preserve the capability version, input hash, command, warnings, and result.

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
