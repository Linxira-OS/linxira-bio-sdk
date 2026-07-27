---
name: analyze-sequence-similarity
description: Run controlled local BLAST+, DIAMOND, or HMMER searches, parse BLAST outfmt 6, outfmt 7, or legacy XML1 results, and calculate deterministic reciprocal best-hit pairs. Use for local nucleotide or protein similarity search, profile-HMM search, result QC, top-hit inspection, or reciprocal best-hit candidate tables.
---

# Analyze Sequence Similarity

Inspect imported files before execution. Use the Rust capabilities; do not
rewrite mature search algorithms, BLAST parsing, or best-hit ranking in Python
or R.

## Choose a capability

- Use `similarity.blast.local.v1` to build an isolated temporary BLAST+
  database from a reference FASTA and search it with `blastn`, `blastp`,
  `blastx`, `tblastn`, or `tblastx`.
- Use `similarity.diamond.v1` for local protein-reference `blastp` or `blastx`
  workflows.
- Use `similarity.hmmer.v1` for local `hmmsearch` or `hmmscan` and a HMMER3
  profile plus sequence FASTA.
- Use `similarity.blast.parse.v1` to parse BLAST outfmt 6, outfmt 7 with
  `# Fields`, or legacy BLAST XML1.
- Use `similarity.reciprocal.v1` with `forward` and `reverse` result files to find reciprocal best hits.

## Execute

```bash
linxira-bio similarity blast QUERY.fa REFERENCE.fa OUTPUT.tsv --program blastn --threads 4 --evalue 1e-3 --max-targets 50 --outfmt 6 --json
linxira-bio similarity diamond QUERY.fa PROTEINS.fa OUTPUT.tsv --mode blastp --threads 4 --evalue 1e-3 --max-targets 50 --outfmt 6 --json
linxira-bio similarity hmmer PROFILE.hmm SEQUENCES.fa OUTPUT.domtblout --mode hmmsearch --threads 4 --evalue 10 --json
cargo run -p linxira-bio-cli -- similarity blast-parse INPUT.tsv --json
cargo run -p linxira-bio-cli -- similarity rbh FORWARD.tsv REVERSE.tsv --max-evalue 1e-5 --min-identity 30 --json
```

For worker schema v2, use roles `query` and `reference` for BLAST+/DIAMOND,
`profile` and `sequences` for HMMER, `blast` for parsing, or `forward` and
`reverse` for reciprocal analysis.

## Interpret and validate

- Rank hits by e-value ascending, bit score descending, identity descending, alignment length descending, then subject identifier.
- Treat coordinates as values reported by the source result; reverse-orientation coordinates are valid.
- Report parser warnings, filtering thresholds, unpaired counts, and ties resolved by the deterministic ranking.
- Reject malformed records, invalid coordinates, non-finite numeric values, unsupported XML variants, or more than 2,000,000 hits.

## Limits

Execution is local CPU. Parsing accepts at most 256 MiB of decompressed text.
Search execution requires installed native tools and never uses a shell. It
does not download databases, infer orthology beyond reciprocal best-hit
criteria, install tools silently, or upload sequence data. Use
`configure-bio-environment` when BLAST+, DIAMOND, or HMMER is missing.
