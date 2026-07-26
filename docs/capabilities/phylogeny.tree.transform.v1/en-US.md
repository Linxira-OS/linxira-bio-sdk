# Phylogeny Tree Transform

## Purpose

Validate, normalize, relabel, summarize, and single-leaf-reroot a local Newick tree.

## Inputs

One plain or gzip Newick tree containing at most 1,000,000 nodes.

## Parameters

`output` is required; optional `reroot_label` selects one leaf and optional `label_map` maps old labels to new labels.

## Outputs

Writes normalized Newick and returns topology counts, depth, total branch length, transform counts, path, and warnings.

## Examples

```bash
linxira-bio phylogeny tree input.nwk output.nwk --reroot outgroup --label-map labels.tsv --json
```

## Interpretation

Rerooting divides the selected leaf edge length equally across the two new root branches.

## Caveats

Only a unique single-leaf outgroup is supported; the capability does not infer topology or render a figure.

## Runtime Dependencies

Local Rust only; no alignment, tree-inference program, or network service is required.

## Citations

Input and output use the Newick parenthesis notation with quoted-label and comment support.

## Troubleshooting

Use a unique leaf label, avoid duplicate mapped labels, and choose an output path that does not already exist.
