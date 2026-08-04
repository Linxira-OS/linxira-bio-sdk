# Motif Sequence Logo

## Purpose

Render a local SVG sequence logo from a MEME motif matrix.

## Inputs

MEME text with `ALPHABET` and a finite `letter-probability matrix`.

## Parameters

The current version has no optional parameters.

## Outputs

SVG plus a JSON result envelope when requested.

## Examples

```text
linxira-bio motif logo motif.meme motif.svg --json
```

## Interpretation

Letter heights reflect the supplied position probabilities.

## Caveats

This renderer uses the first valid matrix and does not discover motifs.

## Runtime Dependencies

Local Rust only.

## Citations

Cite the motif-discovery method that generated the MEME matrix.

## Troubleshooting

Export standard MEME text with a matching alphabet and matrix row widths.
