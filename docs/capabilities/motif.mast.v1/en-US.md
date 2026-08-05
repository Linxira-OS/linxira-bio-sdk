# MAST Motif Scanning

## Purpose

Scan nucleotide or protein sequences for occurrences of motifs from a MEME-format
motif file using the MAST algorithm.

## Inputs

A MEME-format motif file and a FASTA file of sequences to scan.

## Parameters

`--evalue` sets the E-value threshold (default 1e-5). `--hit-list` outputs
a concise hit list. `--threads` controls parallel search threads.

## Outputs

MAST text output listing motif occurrences with positions, scores, and E-values.
JSON result wraps the native tool execution metadata.

## Examples

```bash
linxira-bio motif mast motifs.meme sequences.fa hits.txt --evalue 1e-5 --hit-list --threads 4 --json
```

## Interpretation

Review E-values and positions for each motif hit. Lower E-values indicate more
significant matches. Verify that motif occurrences are consistent with the
expected biological context.

## Caveats

The motif file must be valid MEME format. MAST requires the MEME Suite to be
installed. Sequence count and motif count affect runtime linearly.

## Runtime Dependencies

MEME Suite (mast executable). Set `LINXIRA_BIO_MAST` to override the binary path.

## Citations

Bailey TL, Gribskov M. Combining evidence using p-values: application to
sequence homology searches. Bioinformatics. 1998;14(1):48-54.

## Troubleshooting

Verify that the motif file is a valid MEME-format output. If MAST is not found,
install the MEME Suite or set `LINXIRA_BIO_MAST` to the correct binary path.