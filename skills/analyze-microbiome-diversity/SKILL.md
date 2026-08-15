---
name: analyze-microbiome-diversity
description: Classify metagenomic reads with local Kraken2 and summarize alpha diversity (species richness, Shannon index, evenness) and dominant species for research-only microbiome profiling.
---

# Analyze Microbiome Diversity

Inspect imported files before execution. Use the Rust capability; do not
reimplement read classification or diversity math in Python or R.

## Choose a capability

- Use `medical.microbiome.v1` to classify FASTA/FASTQ reads against a local
  Kraken2 database and report species richness, Shannon index, Pielou
  evenness, and the dominant species-level taxa with the full abundance table.

## Execute

```bash
linxira-bio medical microbiome READS.fq ABUNDANCE.tsv --database /data/kraken2 --confidence 0.2 --threads 4 --json
```

## Interpret

Report `species_richness`, `shannon_index`, and `evenness` together with
`classified_fraction`; higher Shannon/evenness indicate more even community
structure. Compare samples at equal depth or use fractions. This is
research-use-only: read-level classification does not assemble genomes,
estimate strain abundance, or support clinical microbiome conclusions. Keep
clinical samples local.

## Caveats

Requires a local Kraken2 executable and compatible database; results depend on
database composition and confidence settings. Same input caveats as
`metagenomics.classify.v1`.
