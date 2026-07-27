# Local HMMER Profile Search

## Purpose

Run local `hmmsearch` or `hmmscan` and retain deterministic domain-table output.

## Inputs

An HMMER profile or pressed profile database and a sequence FASTA appropriate for the selected mode.

## Parameters

Choose `hmmsearch` or `hmmscan`, CPU threads, and reporting e-value.

## Outputs

HMMER `--domtblout` text plus JSON execution metadata and Worker v2 hashes.

## Examples

```bash
linxira-bio similarity hmmer profile.hmm proteins.fa domains.domtblout --mode hmmsearch --json
```

## Interpretation

Use domain coordinates, independent e-values, scores, profile coverage, and source model provenance.

## Caveats

This wrapper does not build, press, download, or license profile databases. `hmmscan` requires a database prepared for that mode.

## Runtime Dependencies

Requires local HMMER `hmmsearch` or `hmmscan`; Windows normally uses an approved WSL provider unless a compatible executable is configured.

## Citations

Cite HMMER, profile database release, model accession, search mode, and thresholds.

## Troubleshooting

Audit `hmmer`; configure `LINXIRA_BIO_HMMSEARCH` or `LINXIRA_BIO_HMMSCAN` only when the executable is outside `PATH`.
