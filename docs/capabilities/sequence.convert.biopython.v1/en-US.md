# Sequence Convert (Biopython)

## Purpose

Convert biological sequence files between FASTA, FASTQ, GenBank, and EMBL
using a locked Biopython workflow pack. The conversion is strict: records are
parsed with the declared input format and written with the declared output
format, with no silent reinterpretation.

## Inputs

One sequence file in FASTA, FASTQ, GenBank, or EMBL format (uncompressed).

## Parameters

- `--input-format fasta|fastq|genbank|embl` — overrides extension-based input
  format detection.
- `--output-format fasta|fastq|genbank|embl` — overrides extension-based output
  format detection.

## Outputs

A converted file at the requested output path, plus a JSON result envelope with
`records_written`, `input_format`, `output_format`, the converted artifact
(path, size, SHA-256), and provenance (CPython, Biopython, NumPy versions and
the dependency lock hash).

## Examples

```bash
linxira-bio sequence convert input.fasta output.genbank --output-format genbank
linxira-bio sequence convert reads.fastq reads.fa --output-format fasta
```

## Interpretation

Verify the output record count matches the input. Converting FASTA to FASTQ is
not supported because FASTA records carry no quality scores; the pack rejects
the request with an error envelope.

## Caveats

The capability runs the `org.linxira.sequence-conversion-biopython` workflow
pack and requires a CPython 3.12 interpreter plus the pinned Biopython and
NumPy versions from the pack's hashed lock. Existing output files are never
overwritten. Compressed inputs are not supported by this capability.

## Runtime Dependencies

Python 3.12.x with `biopython==1.85` and `numpy==2.2.4`, resolved through the
pack lock (`workflows/org.linxira.sequence-conversion-biopython/requirements.lock`).

## Citations

Cock PJ et al. Biopython: freely available Python tools for computational
molecular biology and bioinformatics. Bioinformatics. 2009;25(11):1422-1423.
<https://doi.org/10.1093/bioinformatics/btp163>

## Troubleshooting

- "cannot infer a sequence format from extension" — pass `--input-format` or
  `--output-format` explicitly.
- A non-zero exit with a `status: error` envelope means the pack rejected the
  request; check the diagnostic message.
- The output path must not already exist.
