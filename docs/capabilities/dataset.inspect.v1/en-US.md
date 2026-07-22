# Dataset inspection

## Purpose

Identify a local biological file by content and compression signature, perform
a bounded validation pass, and return a safe preview before analysis.

## Inputs

`inputs.file` is one local regular file. Supported imports are FASTA, FASTQ,
CSV, TSV, BED, GFF3, GTF, VCF, and SAM. BAM, ZIP, BCF, CRAM, H5AD, LOOM, HDF5,
RDS, PDB, and mmCIF are recognized but not validated or executable in this release.

## Parameters

`max_preview_records` defaults to 200. `max_preview_bytes` defaults to 10 MiB.
Both must be positive integers.

## Outputs

The JSON result reports path, size, detected format, compression, support,
confidence, preview columns and records, warnings, and structured errors.

## Examples

```bash
linxira-bio dataset inspect reads.fastq.gz --json
```

## Interpretation

Continue to analysis only when `support` is `supported` and `errors` is empty.
Content has priority over a conflicting extension.

## Caveats

The preview is sampled and does not replace full validation during analysis.
ZIP archives are not extracted. HDF5 subtypes cannot always be distinguished
without domain metadata.

## Runtime Dependencies

The capability is built into the local Rust worker and needs no Python, R,
Java, container, or network service.

## Citations

Format semantics follow the public FASTA/FASTQ conventions, UCSC BED,
GFF3/GTF, VCF 4.x, and SAM/BAM specifications.

## Troubleshooting

For `format-extension-mismatch`, verify the file source and use the detected
content format. For `recognized-unsupported-format`, use a maintained external
tool or wait for the corresponding capability rather than forcing import.
