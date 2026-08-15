# Metagenomic Taxonomic Classification

## Purpose

Classify reads against a Kraken2 reference database and produce a taxonomy abundance table (clade and taxon read counts with fractions) for metagenomic community profiling.

## Inputs

A FASTA or FASTQ file containing reads. The classification database is selected with `--database` and must be a directory produced by `kraken2-build` or the standard Kraken2 database layout (`hash.k2d`, `opts.k2d`, `taxo.k2d`).

## Parameters

- `--database <dir>` (required): Kraken2 database directory.
- `--confidence <fraction>`: minimum confidence threshold (0–1, default 0.0).
- `--minimum-hit-groups <n>`: minimum number of hit groups for a confident assignment (default 2).
- `--threads <n>`: worker threads (default 1).

## Outputs

A TSV abundance table with columns `percentage`, `clade_count`, `taxon_count`, `rank`, `taxon_id`, `name`, one row per taxonomy node reported by Kraken2. JSON output additionally reports `total_reads`, `classified_reads`, `unclassified_reads`, `classified_fraction`, and `taxon_count`.

## Examples

```bash
linxira-bio metagenomics classify reads.fq abundance.tsv --database /data/kraken2-db --confidence 0.2 --threads 4 --json
```

## Interpretation

`clade_count` includes every read assigned to the taxon or any descendant; `taxon_count` counts reads assigned exactly to the taxon. `rank` follows Kraken2 codes (`R` root, `D` domain, `P` phylum, `C` class, `O` order, `F` family, `G` genus, `S` species, `U` unclassified). The `U` row reports unclassified reads.

## Caveats

Requires a Kraken2 executable and a compatible reference database; results depend on database composition and confidence settings. Classification is read-level and does not assemble genomes or estimate strain abundance. Databases must be licensed for local use; keep controlled-access data local.

## Runtime Dependencies

Kraken2 (2.x). Install via Bioconda (`conda install -c bioconda kraken2`) or build from the Kraken2 repository.

## Citations

Wood, D.E., Lu, J., & Langmead, B. (2019). Improved metagenomic analysis with Kraken 2. Genome Biology, 20:257.

## Troubleshooting

If Kraken2 is not found, install it through Bioconda or your package manager. Verify the `--database` directory contains `hash.k2d`, `opts.k2d`, and `taxo.k2d`. Confidence values outside 0–1 and thread counts outside 1–1024 are rejected.
