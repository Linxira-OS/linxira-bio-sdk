# annotation.gxf.to-bed.v1

## Purpose

Convert selected annotation features from GFF3 or GTF format to BED6 format.

## Inputs

One valid plain or gzip-compressed GFF3/GTF annotation.

## Parameters

- `--feature-types LIST`: comma-separated feature types to convert; each type is matched case-insensitively. The default is `gene`.
- `--json`: emit a structured summary.

## Outputs

A new BED6 file with chrom, start (0-based), end, name, score, and strand columns. The name field is taken from the first available attribute among ID, Name, gene_id, transcript_id, or locus_tag.

## Examples

```bash
linxira-bio annotation to-bed input.gff3 output.bed --feature-types gene,exon --json
```

## Interpretation

BED coordinates are 0-based half-open. Review skipped-no-id counts before using the output downstream.

## Caveats

Matching records without a usable identifier are skipped. Existing outputs are not overwritten. Score values that do not parse as a float default to 0.

## Runtime Dependencies

Runs locally in the Rust core without external runtimes or network access.

## Citations

No external scientific method is introduced; the output is a deterministic projection of annotation records.

## Troubleshooting

If no rows are produced, confirm the requested feature-type spelling and available annotation attributes.