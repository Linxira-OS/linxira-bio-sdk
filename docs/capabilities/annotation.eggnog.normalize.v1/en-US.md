# eggNOG Annotation Normalization

## Purpose

Normalize a completed eggNOG-mapper annotations table into stable local fields.

## Inputs

A standard tab-separated annotations file containing a `#query` header. Gzip input is accepted by content.

## Parameters

Provide the input and a new output TSV path. No database access or search parameters are accepted.

## Outputs

JSON and TSV preserve query IDs, seed orthologs, scores, groups, COG categories, descriptions, preferred names, GO, EC, KEGG orthologs, and KEGG pathways when present.

## Examples

```bash
linxira-bio annotation eggnog annotations.tsv normalized.tsv --json
```

## Interpretation

Counts summarize normalized queries and GO/KEGG assignments; missing source values remain empty rather than being inferred.

## Caveats

The capability normalizes completed results only. It does not run eggNOG-mapper or distribute its databases.

## Runtime Dependencies

Parsing and normalization run in local Rust after the result file exists.

## Citations

Cite the mapper version, database release, search settings, and input sequence source.

## Troubleshooting

Export the standard annotations table with its `#query` header and avoid manually deleting columns.
