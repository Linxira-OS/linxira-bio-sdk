# Basic VCF Filtering

## Purpose

Validate and stream basic VCF record filters locally without rewriting passing records.

## Inputs

One valid plain, gzip, or BGZF VCF text file. BCF is not accepted.

## Parameters

Combine minimum QUAL, PASS-only, repeated contig allow-list, and minimum `INFO/DP` conditions.

## Outputs

A VCF containing the original headers and passing records plus counts for every rejection category.

## Examples

```bash
linxira-bio variant filter input.vcf filtered.vcf --min-qual 20 --pass-only --min-info-dp 10 --json
```

## Interpretation

Missing QUAL fails a minimum-QUAL condition, and missing `INFO/DP` fails a minimum-depth condition.

## Caveats

This does not filter sample FORMAT fields, recalibrate calls, annotate variants, or provide clinical interpretation.

## Runtime Dependencies

Local streaming Rust only; no htslib, Python, R, or Java dependency.

## Citations

QUAL, FILTER, CHROM, and INFO semantics follow the GA4GH VCF specification.

## Troubleshooting

Use the reported VCF line for malformed fields. Convert BCF to VCF with a maintained native tool first.
