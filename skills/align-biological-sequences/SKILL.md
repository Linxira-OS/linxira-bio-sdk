---
name: align-biological-sequences
description: Run verified local multiple-sequence alignment capabilities on biological FASTA data. Use when an agent must align nucleotide or protein sequences with MUSCLE, choose the standard or large-dataset mode, preserve a deterministic output artifact, or verify that the required native executable is available.
---

# Align Biological Sequences

Inspect the FASTA input before execution. Use the versioned Rust capability to
control the native tool; do not generate an alignment implementation in Python
or R.

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

## Limits

Execution is local CPU and requires MUSCLE 5 on `PATH` or
`LINXIRA_BIO_MUSCLE`. The capability does not trim alignments, infer trees, or
silently install software. Use `configure-bio-environment` when MUSCLE is
missing.
