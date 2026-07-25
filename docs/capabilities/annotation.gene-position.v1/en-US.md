# annotation.gene-position.v1

## Purpose

Export selected annotation features as a stable coordinate and metadata table.

## Inputs

One valid plain or gzip-compressed GFF3/GTF annotation.

## Parameters

- `--feature-type TYPE`: select a feature type; repeat as needed. The default is `gene`.
- `--json`: emit a structured summary.

## Outputs

A new TSV with ID, name, sequence, start, end, strand, feature type, parent, and source columns.

## Examples

```bash
linxira-bio annotation positions input.gff3 genes.tsv --feature-type gene --json
```

## Interpretation

Coordinates remain 1-based inclusive. Review missing-identifier counts before using the table downstream.

## Caveats

Matching records without ID, gene_id, transcript_id, locus_tag, or Name are skipped. Existing outputs are not overwritten.

## Runtime Dependencies

Runs locally in the Rust core without external runtimes or network access.

## Citations

No external scientific method is introduced; the output is a deterministic projection of annotation records.

## Troubleshooting

If no rows are produced, confirm the requested feature-type spelling and available annotation attributes.
