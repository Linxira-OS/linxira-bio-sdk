# FASTQ Read Quality Control

## Purpose

Compute deterministic aggregate and per-cycle quality-control metrics from a
FASTQ stream without loading the full dataset into memory.

## Inputs

One readable FASTQ file. Plain text, gzip, and BGZF streams are detected by
content. Wrapped sequence and quality lines are supported; every record must
have matching sequence and quality lengths.

## Parameters

The input path is required. `--quality-encoding` accepts `auto` (default),
`phred+33`, or `phred+64`. `--max-cycles` limits per-cycle output to 500 cycles
by default without limiting aggregate metrics. `--json` returns the standard
analysis result envelope.

## Outputs

Returns read and base counts, minimum, maximum, and mean read length, GC and N
percentages, mean quality, Q20/Q30 percentages, detected and applied quality
encoding, per-cycle metrics, and warnings.

## Examples

```bash
linxira-bio fastq qc tests/fixtures/fastq-qc/valid.fastq --quality-encoding phred+33 --json
```

## Interpretation

Quality metrics use `applied_quality_offset`. In automatic mode, characters
that overlap historical Phred+33 and Phred+64/Solexa ranges produce
`quality_encoding: ambiguous`, a warning, and conservative Phred+33 metrics.
Use an explicit override only when instrument or pipeline metadata establishes
the encoding. A per-cycle cap warning does not affect aggregate metrics.

## Caveats

This release does not detect adapters, duplication, overrepresented sequences,
contamination, or instrument-specific artifacts. Q20/Q30 summaries alone do
not establish whether reads are biologically fit for a downstream analysis.

## Runtime Dependencies

This is a streaming local Rust capability with no Python, R, Java, or external
bioinformatics dependency.

## Citations

FASTQ quality-encoding history follows Cock et al., 2010, Nucleic Acids
Research 38(6):1767-1771, doi:10.1093/nar/gkp1137.

## Troubleshooting

For a malformed or truncated record, use the reported record and line number
to inspect the source. If automatic encoding is ambiguous, consult sequencer
or upstream pipeline metadata instead of guessing from high-quality values.
