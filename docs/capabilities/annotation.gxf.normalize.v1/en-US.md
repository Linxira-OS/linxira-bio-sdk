# annotation.gxf.normalize.v1

## Purpose

Validate GFF3 or GTF and write canonical GFF3.

## Inputs

One plain or gzip-compressed GFF3/GTF annotation.

## Parameters

- `--sort`: order records by sequence, start, end, feature type, and source.
- `--json`: emit a structured summary.

## Outputs

A new GFF3 file plus counts for input/output records, converted GTF attribute records, sorting, and warnings.

## Examples

```bash
linxira-bio annotation normalize input.gtf output.gff3 --sort --json
```

## Interpretation

Use the conversion count to confirm whether GTF-style attributes were detected. Reserved GFF3 separators are percent-encoded.

## Caveats

The command validates and normalizes syntax but does not infer or repair missing biological relationships. Existing outputs are not overwritten.

## Runtime Dependencies

Runs locally in the Rust core without external runtimes or network access.

## Citations

No external algorithm is used; output follows the repository's deterministic GFF3 serialization contract.

## Troubleshooting

If conversion fails, inspect the reported line for incorrect column count, coordinates, strand, phase, or attributes.
