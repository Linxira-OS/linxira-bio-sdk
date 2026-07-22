# Table export

## Purpose

Export structured analysis results through one deterministic implementation
shared by the GUI, CLI, worker, and future SDK clients.

## Inputs

The input is a local JSON object, array of objects, or two-dimensional array.
For an `AnalysisResult` envelope, tabular exports use the nested `result` value.

## Parameters

The required output path ends in `.csv`, `.tsv`, `.json`, `.jsonl`, or `.xlsx`.
The extension selects the format.

## Outputs

The capability writes one local artifact and returns its format, path, and
size in bytes. JSON preserves the input; JSONL writes one object per line;
tabular formats normalize object keys into stable, alphabetically ordered
columns.

## Examples

```bash
linxira-bio export table result.json result.csv --json
linxira-bio export table result.json result.xlsx --json
```

## Interpretation

Use CSV for general interchange, TSV for command-line bioinformatics tools,
JSON for SDKs, JSONL for record streams, and XLSX for spreadsheet users.

## Caveats

JSONL accepts only an object or array of objects. Nested arrays and objects
become JSON text in table cells. XLSX is limited to
1,048,576 rows and 16,384 columns including the header. Domain files such as
VCF and BED should remain in their native format when semantics matter.

## Runtime Dependencies

The exporter is built into the Rust application and uses no external runtime
or network connection.

## Citations

CSV follows RFC 4180-compatible quoting. XLSX output uses the ECMA-376 Office
Open XML workbook format.

## Troubleshooting

An unsupported extension is rejected. Mixed scalar arrays are neither tables
nor JSONL records; convert them to objects or a two-dimensional array before
export.
