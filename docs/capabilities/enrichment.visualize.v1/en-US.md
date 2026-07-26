# Enrichment Visualization

## Purpose

Render local bar, dot, or term-gene network SVG plots from custom, GO, or KEGG over-representation analysis.

## Inputs

A query identifier list and a CSV/TSV term-association table.

## Parameters

Choose `custom`, `go`, or `kegg`; choose `bar`, `dot`, or `network`; set minimum overlap and maximum terms.

## Outputs

An SVG artifact plus JSON metadata describing its style, dimensions, tracks, glyphs, path, and warnings.

## Examples

```bash
linxira-bio enrichment visualize genes.txt associations.tsv enrichment.svg --kind go --style dot --json
```

## Interpretation

Bar and dot plots rank adjusted significance; network plots connect reported terms to overlapping query genes.

## Caveats

Results depend on the supplied universe and associations. The visualization does not add ontology propagation or semantic reduction.

## Runtime Dependencies

Statistics and SVG rendering run in local Rust without network access.

## Citations

Cite the association release, query universe, enrichment method, correction method, and capability version.

## Troubleshooting

Confirm query identifiers occur in the association universe and use a compatible `--kind` for term identifiers.
