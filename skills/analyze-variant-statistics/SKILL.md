---
name: analyze-variant-statistics
description: Compute and validate deterministic local VCF summary statistics with the Linxira Bio `variant.stats.v1` capability. Use for plain, gzip, or BGZF VCF record, FILTER, allele-class, contig, transition/transversion, sample, and genotype-missingness summaries.
---

# Analyze Variant Statistics

Use the tested streaming Rust capability for a descriptive VCF summary. It
does not normalize, filter, annotate, or clinically interpret variants.

## Run

1. Confirm with `inspect-bio-dataset` that the local input is a supported VCF
   and has no structural inspection errors. BCF is not supported by this
   capability.
2. Run:

```bash
linxira-bio variant stats INPUT.vcf --json
```

When developing in the source repository, run:

```bash
cargo run -p linxira-bio-cli -- variant stats INPUT.vcf --json
```

3. Preserve `variant.stats.v1`, CLI version, input hash, warnings, and the
   complete JSON result.
4. Stop on malformed headers, columns, alleles, FORMAT, or genotype indices.
   Do not reinterpret a parser failure as an empty result.

## Interpret

- `record_count`, `pass_record_count`, `filtered_record_count`, and
  `multiallelic_record_count` count VCF rows. FILTER `.` is neither PASS nor a
  named filtered row, so the FILTER counts need not sum to all rows.
- `snp_count`, `indel_count`, `mnv_count`, and `symbolic_count` count ALT
  alleles, not rows; a multiallelic row can increment several classes.
- `ti_tv_ratio` uses only biallelic single-base substitutions and is absent
  when no transversions are present.
- Genotypes with any missing allele are counted as missing. Records without a
  `GT` FORMAT field are excluded from the missingness denominator.
- `contig_counts` reflect CHROM strings and do not validate reference identity
  or contig lengths.

This summary does not establish call quality, pathogenicity, population
frequency, sample identity, or clinical meaning. Compare cohorts only after
normalization, reference-build checks, and equivalent filtering.
