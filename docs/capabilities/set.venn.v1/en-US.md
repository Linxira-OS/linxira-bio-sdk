# Exact Venn-Region Analysis

## Purpose

Count exact membership regions for two to six biological identifier sets.

## Inputs

A local CSV or TSV table whose header names the sets. Each non-empty cell is one identifier in that column's set. Duplicate identifiers within a set are deduplicated.

## Parameters

- `--include-items`: also return the identifiers assigned to each exact
  region (parameter name `include_items` on the worker contract). Counts are
  always returned.
- `--backend auto|rust|python|r`: the implementation backend. `rust` (and
  omission) runs the native engine; `python` and `r` route through the worker
  to the benchmark packs, which reproduce the exact same set arithmetic (no
  plotting library involved), so counts match exactly. `auto` consults
  `runtime-preferences.json`; a non-rust hit emits a
  `backend_from_preferences` warning.
- `--json`: print the full result envelope.

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

- `rust` (default): parsing, deduplication, and exact membership counting run
  in local Rust.
- `python` / `r`: the matching benchmark pack; both re-implement the set
  arithmetic with no third-party packages. No network access.

## Citations

Cite the source and filtering rules used to define each biological set.

## Troubleshooting

- Ensure the first row contains unique non-empty set names and use the correct `.csv` or `.tsv` extension.
- `unknown --backend value`: the flag accepts `auto`, `rust`, `python`, and
  `r` only.
