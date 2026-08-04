# fastq.deduplicate.v1

## Purpose

Remove exact duplicate FASTQ reads locally and optionally include a strict UMI
in the duplicate key.

## Inputs

- One plain, gzip, or BGZF FASTQ file.

## Parameters

- `output`: required FASTQ output path; existing files are not overwritten.
- `header_umi_delimiter`: use the suffix after the final delimiter in the read
  identifier as the UMI.
- `sequence_prefix_umi`: use this many leading sequence bases as the UMI and
  the remaining bases as the insert. Choose at most one UMI source.

## Outputs

- A normalized four-line FASTQ containing the first read for every exact key.
- JSON counts for input, output, duplicate reads, bases, strategy, and warnings.

## Examples

```bash
linxira-bio fastq deduplicate reads.fastq.gz unique.fastq --json
linxira-bio fastq deduplicate reads.fastq umi-unique.fastq \
  --header-umi-delimiter : --json
```

## Interpretation

Sequence matching is case-insensitive. Without a UMI, the sequence is the key.
With a UMI, both the UMI and insert sequence must match exactly.

## Caveats

This is single-file exact deduplication. It does not correct UMI errors, cluster
nearby UMIs, select a consensus or highest-quality representative, synchronize
paired reads, or deduplicate mapped fragments by coordinates.

## Runtime Dependencies

Pure local Rust capability; no Python, R, Java, or external FASTQ tool is used.

## Citations

Record the strategy, UMI extraction rule, input hash, output path, and result
JSON. Do not compare duplicate rates across libraries with different protocols.

## Troubleshooting

- A missing or empty header UMI fails instead of silently using a wrong key.
- A sequence-prefix UMI must leave at least one insert base.
- Choose a new output path when the destination already exists.
