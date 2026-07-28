# MEME Motif Discovery

## Purpose

Run local de novo motif discovery on nucleotide or protein FASTA sequences.

## Inputs

Provide a local nucleotide or protein FASTA.

## Parameters

Choose alphabet, occurrence model, motif count, width bounds, and CPU threads.

## Outputs

The capability copies MEME's canonical `meme.txt` result to the requested
output and records the controlled command metadata.

```bash
linxira-bio motif meme sequences.fa motifs.meme --alphabet dna --motifs 3 --json
```

## Examples

The command above discovers up to three DNA motifs with default width bounds.

## Interpretation

Review motif E-values, site counts, widths, and sequence composition together.

## Caveats

Motif significance depends on background composition, sequence selection, and
the occurrence model. This wrapper does not download databases or redistribute
MEME Suite.

## Runtime Dependencies

Requires `meme` on `PATH` or `LINXIRA_BIO_MEME`.

## Citations

Cite MEME Suite, its version, alphabet, occurrence model, and sequence source.

## Troubleshooting

Audit `meme`; configure `LINXIRA_BIO_MEME` only when it is outside `PATH`.
