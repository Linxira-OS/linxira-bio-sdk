# Pharmacogenomic Variant Interpretation

## Purpose

Interpret common pharmacogenomic (PGx) star alleles in a VCF against a built-in, offline allele table (GRCh38) and produce a tabular interpretation with allele consequences, phenotypes, and affected drugs.

## Inputs

A VCF file (optionally gzip-compressed) containing variant records. The built-in table matches the variant's chromosome, position, reference, and alternate allele.

## Parameters

No parameters are required.

## Outputs

A TSV interpretation table with columns `chrom`, `position`, `reference`, `alternate`, `rsid`, `gene`, `allele`, `consequence`, `phenotype`, `drugs`, `genotype` (hom-alt/het-alt/ref when the VCF carries sample genotypes). JSON output additionally reports `reference_build`, `record_count`, `matched_variant_count`, `allele_count`, `genes_affected`, `variants`, and `combined_phenotypes`.

## Examples

```bash
linxira-bio medical pharmacogenomics variants.vcf interpretation.tsv --json
```

## Interpretation

Records with a homozygous-reference genotype are excluded from matches (allele presence is required). When multiple alleles of CYP2C19 or CYP2D6 are present, a combined diplotype phenotype is reported (for example `CYP2C19*2/*3` → poor metabolizer). Other genes report allele-level consequences only.

## Caveats

Research-use-only: this is not clinical interpretation or prescribing advice. The built-in table covers a small, common set of GRCh38 star alleles and tag variants; absence of a match is not evidence of normal metabolizer status. Genotype calls are read directly from the VCF sample column and are not re-validated. Positions are GRCh38; variants from other builds must be lifted over first.

## Runtime Dependencies

None — the interpretation is a pure local Rust capability with an offline allele table.

## Citations

Reference facts compiled from public pharmacogenomic literature and database records; verify against the source genotype calls before any clinical use.

## Troubleshooting

If no alleles match, confirm the VCF uses GRCh38 coordinates and that the sample genotype column contains the alternate allele. Confirm the REF/ALT strings match the table exactly (strand and alleles are case-sensitive).
