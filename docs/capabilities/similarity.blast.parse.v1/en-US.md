# BLAST Result Parsing

## Purpose

Parse completed BLAST tabular or legacy XML1 results into one deterministic local result.

## Inputs

One plain or gzip file in outfmt 6, outfmt 7 with `# Fields`, or legacy BLAST XML1 format.

## Parameters

No analysis parameters are accepted.

## Outputs

Returns format, hit/query/subject counts, score summaries, normalized hit records, and warnings.

## Examples

```bash
linxira-bio similarity blast-parse results.tsv --json
```

## Interpretation

Coordinates and scores preserve the source report. Identity is reported as a percentage.

## Caveats

Parsing does not run a search, assess database completeness, or establish homology by itself.

## Runtime Dependencies

Local Rust only; the completed result file must already exist.

## Citations

Field meanings follow the documented BLAST tabular and legacy XML1 result conventions.

## Troubleshooting

Use standard outfmt 6 columns, retain the outfmt 7 `# Fields` declaration, or export legacy XML1 rather than XML2.
