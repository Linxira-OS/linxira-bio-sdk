# SAM Alignment Quality Control

## Purpose

Validate SAM text records and summarize mapping, flag, MAPQ, duplicate, and
reference-level metrics locally.

## Inputs

One readable SAM text file. Plain text and gzip streams are detected by magic
bytes. BAM and CRAM are not accepted.

## Parameters

The input path is required. `--json` returns the standard analysis result
envelope. Version 1 has no scientific thresholds.

## Outputs

Returns header and record counts; primary, secondary, and supplementary record
counts; mapped and unmapped counts and percentage; paired, proper-pair, read-1,
read-2, duplicate, and QC-fail counts; zero-MAPQ count; mean MAPQ across mapped
records; per-reference counts; and warnings.

## Examples

```bash
linxira-bio alignment qc tests/fixtures/alignment-qc/valid.sam --json
```

## Interpretation

Metrics count SAM alignment records, not unique biological fragments. Secondary
and supplementary records can represent the same read as a primary record.
MAPQ meaning and scale depend on the aligner.

## Caveats

This capability does not read BAM or CRAM, validate CIGAR against sequence
length, estimate insert-size distributions, inspect base qualities, or replace
the complete reports produced by samtools, Picard, or an aligner-specific tool.

## Runtime Dependencies

This is a streaming local Rust capability with no Python, R, Java, htslib, or
external command-line dependency.

## Citations

Field and FLAG semantics follow the SAM/BAM Format Specification maintained by
the Global Alliance for Genomics and Health.

## Troubleshooting

Use the reported line number to locate malformed columns, numeric fields, or
SEQ/QUAL length mismatches. Convert BAM or CRAM with a maintained samtools
workflow before running this capability.
