# Local DIAMOND Search

## Purpose

Build an isolated DIAMOND protein database from a reference FASTA and run local `blastp` or `blastx` search.

## Inputs

A protein or translated-nucleotide query FASTA and a protein reference FASTA.

## Parameters

Choose `blastp` or `blastx`; set threads, e-value, maximum targets, and tabular outfmt 6 or 7.

## Outputs

A DIAMOND tabular result plus JSON execution metadata and Worker v2 artifact hashes.

## Examples

```bash
linxira-bio similarity diamond proteins.fa reference.fa hits.tsv --mode blastp --threads 8 --json
```

## Interpretation

Review alignment identity, coverage, e-value, and score together with database completeness.

## Caveats

The wrapper creates and removes a temporary database. It does not download protein databases or silently alter sensitivity presets.

## Runtime Dependencies

Requires a local DIAMOND executable discoverable in the managed environment.

## Citations

Cite DIAMOND, its version, the reference database source and release, mode, and thresholds.

## Troubleshooting

Audit the `diamond` tool and configure `LINXIRA_BIO_DIAMOND` when the executable is outside `PATH`.
