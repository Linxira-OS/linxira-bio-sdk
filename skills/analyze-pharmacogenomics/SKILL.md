---
name: analyze-pharmacogenomics
description: Interpret common pharmacogenomic star alleles in VCF files against a local offline allele table with allele consequences, phenotypes, and affected drugs. Use for research-only PGx variant screening, metabolizer phenotype review, or drug-gene association lookup.
---

# Analyze Pharmacogenomics

Inspect imported files before execution. Use the Rust capability; do not
reimplement variant matching in Python or R.

## Choose a capability

- Use `medical.pharmacogenomics.v1` to interpret common GRCh38 PGx star
  alleles (CYP2C19, CYP2D6, SLCO1B1, VKORC1, TPMT, DPYD, HLA-B tag) in a VCF
  and produce a TSV interpretation table plus a JSON summary.

## Execute

```bash
linxira-bio medical pharmacogenomics VARIANTS.vcf INTERPRETATION.tsv --json
```

## Interpret

The table reports allele-level consequences and affected drugs per matched
variant; `genotype` (hom-alt/het-alt) comes from the VCF sample column and
homozygous-reference records are excluded. CYP2C19/CYP2D6 multi-allele
samples get a combined diplotype phenotype in `combined_phenotypes`. This is
research-use-only: report the exact alleles observed, not a prescribing
recommendation, and note the GRCh38 coordinate requirement. Keep clinical
data local; never upload VCFs to public interpretation services without an
approved data-governance path.

## Caveats

The built-in table covers a small common allele set; no match is not evidence
of normal metabolizer status. Genotype calls are read from the VCF without
re-validation. Coordinates must be GRCh38.
