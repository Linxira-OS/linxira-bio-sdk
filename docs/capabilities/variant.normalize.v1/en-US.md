# Reference-guided VCF Normalization

## Purpose

Validate REF and normalize biallelic small variants to minimal, repeat-aware left-aligned representations.

## Inputs

One VCF text file and the exact matching reference FASTA. Both may be plain, gzip, or BGZF.

## Parameters

Input VCF, reference FASTA, and output VCF paths are required. `--json` returns the standard result envelope.

## Outputs

A normalized VCF plus counts for validated, changed, and left-aligned records.

## Examples

```bash
linxira-bio variant normalize input.vcf reference.fa normalized.vcf --json
```

## Interpretation

REF is checked against the named contig and coordinate before common suffix/prefix minimization and indel left alignment.

## Caveats

Multiallelic, symbolic, breakend, and spanning-deletion ALT values are rejected. The capability does not split alleles or remap genotypes.

## Runtime Dependencies

Local Rust only; no htslib or external normalization executable is required.

## Citations

Representation follows standard minimal-representation and repeat-aware left-alignment conventions for small variants.

## Troubleshooting

Reference contig names, build, and coordinates must match exactly. Process rejected complex records with a maintained native workflow.
