# FASTA Split

## Purpose

Split one FASTA file into deterministic numbered chunk files for batching, upload limits, or downstream tools.

## Inputs

One readable FASTA file. Plain text and gzip streams are supported.

## Parameters

The command requires an input FASTA and output directory. Optional parameters are `--records-per-file` and `--prefix`. `--json` returns the standard result envelope.

## Outputs

Writes numbered FASTA chunks such as `part_001.fa` into the output directory. JSON reports input records, output files, residues, records per file, and prefix.

## Examples

```bash
linxira-bio sequence split input.fa chunks --records-per-file 1000 --prefix part --json
```

## Interpretation

Chunk numbering is deterministic and follows input record order.

## Caveats

Existing chunk filenames are never overwritten. Choose an empty output directory or a fresh prefix for reruns.

## Runtime Dependencies

This is a pure local Rust capability with no Python, R, Java, or external bioinformatics tools.

## Citations

Chunked FASTA output follows conventional record-order preserving batching behavior.

## Troubleshooting

If the command refuses to write a chunk, remove stale output files or choose a new output directory.
