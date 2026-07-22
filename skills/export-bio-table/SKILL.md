---
name: export-bio-table
description: Export structured Linxira Bio analysis results to CSV, TSV, JSON, JSONL, or XLSX with the local `table.export.v1` capability. Use when a user, workflow, SDK, spreadsheet, or downstream bioinformatics tool needs a stable table artifact instead of terminal output.
---

# Export Bio Table

Use the shared exporter instead of writing one-off CSV or spreadsheet code.

## Run

1. Require a valid JSON result object, array of objects, or two-dimensional
   array. An `AnalysisResult` envelope is accepted and its `result` value is
   used for tabular formats. JSONL accepts only an object or an array of
   objects.
2. Choose the output by consumer:
   - CSV is the default interoperable table.
   - TSV avoids comma ambiguity in traditional bioinformatics tools.
   - JSON preserves the complete structured result envelope.
   - JSONL writes one object per line for streaming consumers.
   - XLSX is for desktop spreadsheet users.
3. Run:

```bash
linxira-bio export table INPUT.json OUTPUT.csv --json
```

Change the output extension to `.tsv`, `.json`, `.jsonl`, or `.xlsx` as needed.
4. Preserve the returned output path, format, size, capability ID, and source
   result provenance.

## Boundaries

- Keep FASTA, FASTQ, BED, GFF, VCF, SAM, and BAM as domain formats when a
  downstream tool requires their semantics. Do not replace them with CSV.
- XLSX is limited to 1,048,576 rows and 16,384 columns including its header.
- Nested JSON values are encoded as JSON text in CSV, TSV, and XLSX cells.
- Export does not upload data and does not require Python, R, Java, or network
  access.
