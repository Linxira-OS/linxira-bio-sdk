---
name: analyze-variant-statistics
description: Run deterministic local Linxira Bio VCF statistics, basic filtering, and reference-guided biallelic small-variant normalization. Use for VCF summaries, QUAL/FILTER/contig/INFO-DP filtering, REF validation, minimal representation, and repeat-aware left alignment.
---

# Analyze Variant Statistics

Use the tested Rust capabilities for descriptive VCF summaries, basic record
filters, or reference-guided small-variant normalization. None clinically
interpret or annotate variants.

## Run

1. Confirm with `inspect-bio-dataset` that the local input is a supported VCF
   and has no structural inspection errors. BCF is not supported by this
   capability.
2. Run:

```bash
linxira-bio variant stats INPUT.vcf --json
linxira-bio variant filter INPUT.vcf OUTPUT.vcf --min-qual 20 --pass-only --min-info-dp 10 --json
linxira-bio variant normalize INPUT.vcf REFERENCE.fa OUTPUT.vcf --json
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

## Filter And Normalize Safely

- Treat missing QUAL or `INFO/DP` as failing the corresponding minimum filter.
- Preserve filter parameters and the output artifact hash with the result.
- Supply the exact matching reference build for normalization.
- Expect normalization to reject multiallelic, symbolic, breakend, and
  spanning-deletion ALT values; split or process them with a maintained native
  workflow before retrying.
- Do not treat filtering or normalization as pathogenicity evidence.
