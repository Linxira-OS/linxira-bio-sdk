# Synteny Anchor Visualization

## Purpose

Render a local SVG from an anchor TSV with `source_id`, `source_position`, `target_id`, and `target_position`. Layout styles are dual, multiple, micro, and circular.

## Inputs

One tab-separated anchor table.

## Parameters

The output SVG path is required.

## Outputs

An SVG with normalized anchor connections.

## Examples

```text
linxira-bio comparative synteny-plot anchors.tsv synteny.svg --style circular --json
```

## Interpretation

Each curve represents one supplied anchor; this renderer does not infer collinearity. The micro layout is a focused dual-track rendering of the supplied anchor subset.

## Caveats

Only the first 2,000 anchors are rendered.

## Runtime Dependencies

Local Rust only.

## Citations

Cite the upstream anchor or collinearity method.

## Troubleshooting

Use a tab-separated table with finite numeric positions.
