# Variant Annotation

## Purpose

Annotate VCF variants with functional consequence predictions using a
reference database such as Ensembl VEP.

## Inputs

A VCF file with variant calls to annotate.

## Parameters

`--database` selects the annotation database (e.g., `GRCh38.99`).

## Outputs

An annotated VCF file with added INFO field annotations. JSON result
wraps the native tool execution metadata.

## Examples

```bash
linxira-bio variant annotate input.vcf output.vcf --database GRCh38.99 --json
```

## Interpretation

Review the added annotations for functional impact predictions, gene
consequences, and population frequencies where available.

## Caveats

Requires the annotation database to be locally available. Annotation
accuracy depends on the database version and completeness. Large VCF
files may require significant processing time.

## Runtime Dependencies

Ensembl VEP or equivalent annotation tool. Set `LINXIRA_BIO_VEP` to
override the binary path.

## Citations

McLaren W, et al. The Ensembl Variant Effect Predictor. Genome Biol.
2016;17(1):122.

## Troubleshooting

If the annotation database is not found, verify the database path and
version. Ensure the VCF contig names match the reference used by the
annotation database. Existing output files are never overwritten.