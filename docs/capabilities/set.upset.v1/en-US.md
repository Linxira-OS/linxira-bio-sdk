# UpSet Analysis

## Purpose

Summarize exact intersections across two to 64 biological identifier sets without relying on an unreadable high-order Venn diagram.

## Inputs

A local CSV or TSV table whose columns are named sets and whose non-empty cells are identifiers.

## Parameters

Use `--max-intersections N` to bound the ranked result and `--include-items` to include identifiers. The default returns the 50 largest exact intersections without item lists.

## Outputs

JSON reports set sizes, union size, total observed intersection count, omitted count, and intersections ranked by count, degree, and set name.

## Examples

```bash
linxira-bio set upset sets.tsv --max-intersections 100 --json
```

## Interpretation

Use set-size bars and exact-intersection bars together. A high-degree intersection represents identifiers shared by many listed sets and by no unlisted set.

## Caveats

The input is capped at 64 columns, one million rows, and one million unique identifiers. Truncation affects display only; global set and intersection counts remain available.

## Runtime Dependencies

The capability runs entirely in local Rust.

## Citations

Cite the source and filtering rules used to construct each set.

## Troubleshooting

Increase `--max-intersections` when the result reports omitted intersections, or filter the input sets before analysis.
