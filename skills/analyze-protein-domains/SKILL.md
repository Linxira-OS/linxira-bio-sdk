---
name: analyze-protein-domains
description: Parse local InterProScan TSV and HMMER domtblout protein-domain results into one validated structure. Use for Pfam, SMART, CDD, InterPro, HMM profile, GO-term, pathway, and protein-domain architecture result inspection after an external domain search has completed.
---

# Analyze Protein Domains

Inspect imported files before execution. Use `protein.domain.parse.v1`; do not rewrite the parser in Python or R.

## Execute

```bash
cargo run -p linxira-bio-cli -- protein domains INPUT.tsv --json
```

For worker schema v2, supply one input with role `domains`. No parameters are accepted.

## Interpret and validate

- InterProScan TSV unifies analysis sources such as Pfam, SMART, and CDD while preserving accessions, descriptions, GO terms, and pathways.
- Treat InterProScan column 9 as `score`; do not describe it as an e-value.
- HMMER domtblout preserves the reported domain e-value and score.
- Keep coordinates 1-based inclusive and verify them against sequence length when the format supplies one.
- Report source and accession counts together with every warning.
- Reject malformed rows, non-finite numbers, invalid coordinates, unsupported formats, or more than 2,000,000 hits.

## Limits

Execution is local CPU and accepts at most 256 MiB of decompressed text. It parses completed results; it does not run InterProScan, HMMER, Pfam, SMART, or CDD searches and does not download databases.
