# Genome Feature Density

## Purpose

Calculate sliding-window annotation feature counts and features per megabase from local GFF3 or GTF.

## Inputs

One plain or gzip GFF3 or GTF annotation file.

## Parameters

`feature_types` defaults to `gene`; `window_size` and `step_size` default to 1,000,000 bases and must be positive.

## Outputs

Returns selected-feature counts and per-sequence windows with coordinates, counts, density, and warnings.

## Examples

```bash
linxira-bio annotation gene-density genes.gff3 --window-size 1000000 --step-size 250000 --json
```

## Interpretation

A feature contributes to every window that overlaps its 1-based inclusive interval.

## Caveats

Sequence lengths are inferred from the maximum annotation end coordinate, so terminal empty regions are unknown.

## Runtime Dependencies

Local Rust only; no reference FASTA, Python, R, Java, network, or external executable is required.

## Citations

Density is `feature_count * 1,000,000 / window_width` for each clipped window.

## Troubleshooting

Use exact annotation feature names and choose window and step sizes that keep the result below 2,000,000 bins.
