# VCF Variant Statistics

## Purpose

Compute a deterministic descriptive summary of VCF records, alternate alleles,
FILTER values, contigs, samples, and genotype missingness.

## Inputs

One readable VCF file with a valid `##fileformat` declaration and `#CHROM`
header. Plain text, gzip, and BGZF are detected by content. BCF is not accepted.

## Parameters

The input path is required. There are no scientific tuning parameters in v1.
`--json` returns the standard analysis result envelope.

## Outputs

Returns record and sample counts, PASS and named-filter counts, SNP, indel, MNV,
and symbolic ALT-allele counts, multiallelic record count, transition and
transversion counts and ratio, called and missing genotype counts and rate,
per-contig record counts, and warnings.

## Examples

```bash
linxira-bio variant stats tests/fixtures/variant-stats/mixed.vcf --json
```

## Interpretation

Record metrics count rows, while variant-class metrics count ALT alleles. A
multiallelic row can increment several classes. FILTER `.` is neither PASS nor
a named filtered value. Ti/Tv uses only biallelic single-base substitutions.
A genotype containing any missing allele is missing; records without a `GT`
field are excluded from the missingness denominator.

## Caveats

The capability does not normalize alleles, compare reference builds, evaluate
depth or likelihoods, annotate variants, assess pathogenicity, or validate
sample identity. Compare datasets only after equivalent preprocessing and
filtering.

## Runtime Dependencies

This is a streaming local Rust capability with no Python, R, Java, htslib, or
external command-line dependency.

## Citations

Field semantics follow the GA4GH VCF specification. The supported parser
expects VCF 4.x text records and reports unsupported or malformed structures.

## Troubleshooting

Use the reported line for malformed headers, records, FORMAT declarations, or
genotype allele indices. Convert BCF to VCF with a maintained tool such as
bcftools before using this capability.
