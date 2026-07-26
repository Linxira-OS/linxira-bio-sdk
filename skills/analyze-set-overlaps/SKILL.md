---
name: analyze-set-overlaps
description: Run deterministic local exact-overlap analysis for biological identifier sets in CSV or TSV tables. Use for two-to-six-set Venn regions, multi-set UpSet summaries, set sizes, union sizes, ranked intersections, and optional intersection member lists.
---

# Analyze Set Overlaps

Use the tested Rust capabilities to count exact membership patterns without
uploading identifiers or generating analysis code.

## Run

1. Arrange the input as CSV or TSV: each column header is a unique set name;
   each non-empty cell is an identifier in that set. Duplicate identifiers
   within a column are deduplicated.
2. Use Venn only for two to six columns. Use UpSet for two to 64 columns:

```bash
linxira-bio set venn INPUT.tsv --json
linxira-bio set upset INPUT.tsv --max-intersections 50 --json
```

When developing in the source repository, prefix the command with
`cargo run -p linxira-bio-cli --`.

3. Add `--include-items` only when downstream work needs the member lists;
   counts are safer for large or controlled identifier sets.
4. Preserve the capability ID, CLI version, input hash, set definitions, and
   complete JSON result.

## Interpret

- Every reported intersection is exact: its identifiers occur in precisely
  the listed sets and in no other input set.
- `set_sizes` are deduplicated per input column; `union_size` counts distinct
  identifiers across all columns.
- UpSet intersections are ordered by count, then degree, then set names.
- `omitted_intersection_count` means the ranked display was truncated; it does
  not mean the input was partially analyzed.

Do not infer biological enrichment or significance from overlap counts alone.
Document how each set was filtered and use an appropriate background universe
for later statistical testing.
