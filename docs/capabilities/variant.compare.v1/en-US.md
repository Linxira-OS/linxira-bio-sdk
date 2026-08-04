# VCF Allele-Set Comparison

## Purpose

Compare two local VCF files and report shared, left-only, and right-only variant
alleles without uploading data.

## Inputs

Two valid VCF text files. Plain, gzip, and BGZF inputs are detected by magic
bytes. Header, record, FORMAT, and GT syntax is validated before comparison.

## Parameters

`<left.vcf>` and `<right.vcf>` are required. `--json` returns the standard
result envelope.

## Outputs

JSON reports the three set counts and a deterministic table ordered by CHROM,
POS, REF, and ALT. Each row is marked `shared`, `left-only`, or `right-only`.

## Examples

```bash
linxira-bio variant compare calls-a.vcf calls-b.vcf --json
```

## Interpretation

Multiallelic records are split into individual ALT keys and duplicate keys are
collapsed. Sequence alleles are uppercased and independently reduced to a
minimal representation. CHROM strings and symbolic ALT strings must match
exactly.

## Caveats

This capability does not compare samples, genotype calls, phasing, depth,
quality, FILTER, INFO, or clinical meaning. Minimal representation alone does
not make repeat-shifted indels equivalent; normalize both files against the
same reference first when that equivalence is required.

## Runtime Dependencies

Local Rust only; no Python, R, Java, htslib, or external executable is required.

## Citations

VCF syntax follows the GA4GH VCF specification. Record the exact reference
build and normalization method used for each compared file.

## Troubleshooting

Confirm reference build, contig naming, filtering, and normalization policy are
identical before interpreting discordance.
