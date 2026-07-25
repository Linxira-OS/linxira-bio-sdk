# annotation.gxf.stats.v1

## Purpose

Summarize a local GFF3 or GTF annotation without modifying it.

## Inputs

- One valid nine-column GFF3 or GTF file using 1-based inclusive coordinates.
- Plain text and gzip-compressed input are supported.

## Parameters

- `--json`: emit the structured result envelope.

## Outputs

Counts for records, directives, sequence regions, feature types, sequence IDs, sources, strands, IDs, Parents, and the observed coordinate range.

## Examples

```bash
linxira-bio annotation stats input.gff3 --json
```

## Interpretation

Use feature and sequence counts to verify annotation scope before downstream extraction or normalization.

## Caveats

Malformed records fail the complete job. Statistics do not repair parent-child relationships.

## Runtime Dependencies

Runs locally in the Rust core without Python, R, Java, GPU, containers, or network access.

## Citations

No external scientific method is introduced; the capability implements deterministic GFF3/GTF parsing and descriptive counts.

## Troubleshooting

Check that every feature row has exactly nine tab-separated columns and valid coordinates, strand, and phase fields.
