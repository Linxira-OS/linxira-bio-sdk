# Local BLAST+ Search

## Purpose

Build an isolated BLAST+ database from a local reference FASTA and run a local nucleotide or protein similarity search.

## Inputs

A query FASTA and a reference FASTA. Both remain local.

## Parameters

Choose `blastn`, `blastp`, `blastx`, `tblastn`, or `tblastx`; set threads, e-value, maximum targets, and tabular outfmt 6 or 7.

## Outputs

A BLAST tabular file plus JSON execution metadata and Worker v2 input/output hashes.

## Examples

```bash
linxira-bio similarity blast query.fa reference.fa hits.tsv --program blastn --threads 4 --json
```

## Interpretation

Interpret identity, alignment length, e-value, and bit score in the context of query and reference composition.

## Caveats

The temporary database is deleted after the search. The wrapper does not download reference databases and disables BLAST usage reporting.

## Runtime Dependencies

Requires local NCBI BLAST+ `makeblastdb` and the selected search executable. Windows may use a configured native executable or approved WSL environment.

## Citations

Cite NCBI BLAST+, the reference FASTA source and release, search program, and parameters.

## Troubleshooting

Run the environment audit for `ncbi-blast`; use `LINXIRA_BIO_MAKEBLASTDB` and the matching `LINXIRA_BIO_BLASTN`-style variable only for an explicitly configured executable.
