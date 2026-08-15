---
name: analyze-metagenomics
description: Classify metagenomic reads against a local Kraken2 database, produce taxonomy abundance tables, and interpret clade versus taxon read counts. Use for shotgun or amplicon read taxonomic profiling, community composition summaries, or contamination screening against a curated reference database.
---

# Analyze Metagenomics

Inspect imported files before execution. Use the Rust capability; do not
reimplement read classification or abundance aggregation in Python or R.

## Choose a capability

- Use `metagenomics.classify.v1` to classify FASTA or FASTQ reads against a
  local Kraken2 database (`--database`) with controlled confidence and
  minimum-hit-group settings.

## Execute

```bash
linxira-bio metagenomics classify READS.fq ABUNDANCE.tsv --database /data/kraken2 --confidence 0.2 --minimum-hit-groups 2 --threads 4 --json
```

## Interpret

The abundance table reports `clade_count` (reads assigned to the taxon or any
descendant) and `taxon_count` (reads assigned exactly to the taxon) with rank
codes from Kraken2 (`R` root, `D` domain, `P` phylum, `C` class, `O` order,
`F` family, `G` genus, `S` species, `U` unclassified). The JSON envelope's
`classified_fraction` and `unclassified_reads` summarize overall assignment
success. Report fractions of classified reads, not raw counts, when comparing
samples with different sequencing depth. Keep controlled-access data local;
do not upload reads to public classification services without an approved
data-governance path.

## Caveats

Requires a locally installed Kraken2 executable and a compatible database
(`hash.k2d`, `opts.k2d`, `taxo.k2d`). Classification is read-level; it does
not assemble genomes or estimate strain abundance. Results depend on database
composition and confidence settings.
