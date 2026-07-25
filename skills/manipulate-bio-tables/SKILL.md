---
name: manipulate-bio-tables
description: Filter, reshape, and export local biological CSV/TSV tables with the verified `table.manipulate.v1` capability. Use when Codex needs to select columns, drop columns, filter rows, skip rows, limit rows, convert CSV/TSV delimiters, or prepare rectangular biology tables for downstream tools without writing ad-hoc scripts.
---

# Manipulate Bio Tables

Use `table.manipulate.v1` for deterministic local row and column operations on rectangular CSV or TSV biology tables.

## Run

1. Inspect the input first when format, compression, or table shape is uncertain.
2. Choose the delimiter from content or extension. `.csv`, `.tsv`, `.tab`, `.csv.gz`, `.tsv.gz`, `.tab.gz`, `.bgz`, and `.bgzip` are supported; pass `--delimiter csv|tsv` when a compressed or extensionless file is ambiguous.
3. Select one column mode:
   - Use repeated `--select-column NAME` to keep only those columns in that order.
   - Use repeated `--drop-column NAME` to remove columns while preserving the rest.
   - Do not combine select and drop.
4. Add at most one string row filter:
   - `--filter-column NAME --filter-op equals --filter-value VALUE`
   - `--filter-column NAME --filter-op contains --filter-value VALUE`
   - `--filter-column NAME --filter-op non-empty`
5. Use `--skip-rows N` for initial data rows after the header and `--limit N` for maximum output rows.
6. Write to a new output path; the capability refuses to overwrite existing files.

```bash
linxira-bio table manipulate input.tsv output.tsv --select-column gene_id --select-column sample_a --filter-column sample_a --filter-op non-empty --json
```

## Worker Contract

Use input role `table`. Set `parameters.output` to the output CSV/TSV path. Optional parameters are `delimiter`, `output_delimiter`, `select_columns`, `drop_columns`, `filter_column`, `filter_op`, `filter_value`, `skip_rows`, and `limit`.

## Result Handling

Preserve the capability ID, output path, input/output row counts, skipped and filtered row counts, input/output column counts, selected and dropped columns, delimiters, warnings, and file hashes from worker v2 artifacts when present.

## Boundaries

- This is not expression-matrix QC; use `analyze-expression-matrix` for matrix missingness and numeric summaries.
- This does not implement joins, merges, group-by statistics, differential expression, sparse matrices, or database queries.
- Keep FASTA, FASTQ, BED, GFF/GTF, VCF, SAM/BAM, and PDB in their domain formats unless a downstream step explicitly needs a table.
- Do not upload data or use cloud services for this capability.
