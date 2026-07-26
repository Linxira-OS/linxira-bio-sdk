---
name: visualize-bio-results
description: Create deterministic local SVG plots for GFF3/GTF annotation structure, custom/GO/KEGG enrichment results, and InterProScan/HMMER protein-domain architecture, or open PDB/mmCIF coordinates in the native interactive structure viewer. Use when an agent needs a reusable scientific figure artifact, bounded plot parameters, molecular representation selection, or a local PNG structure snapshot without writing plotting code or uploading biological data.
---

# Visualize Bio Results

Inspect imported inputs first. Use the versioned capabilities below instead of
writing a Python, R, browser, or ad hoc SVG script.

## Select the visualization

- GFF3/GTF gene or transcript structure: use
  `annotation.structure.visualize.v1`.
- Custom, GO, or KEGG enrichment bar, dot, or term-gene network: use
  `enrichment.visualize.v1`.
- Completed InterProScan TSV or HMMER domtblout domain architecture: use
  `protein.domain.visualize.v1`.
- Interactive PDB/mmCIF rotation, zoom, backbone, ball-and-stick,
  space-filling, confidence coloring, and PNG snapshot: use
  `structure.viewer.v1` in the native GUI.

## Execute

```bash
linxira-bio annotation plot INPUT.gff3 OUTPUT.svg --feature-id ID --json
linxira-bio enrichment visualize GENES.txt ASSOCIATIONS.tsv OUTPUT.svg --kind go --style bar --json
linxira-bio protein domain-plot DOMAINS.tsv OUTPUT.svg --sequence-id ID --json
linxira-bio-ui INPUT.pdb
```

When developing in this repository, prefix CLI commands with
`cargo run -p linxira-bio-cli --`. For Worker v2, use roles `annotation`,
`genes` plus `associations`, or `domains`; declare the output in `parameters.output`.

Bound annotation output with `max_features`, enrichment output with
`max_terms`, and domain output with `max_sequences` plus `max_domains`.
Choose only one of annotation `feature_id` and `seqid`. The renderer refuses
to overwrite an existing output.

## Validate and report

Verify that the SVG begins with an SVG root, the JSON status is `ok`,
`glyph_count` is nonzero, and warnings are reviewed. For Worker v2, preserve
the SVG artifact SHA-256, input hashes, media type `image/svg+xml`, capability
version, plot parameters, and output path.

Treat the plots as summaries of supplied annotations and analysis results.
Report database or annotation versions, background universe, coordinate
convention, thresholds, truncation limits, and every warning. Do not infer
missing feature relationships, reconcile overlapping domain calls, or turn an
enrichment plot or predicted structure into a causal or clinical conclusion.
