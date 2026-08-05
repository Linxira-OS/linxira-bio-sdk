# Consensus Sequence from Multiple Alignment

## Purpose

Compute a consensus sequence from a multiple sequence alignment (MSA) in FASTA format. For each column, the most frequent base (A, C, G, T, U, N) is selected if it meets the threshold fraction; otherwise N is used. Gap characters (`-`, `.`) are excluded from frequency counting.

## Inputs

One plain or gzip FASTA alignment file. All sequences must be the same length after removing gaps.

## Parameters

Set `--threshold` from 0.0 to 1.0 (default 0.5). The threshold determines the minimum fraction of sequences that must agree on a base before it is accepted; positions below the threshold are marked N.

## Outputs

A single FASTA record with ID `consensus` containing the consensus sequence. The JSON result includes input sequence count, alignment length, consensus length, ambiguous position count, GC content percentage, and any warnings.

## Examples

```bash
linxira-bio sequence consensus alignment.fa consensus.fa --threshold 0.5 --json
```

## Interpretation

`ambiguous_position_count` is the number of positions where no base met the threshold. A single-sequence input produces a warning but still yields a valid consensus (identical to the input sequence with gaps removed).

## Caveats

This is a simple majority-rule consensus. It does not use IUPAC ambiguity codes for ties or consider quality scores. All-gap columns are skipped, so `consensus_length` may be less than `alignment_length`.

## Runtime Dependencies

Local Rust only; no Python, R, Java, or external executable is required.

## Citations

Majority-rule consensus is a standard method in molecular phylogenetics and sequence analysis.

## Troubleshooting

Ensure all aligned sequences have the same ungapped length. Existing output files are never overwritten.