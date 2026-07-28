---
name: analyze-sequence-motifs
description: Run verified local de novo motif discovery on nucleotide or protein FASTA files with MEME. Use when an agent must choose an alphabet and occurrence model, bound motif count and width, preserve canonical MEME text output, or verify the native motif-analysis environment.
---

# Analyze Sequence Motifs

Inspect the FASTA before execution and select the alphabet explicitly. Use the
versioned capability; do not rewrite MEME in Python or R.

## Execute

```bash
linxira-bio motif meme sequences.fa motifs.meme --alphabet dna --distribution zoops --motifs 3 --min-width 6 --max-width 15 --threads 4 --json
```

Worker v2 uses input role `fasta` and capability `motif.meme.v1`.

## Validate

- Preserve the input hash, alphabet, occurrence model, widths, motif count,
  tool version, and canonical MEME text artifact.
- Review motif E-values, site counts, sequence composition, and background
  assumptions together.
- Do not treat a discovered motif as a validated binding mechanism without
  independent evidence.

## Limits

Requires `meme` on `PATH` or `LINXIRA_BIO_MEME`. The wrapper neither downloads
databases nor redistributes MEME Suite. Use `configure-bio-environment` when
the executable is missing.
