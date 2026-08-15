# Phylogenetic Tree Visualization

## Purpose

Render a Newick-format phylogenetic tree as a rectangular cladogram SVG image.

## Inputs

A Newick-format tree file (plain or gzip-compressed) with at least 2 leaves.

## Parameters

`--width` and `--height` control the output image dimensions (200–4096, default 800×600).
`--font-size` sets the leaf label font size (6–48, default 14).
`--no-branch-lengths` draws a uniform cladogram without scaling by branch lengths.

## Outputs

An SVG image with labeled leaves and branch lines. JSON result wraps the
visualization metadata including leaf count and dimensions.

## Examples

```bash
linxira-bio phylogeny tree-plot tree.nwk tree.svg --json
linxira-bio phylogeny tree-plot tree.nwk tree.svg --width 1200 --height 800 --no-branch-lengths --json
```

## Interpretation

The tree is drawn left-to-right with leaf labels on the right. Branch lengths
are scaled proportionally when present. Internal nodes are drawn as connecting
lines. The visualization is a cladogram (not a phylogram with a scale bar).

## Caveats

The tree must have at least 2 leaves and at most 1,000,000 nodes. The Newick
file must not exceed 128 MiB decompressed. Only rectangular style is supported.

## Runtime Dependencies

Pure Rust implementation; no external tools required.

## Citations

No external citations required for the rendering algorithm.

## Troubleshooting

Verify that the input file is valid Newick format. If the tree does not render,
check that each leaf has a label and that the file ends with a semicolon.