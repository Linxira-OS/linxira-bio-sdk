# Microbiome Diversity Analysis

## Purpose

Classify metagenomic reads with Kraken2 (shared with `metagenomics.classify.v1`) and compute alpha-diversity summaries (species richness, Shannon index, Pielou evenness) plus the dominant species-level taxa for research-only microbiome profiling.

## Inputs

A FASTA or FASTQ file containing reads and a Kraken2 database directory (`--database`).

## Parameters

- `--database <dir>` (required): Kraken2 database directory.
- `--confidence <fraction>`: minimum confidence threshold (0–1, default 0.0).
- `--minimum-hit-groups <n>`: minimum hit groups for a confident assignment (default 2).
- `--threads <n>`: worker threads (default 1).

## Outputs

A TSV taxonomy abundance table (same layout as `metagenomics.classify.v1`). JSON output reports classification totals plus `species_richness`, `shannon_index`, `evenness`, and `top_species` (top 5 species by read count with fractions).

## Examples

```bash
linxira-bio medical microbiome reads.fq abundance.tsv --database /data/kraken2-db --confidence 0.2 --threads 4 --json
```

## Interpretation

The Shannon index is computed over species-rank (`S`) taxon counts; evenness is Shannon divided by the natural log of richness. Richness of 0 or 1 yields an evenness of 0. `classified_fraction` and `unclassified_reads` summarize overall assignment success. Compare samples at equal sequencing depth or use rarefied fractions.

## Caveats

Requires a locally installed Kraken2 executable and compatible database. Research-use-only: read-level classification does not assemble genomes, estimate strain abundance, or replace clinical microbiome diagnostics. Diversity depends on database composition and confidence settings.

## Runtime Dependencies

Kraken2 (2.x), installable via Bioconda (`conda install -c bioconda kraken2`).

## Citations

Wood, D.E., Lu, J., & Langmead, B. (2019). Improved metagenomic analysis with Kraken 2. Genome Biology, 20:257.

## Troubleshooting

If no species are detected, check the database content and confidence settings, and confirm the reads are appropriate for the database (shotgun reads for nucleotide databases). Confidence outside 0–1 or threads outside 1–1024 are rejected.
