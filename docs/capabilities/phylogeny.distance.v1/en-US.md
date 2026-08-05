# Phylogeny Distance Matrix

## Purpose

Compute a pairwise distance matrix from a multiple sequence alignment (MSA) in FASTA format.

## Inputs

One FASTA multiple sequence alignment with all sequences of equal length, containing at least two sequences.

## Parameters

`output` is required; optional `model` selects the distance model (`p-distance`, `jc69`, or `k80`). Defaults to `p-distance`.

## Outputs

Writes a TSV file with columns `seq_a`, `seq_b`, and `distance` (full N×N matrix). Returns sequence count, alignment length, compared position count, model name, distance entries, and warnings.

## Examples

```bash
linxira-bio phylogeny distance alignment.fa distances.tsv --model p-distance --json
linxira-bio phylogeny distance alignment.fa distances.tsv --model jc69 --json
linxira-bio phylogeny distance alignment.fa distances.tsv --model k80 --json
```

## Interpretation

- `p-distance`: proportion of differing sites. Positions where both sequences are gaps are excluded from the denominator. Gaps compared to a character are treated as a difference.
- `jc69`: Jukes-Cantor correction: d = -3/4 × ln(1 - 4/3 × p). Produces `Infinity` when p ≥ 0.75.
- `k80`: Kimura 2-parameter correction using transition/transversion ratio. Produces `Infinity` when the correction formula saturates.

## Caveats

All sequences must be pre-aligned with equal length. The capability does not perform alignment itself — use `msa.muscle.v1` for alignment.

## Runtime Dependencies

Local Rust only; no external tools or network services required.

## Citations

Jukes TH, Cantor CR (1969). Evolution of protein molecules. *Mammalian Protein Metabolism*.

Kimura M (1980). A simple method for estimating evolutionary rates of base substitutions. *Journal of Molecular Evolution*.

## Troubleshooting

Ensure all sequences have the same length (alignment format). Use `--model` with one of: `p-distance`, `jc69`, `k80`. Infinity values in the output indicate saturated distance corrections.