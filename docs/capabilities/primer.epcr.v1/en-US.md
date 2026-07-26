# Simple Electronic PCR

## Purpose

Locate exact local amplicons from primer pairs and a reference FASTA.

## Inputs

A FASTA reference and a TSV with `id`, `forward`, and `reverse` columns.

## Parameters

Set minimum and maximum amplicon lengths and a safety limit with `--max-hits`.

## Outputs

A TSV containing primer ID, sequence ID, 1-based inclusive start/end, amplicon length, and strand, plus a summary.

## Examples

```bash
linxira-bio primer epcr reference.fa primers.tsv amplicons.tsv --max-amplicon 5000 --json
```

## Interpretation

The reverse primer is reverse-complemented before exact same-sequence amplicons are paired.

## Caveats

Only exact non-degenerate primers are supported. The result does not predict melting temperature, dimers, or experimental success.

## Runtime Dependencies

Local Rust only; no external aligner or runtime is required.

## Citations

The implementation follows standard in-silico PCR orientation and coordinate conventions.

## Troubleshooting

Confirm the primer table is tab-separated with all three required columns and contains only A/C/G/T/U primer symbols.
