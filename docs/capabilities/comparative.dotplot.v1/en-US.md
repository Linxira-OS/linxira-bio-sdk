# Comparative Genomics Dotplot

## Purpose

Generate a k-mer based dotplot SVG visualization comparing two FASTA sequences to identify regions of similarity, repeats, and structural rearrangements.

## Inputs

Two FASTA files: a query sequence and a reference sequence. Each file should contain at least one sequence record.

## Parameters

`--width` and `--height` control the output image dimensions (200–4096, default 800×800).
`--kmer` sets the k-mer size for matching (1–32, default 12).

## Outputs

An SVG dotplot image where each matching k-mer position is plotted as a point. JSON result wraps the visualization metadata including match count and dimensions.

## Examples

```bash
linxira-bio comparative dotplot query.fa reference.fa dotplot.svg --json
linxira-bio comparative dotplot query.fa reference.fa dotplot.svg --width 1200 --height 1200 --kmer 15 --json
```

## Interpretation

Each dot represents a k-mer match between the query (y-axis) and reference (x-axis). Diagonal lines indicate regions of similarity. Reverse diagonals indicate inversions. Gaps in diagonals indicate insertions or deletions.

## Caveats

Input sequences must be valid FASTA format. Very large genomes may produce dense plots that are hard to interpret. The k-mer size affects sensitivity and specificity: smaller k-mers find more matches but may include noise.

## Runtime Dependencies

Pure Rust implementation; no external tools required.

## Citations

No external citations required for the dotplot algorithm.

## Troubleshooting

If the dotplot is empty, try reducing the k-mer size. If the plot is too dense, increase the k-mer size. Ensure input files are valid FASTA format with standard nucleotide characters.