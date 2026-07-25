# annotation.sequence.extract.v1

## Purpose

Extract genes, transcripts, CDS, exons, UTRs, or promoters from a matching reference FASTA.

## Inputs

- One valid GFF3/GTF annotation.
- One FASTA whose header identifiers match annotation sequence IDs.
- Plain and gzip-compressed inputs are supported.

## Parameters

- `--feature-type`: `gene`, `transcript`, `cds`, `exon`, `utr`, `five_prime_utr`, `three_prime_utr`, or `promoter`; default `gene`.
- `--promoter-length N`: positive promoter length; default `1000`.
- `--json`: emit a structured summary.

## Outputs

A new FASTA plus matched, output, missing-reference, skipped-feature, and base counts.

## Examples

```bash
linxira-bio annotation extract genes.gff3 genome.fa cds.fa --feature-type cds --json
```

## Interpretation

Multi-segment features are concatenated, minus-strand results are reverse-complemented, and CDS phase is applied per segment.

## Caveats

The capability does not download references or infer missing parents. Promoters are derived from gene coordinates and clipped to the reference sequence. Existing outputs are not overwritten.

## Runtime Dependencies

Runs locally in the Rust core without Python, R, Java, GPU, containers, or network access.

## Citations

No external prediction method is used; extraction follows standard annotation coordinates and strand orientation.

## Troubleshooting

If outputs are missing, compare annotation sequence IDs with the first token of each FASTA header and review missing-reference and skipped-feature counts.
