---
name: analyze-sequence-similarity
description: Parse local BLAST outfmt 6, outfmt 7, or legacy XML1 result files and calculate deterministic reciprocal best-hit pairs. Use for similarity-result QC, top-hit inspection, orthology candidate tables, or two-direction reciprocal searches when the search results already exist locally.
---

# Analyze Sequence Similarity

Inspect imported files before execution. Use the Rust capabilities; do not rewrite BLAST parsing or best-hit ranking in Python or R.

## Choose a capability

- Use `similarity.blast.parse.v1` to parse BLAST outfmt 6, outfmt 7 with `# Fields`, or legacy BLAST XML1.
- Use `similarity.reciprocal.v1` with `forward` and `reverse` result files to find reciprocal best hits.
- Do not claim that these capabilities run a similarity search. Use an installed local search engine for database creation and searching, then parse its output here.

## Execute

```bash
cargo run -p linxira-bio-cli -- similarity blast-parse INPUT.tsv --json
cargo run -p linxira-bio-cli -- similarity rbh FORWARD.tsv REVERSE.tsv --max-evalue 1e-5 --min-identity 30 --json
```

For worker schema v2, use role `blast` for parsing, or roles `forward` and `reverse` for reciprocal analysis. Optional reciprocal parameters are `max_evalue` and `min_identity_percent`.

## Interpret and validate

- Rank hits by e-value ascending, bit score descending, identity descending, alignment length descending, then subject identifier.
- Treat coordinates as values reported by the source result; reverse-orientation coordinates are valid.
- Report parser warnings, filtering thresholds, unpaired counts, and ties resolved by the deterministic ranking.
- Reject malformed records, invalid coordinates, non-finite numeric values, unsupported XML variants, or more than 2,000,000 hits.

## Limits

Execution is local CPU and accepts at most 256 MiB of decompressed text. It does not run BLAST, construct databases, infer orthology beyond reciprocal best-hit criteria, or upload sequence data.
