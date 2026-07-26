# Exact k-mer Counting

## Purpose

Count exact FASTA k-mers locally with packed Rust keys and optional canonical reverse-complement collapsing.

## Inputs

One plain, gzip, or BGZF FASTA file.

## Parameters

Set `--k` from 1 to 31, add `--canonical` to merge reverse complements, and set `--top-n` for the JSON preview.

## Outputs

A complete TSV with `kmer` and `count` columns plus totals, distinct count, skipped ambiguous windows, and top k-mers.

## Examples

```bash
linxira-bio sequence kmer-count input.fa kmers.tsv --k 21 --canonical --top-n 50 --json
```

## Interpretation

`counted_windows` includes only A/C/G/T/U windows. U is normalized to T; ambiguous windows are reported separately.

## Caveats

This is exact counting, not a genome-size estimator, sequencing-error model, or approximate sketch.

## Runtime Dependencies

Local Rust only; no Python, R, Java, or external executable is required.

## Citations

Canonical counting uses the lexicographically smaller packed code of each k-mer and its reverse complement.

## Troubleshooting

Reduce `k` to 31 or less and confirm the input is valid FASTA. Existing output files are never overwritten.
