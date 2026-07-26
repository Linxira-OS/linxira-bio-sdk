# Protein Domain Result Parsing

## Purpose

Parse completed InterProScan TSV or HMMER domtblout domain annotations into one local structure.

## Inputs

One plain or gzip InterProScan TSV or HMMER domtblout file.

## Parameters

No analysis parameters are accepted.

## Outputs

Returns format, sequence and hit counts, source/accession counts, domain coordinates, annotations, and warnings.

## Examples

```bash
linxira-bio protein domains interproscan.tsv --json
```

## Interpretation

InterProScan column 9 is retained as `score`; HMMER domain e-values and scores retain their reported meanings.

## Caveats

The capability parses completed searches and does not judge domain significance beyond supplied scores.

## Runtime Dependencies

Local Rust only; search software and databases are not required for parsing.

## Citations

Columns follow the InterProScan TSV and HMMER domtblout format specifications.

## Troubleshooting

Preserve all tab-separated InterProScan columns or the full HMMER domtblout record.
