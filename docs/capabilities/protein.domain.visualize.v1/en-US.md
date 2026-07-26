# Protein Domain Architecture Visualization

## Purpose

Render parsed protein-domain hits as deterministic local SVG architecture tracks.

## Inputs

InterProScan TSV or supported HMMER domtblout data containing sequence coordinates.

## Parameters

Optionally select one sequence and bound the maximum number of sequences and domain glyphs.

## Outputs

An SVG artifact plus JSON metadata describing dimensions, sequence tracks, domain glyphs, path, and warnings.

## Examples

```bash
linxira-bio protein domain-plot interproscan.tsv domains.svg --sequence-id protein1 --json
```

## Interpretation

Each horizontal track is a protein coordinate span; colored blocks represent ordered domain hits with accession tooltips.

## Caveats

Overlapping calls are displayed rather than reconciled. Source databases and search thresholds determine biological meaning.

## Runtime Dependencies

Parsing and rendering run in local Rust without network access.

## Citations

Cite the search tool, domain database release, thresholds, source data, and capability version.

## Troubleshooting

Verify the input flavor is supported and that the requested sequence ID has at least one valid coordinate hit.
