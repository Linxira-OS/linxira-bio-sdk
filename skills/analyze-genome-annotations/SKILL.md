---
name: analyze-genome-annotations
description: Analyze and transform local GFF3 or GTF genome annotations with executable statistics, normalization, position-table, and reference-guided sequence-extraction capabilities. Use for annotation QC, feature counts, GTF-to-GFF3 normalization, gene or transcript coordinate tables, and extraction of genes, transcripts, CDS, exons, UTRs, or promoters from a matching FASTA reference.
---

# Analyze Genome Annotations

Inspect imported data before execution. Use the deterministic Rust capabilities below; do not write replacement Python or R parsers.

## Choose a capability

- Use `annotation.gxf.stats.v1` for feature, sequence, source, strand, ID, and Parent counts.
- Use `annotation.gxf.normalize.v1` to convert valid GFF3/GTF records into canonical GFF3. Set `sort=true` only when coordinate ordering is required.
- Use `annotation.gene-position.v1` to emit a TSV coordinate table. The default feature type is `gene`; pass `feature_types` for transcripts or other records.
- Use `annotation.sequence.extract.v1` with both `annotation` and `fasta` inputs to extract `gene`, `transcript`, `cds`, `exon`, `utr`, `five_prime_utr`, `three_prime_utr`, or `promoter` sequences.

## Validate inputs

- Require nine tab-separated annotation columns and 1-based inclusive coordinates.
- Accept plain or gzip-compressed GFF3, GTF, and FASTA inputs.
- Confirm that annotation sequence identifiers match the first token of each FASTA header before extraction.
- Treat malformed records, duplicate FASTA identifiers, mixed-sequence feature groups, and existing output paths as hard failures.

## Execute through the worker

For schema v2, use role `annotation` for GFF3/GTF, role `fasta` for the reference, and parameter `output` for file-producing capabilities. Optional parameters are:

- `sort`: boolean for normalization.
- `feature_types`: string array for the position table.
- `feature_type`: extraction target.
- `promoter_length`: positive integer; default `1000`.

## Interpret results

- Keep coordinates as 1-based inclusive annotation coordinates.
- GFF3 normalization preserves parsed attributes but percent-encodes reserved separators.
- CDS extraction applies each segment phase before concatenation.
- Minus-strand multi-segment features are reverse-complemented into biological 5-prime to 3-prime orientation.
- Promoters are upstream of the annotated gene start on `+` and downstream of the gene end on `-`; extraction clips against available reference sequence.
- Report warnings and skipped or missing-reference counts. Do not present an empty output as a successful biological finding.

## Limits

The current implementation is deterministic local CPU execution. It does not infer missing parent-child relationships, repair invalid coordinates, download references, or overwrite an existing output.
