# VCF to Table Conversion

## Purpose

Convert VCF variant records to a TSV table with fixed CHROM, POS, ID, REF, ALT,
QUAL, FILTER, and INFO columns plus one column per sample.

## Inputs

One valid plain, gzip, or BGZF VCF text file. BCF is not accepted.

## Parameters

The input and output paths are required. `--json` returns the standard analysis
result envelope.

## Outputs

A TSV file with a header row. The first eight columns are the fixed VCF fields.
If the VCF header declares samples, one additional column per sample is appended
with the sample name as the column header. Sample columns contain the full
FORMAT:values string.

Returns a summary with input and output record counts, sample count, and
warnings.

## Examples

```bash
linxira-bio variant to-table tests/fixtures/variant-stats/mixed.vcf output.tsv --json
```

## Interpretation

Each row corresponds to a VCF record. ALT alleles are kept as the original
comma-separated string. INFO is the raw value. Sample columns are the raw
FORMAT:values text.

## Caveats

This capability does not validate genotypes, normalize alleles, split
multiallelic records, or parse INFO fields. It streams the original VCF
columns into a tabular layout.

## Runtime Dependencies

Pure Rust streaming local capability with no external dependencies.

## Citations

VCF field semantics follow the GA4GH VCF specification.

## Troubleshooting

Use the reported line for malformed headers or records. Convert BCF to VCF with
a maintained tool such as bcftools before using this capability.