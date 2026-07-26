# Expression Normalization

## Purpose

Normalize a complete local CSV or TSV bulk-expression matrix with CPM,
log2-CPM, or median-ratio scaling.

## Inputs

The first column contains unique feature identifiers. Remaining columns contain
finite, non-negative sample values. Plain and gzip-compressed inputs are read.

## Parameters

Select `--method cpm|log2-cpm|median-ratio`. Log2-CPM accepts a non-negative
`--pseudocount`, defaulting to 1. Input and output paths must differ.

## Outputs

Writes a TSV matrix preserving feature and sample order. JSON reports the
method, dimensions, sample input/output totals, scale factors, and warnings.

## Examples

```bash
linxira-bio expression normalize counts.tsv normalized.tsv --method cpm --json
```

## Interpretation

CPM adjusts library size only. Median-ratio scaling reduces composition effects
when enough features are positive in every sample. Log2-CPM supports exploration.

## Caveats

Missing, duplicate-feature, negative, non-finite, and zero-library inputs are
rejected. Local numeric analysis is capped at 10 million matrix cells. Retain
raw counts for count-based statistical models.

## Runtime Dependencies

The implementation is local Rust and requires no Python, R, or Java runtime.

## Citations

When reporting median-ratio normalization, cite the downstream statistical
method whose normalization assumptions are being followed.

## Troubleshooting

Run matrix QC first. Resolve missing values and duplicate identifiers, and
ensure every sample has a positive total.
