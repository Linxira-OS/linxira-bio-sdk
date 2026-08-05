---
name: align-biological-sequences
description: Run verified local multiple-sequence alignment, trimming, and read-alignment capabilities on biological FASTA data. Use when an agent must align nucleotide or protein sequences with MUSCLE, trim an alignment with trimAl, align short reads with minimap2+samtools, align long reads (PacBio/ONT) with minimap2, preserve deterministic output artifacts, or verify the required native executables.
---

# Align Biological Sequences

Inspect the FASTA input before execution. Use the versioned Rust capability to
control the native tool; do not generate an alignment implementation in Python
or R.

## Long-Read Alignment

Align PacBio or Oxford Nanopore reads to a reference with minimap2:

```bash
linxira-bio alignment long-read REFERENCE.fa READS.fastq OUTPUT.sam --preset map-ont --threads 4 --json
```

Presets: `map-ont` (default), `map-pb`, `map-hifi`, `splice`, `asm5`, `asm10`,
`asm20`, `sr`. Use `--secondary` to output secondary alignments, and
`--max-secondary N` to limit them.

For worker v1, provide `inputs.reference`, `inputs.reads`, and
`parameters.output`. Optional parameters: `preset`, `threads`, `secondary`,
`max_secondary`. The capability is `alignment.long-read.v1`.

## Short-Read Alignment

```bash
linxira-bio alignment short-read REFERENCE.fa READS.fastq OUTPUT.bam --threads 4 --json
```

Capability `alignment.short-read.v1` uses minimap2 for alignment and samtools
for sorting. Inputs: `reference`, `reads`. Output: sorted BAM.

## Execute MUSCLE

Use `align` for ordinary inputs and `super5` when the dataset is too large for
the standard workflow:

```bash
linxira-bio msa muscle INPUT.fa OUTPUT.fa --mode align --threads 4 --json
linxira-bio msa muscle INPUT.fa OUTPUT.fa --mode super5 --threads 8 --json
```

For worker schema v2, use input role `fasta` and parameters `output`, `mode`,
and `threads`. The capability is `msa.muscle.v1`.

## Validate

- Confirm the output exists, differs from the input path, and is non-empty.
- Preserve tool identity, command arguments, input hashes, warnings, and the
  aligned FASTA artifact reported by the result envelope.
- Check sequence identifiers and compare aligned sequence counts with the
  input before downstream trimming or phylogenetic inference.

## Trim an alignment

```bash
linxira-bio msa trimal alignment.fa trimmed.fa --mode automated1 --json
```

Use capability `msa.trimal.v1` with input role `alignment`. Record the selected
heuristic and compare retained alignment length before inference.

## Limits

Execution is local CPU and requires MUSCLE 5 and/or trimAl on `PATH`, or their
`LINXIRA_BIO_*` overrides. These capabilities do not infer trees or silently
install software. Use `configure-bio-environment` when a tool is missing.
