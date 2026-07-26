# Exact Venn-Region Analysis

## Purpose

Count exact membership regions for two to six biological identifier sets.

## Inputs

A local CSV or TSV table whose header names the sets. Each non-empty cell is one identifier in that column's set. Duplicate identifiers within a set are deduplicated.

## Parameters

Use `--include-items` only when the result must contain the identifiers assigned to each exact region. Counts are always returned.

## Outputs

JSON reports each set size, union size, and every observed exact intersection. An exact intersection contains items present in precisely the listed sets.

## Examples

```bash
linxira-bio set venn sets.tsv --json
```

## Interpretation

Distinguish exact regions from inclusive pairwise intersections. For example, the `A ∩ B` region excludes identifiers also present in `C`.

## Caveats

Venn analysis is limited to six columns. Input is capped at one million rows and one million unique identifiers. Items are omitted by default to keep JSON bounded.

## Runtime Dependencies

Parsing, deduplication, and exact membership counting run in local Rust.

## Citations

Cite the source and filtering rules used to define each biological set.

## Troubleshooting

Ensure the first row contains unique non-empty set names and use the correct `.csv` or `.tsv` extension.
