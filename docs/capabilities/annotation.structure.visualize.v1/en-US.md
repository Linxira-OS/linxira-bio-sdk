# Annotation Structure Visualization

## Purpose

Render a deterministic local SVG of gene, transcript, exon, CDS, UTR, motif, or other GFF/GTF feature structure.

## Inputs

A validated GFF3 or GTF annotation file.

## Parameters

Optionally select a feature ID or sequence ID and bound the maximum number of rendered features.

## Outputs

An SVG artifact plus JSON metadata describing dimensions, tracks, glyphs, output path, and warnings.

## Examples

```bash
linxira-bio annotation plot genes.gff3 gene.svg --feature-id gene1 --json
```

## Interpretation

Feature coordinates are scaled within the selected locus; colors distinguish feature types and tooltips preserve labels and coordinates.

## Caveats

The plot summarizes annotation records and does not infer missing parent-child relationships. Existing outputs are never overwritten.

## Runtime Dependencies

Rendering runs in local Rust without network, Python, R, or browser dependencies.

## Citations

Cite the annotation source, genome assembly, coordinate convention, and this capability version.

## Troubleshooting

Confirm the requested feature or sequence exists and increase `--max-features` only for a bounded, readable locus.
