---
name: analyze-sequence-motifs
description: Run verified local de novo motif discovery with MEME and motif occurrence scanning with MAST on nucleotide or protein FASTA files. Use when an agent must choose an alphabet and occurrence model, bound motif count and width, scan known motifs against sequences, or verify the native motif-analysis environment.
---

# Analyze Sequence Motifs

Inspect the FASTA before execution and select the alphabet explicitly. Use the
versioned capability; do not rewrite MEME or MAST in Python or R.

## Execute

### De novo discovery (MEME)

```bash
linxira-bio motif meme sequences.fa motifs.meme --alphabet dna --distribution zoops --motifs 3 --min-width 6 --max-width 15 --threads 4 --json
```

Worker v2 uses input role `fasta` and capability `motif.meme.v1`.

### Motif occurrence scanning (MAST)

```bash
linxira-bio motif mast motifs.meme sequences.fa hits.txt --evalue 1e-5 --hit-list --threads 4 --json
```

Worker v2 uses input roles `motif` and `sequences` and capability `motif.mast.v1`.

## Validate

- Preserve the input hash, alphabet, occurrence model, widths, motif count,
  tool version, and canonical MEME/MAST text artifact.
- Review motif E-values, site counts, sequence composition, and background
  assumptions together.
- Do not treat a discovered motif as a validated binding mechanism without
  independent evidence.
- For MAST, verify that the motif file is a valid MEME-format output and
  that the hit list E-values are consistent with the search threshold.

## Limits

Requires `meme` and `mast` on `PATH` or `LINXIRA_BIO_MEME` / `LINXIRA_BIO_MAST`.
The wrapper neither downloads databases nor redistributes MEME Suite.
Use `configure-bio-environment` when the executable is missing.
