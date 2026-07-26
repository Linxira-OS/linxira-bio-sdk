# Reciprocal Best Hits

## Purpose

Find deterministic reciprocal best-hit pairs from completed forward and reverse BLAST results.

## Inputs

Two plain or gzip BLAST result files, supplied as `forward` and `reverse`.

## Parameters

Optional `max_evalue` is non-negative; optional `min_identity_percent` is between 0 and 100.

## Outputs

Returns query counts, reciprocal pairs, unpaired counts, directional scores, identities, and warnings.

## Examples

```bash
linxira-bio similarity rbh forward.tsv reverse.tsv --max-evalue 1e-5 --min-identity 30 --json
```

## Interpretation

Hits rank by e-value, bit score, identity, alignment length, then subject identifier.

## Caveats

Reciprocal best hits are orthology candidates, not proof of one-to-one orthology or conserved function.

## Runtime Dependencies

Local Rust only; both directional searches must already be complete.

## Citations

The method implements the reciprocal-best-hit heuristic with an explicit deterministic tie order.

## Troubleshooting

Confirm that forward subject identifiers match reverse query identifiers and vice versa.
