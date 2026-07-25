# table.manipulate.v1

## Purpose

Filter and reshape local rectangular CSV or TSV biology tables without writing one-off scripts. The capability is intended for sample sheets, gene lists, count tables, annotation exports, and other delimited files that need column selection, column dropping, row filtering, row skipping, row limiting, or delimiter conversion.

## Inputs

- One CSV or TSV table with a header row.
- Plain text and gzip-compressed inputs are supported.
- `.csv`, `.tsv`, `.tab`, `.csv.gz`, `.tsv.gz`, `.tab.gz`, `.bgz`, and `.bgzip` extensions are recognized when possible.

## Parameters

- `--select-column NAME`: keep a column. Repeat to choose order.
- `--drop-column NAME`: remove a column. Repeat as needed.
- `--filter-column NAME`: column used by the row filter.
- `--filter-op equals|contains|non-empty`: string filter operation.
- `--filter-value VALUE`: value for `equals` and `contains` filters.
- `--skip-rows N`: skip the first N data rows after the header.
- `--limit N`: write at most N output data rows.
- `--delimiter csv|tsv`: override input delimiter detection.
- `--output-delimiter csv|tsv`: override output delimiter detection.
- `--json`: emit a structured result envelope.

## Outputs

- A new CSV or TSV output file. Existing files are not overwritten.
- A summary with input/output row counts, skipped and filtered rows, input/output column counts, selected and dropped columns, delimiters, and warnings.
- Worker v2 jobs also return a table artifact with format, media type, size, and SHA-256.

## Examples

```bash
linxira-bio table manipulate counts.tsv selected.tsv --select-column gene_id --select-column sample_a --limit 100 --json
```

```bash
linxira-bio table manipulate annotations.csv genes.tsv --filter-column type --filter-op equals --filter-value gene --output-delimiter tsv --json
```

## Interpretation

Use `input_rows`, `skipped_rows`, `filtered_rows`, and `output_rows` to confirm that the intended rows were retained. Use `selected_columns` and `dropped_columns` to verify the column projection before sending the table into downstream analysis.

## Caveats

- Filtering is string-based and supports one filter per run.
- Selection and dropping are mutually exclusive.
- Joins, merges, group-by statistics, expression-matrix QC, sparse tables, and statistical modeling are outside this capability.
- Use domain formats directly when downstream tools expect FASTA, FASTQ, BED, GFF/GTF, VCF, SAM/BAM, or PDB semantics.

## Runtime Dependencies

Runs locally in the Rust core. It does not require Python, R, Java, BLAST, DIAMOND, Docker, WSL, GPU, or network access.

## Citations

No external scientific method is introduced. The capability implements deterministic delimited-table I/O and row/column selection.

## Troubleshooting

- If delimiter detection fails, pass `--delimiter csv` or `--delimiter tsv`.
- If a column is not found, inspect the header row for spelling, whitespace, or duplicate names.
- If the output already exists, choose a new path or remove the old file intentionally before rerunning.
