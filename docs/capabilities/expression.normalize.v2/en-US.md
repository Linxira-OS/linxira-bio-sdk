# Expression Length Normalization

## Purpose

Normalize a complete local CSV or TSV bulk-expression count matrix to TPM,
FPKM, or RPKM using a per-feature length table in base pairs.

## Inputs

Two inputs are required. The matrix's first column contains unique feature
identifiers and remaining columns contain finite, non-negative count values.
The length table has exactly two columns: feature identifier and length in bp.
Plain and gzip-compressed inputs are read.

## Parameters

Select `--method tpm|fpkm|rpkm` and pass the length table with `--lengths`.
Every matrix feature must have a positive finite length; identifiers missing
from the length table are rejected and unused identifiers warn. Input and
output paths must differ.

## Outputs

Writes a TSV matrix preserving feature and sample order. JSON reports the
method, dimensions, length unit, length statistics (minimum, median, maximum,
zero-count features), per-sample input/output totals, scale factors, and
warnings.

## Examples

```bash
linxira-bio expression normalize counts.tsv tpm.tsv --method tpm --lengths feature-lengths.tsv --json
```

## Interpretation

TPM columns each sum to one million and compare composition across samples.
FPKM and RPKM are the same calculation under historical single-end and
paired-end names; both normalize for length and library size.

## Caveats

Missing lengths, non-positive lengths, zero library sizes, and samples whose
per-kilobase rates are all zero are rejected. Values are normalized rates, not
counts; do not feed them to count-based statistical models. Local analysis is
capped at 10 million matrix cells.

## Runtime Dependencies

The implementation is local Rust and requires no Python, R, or Java runtime.

## Citations

When reporting TPM, cite Wagner et al. 2012 (Theory Biosci. 111:1-21). When
reporting FPKM or RPKM, cite Mortazavi et al. 2008 (Nat. Methods 5:621-628)
and Trapnell et al. 2010 (Nat. Biotechnol. 28:511-515).

## Troubleshooting

Run matrix QC first and resolve missing values and duplicate identifiers.
Confirm the length table covers every feature with positive lengths and that
length units are base pairs, not kilobases.
