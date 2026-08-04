# Nearest Genomic Interval Lookup

## Purpose

Find one deterministic nearest target BED interval for each query interval on
the same contig.

## Inputs

A query BED file and a target BED file. Plain and gzip streams are detected by
magic bytes. BED3 or wider records are accepted; extra fields are ignored.

## Parameters

`<query.bed>`, `<target.bed>`, and `<output.tsv>` are required. `--json`
returns the standard result envelope.

## Outputs

A headered TSV contains query BED3, target BED3, non-negative distance, and
`upstream`, `downstream`, or `overlap`. JSON reports matched and unmatched
queries plus per-contig counts.

## Examples

```bash
linxira-bio interval closest variants.bed genes.bed nearest-genes.tsv --json
```

## Interpretation

Coordinates use zero-based, half-open `[start, end)` semantics. Overlaps have
distance zero. Bookended intervals also have distance zero but remain
directional. Distance ties choose the smallest target `(start, end)`.

## Caveats

Only one target is returned per query. Strand, names, scores, extra BED fields,
all-ties reporting, and reference-assembly validation are not implemented.

## Runtime Dependencies

Local Rust only; no Python, R, Java, bedtools, or network access is required.

## Citations

Coordinate semantics follow the UCSC BED specification. Report the reference
assembly and the query and target feature sources.

## Troubleshooting

Confirm both files use the same assembly, coordinate convention, and contig
naming. A query with no target on its exact contig is reported as unmatched.
