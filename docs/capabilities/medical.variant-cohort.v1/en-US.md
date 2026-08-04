# Research Cohort Variant Aggregation

## Purpose

Summarize a local multi-sample VCF for research: variants by contig, called and missing genotypes, alternate-allele copies, and carrier genotypes. It does not classify pathogenicity, diagnose, or recommend treatment.

## Inputs

One standards-conformant multi-sample VCF or gzip-compressed VCF.

## Parameters

The input path is required. Genotypes are read from the `GT` FORMAT field.

## Outputs

Record, sample, contig, genotype missingness, carrier-genotype, and alternate-allele statistics.

## Examples

```text
linxira-bio medical variant-cohort cohort.vcf --json
```

## Interpretation

These are descriptive cohort statistics, not evidence of clinical impact for an individual or variant.

## Caveats

Interpretation depends on VCF normalization, filters, sample selection, and reference build.

## Runtime Dependencies

Local Rust only; data is not uploaded.

## Citations

Cite the reference genome, caller, filters, and cohort protocol.

## Troubleshooting

Ensure the VCF header and GT fields follow the VCF specification.
