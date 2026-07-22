---
name: analyze-fastq-quality
description: Compute and validate deterministic local FASTQ read-quality metrics with the Linxira Bio `fastq.qc.v1` capability. Use for plain, gzip, or BGZF FASTQ read counts, length and base composition, mean quality, Q20/Q30 percentages, quality-encoding checks, and bounded per-cycle QC.
---

# Analyze FASTQ Quality

Use the tested streaming Rust capability instead of writing a one-off FASTQ
parser.

## Run

1. Confirm with `inspect-bio-dataset` that the local input is a supported FASTQ
   and has no structural inspection errors.
2. Use `quality_encoding=auto` unless sequencer metadata establishes Phred+33
   or legacy Phred+64. Never select an encoding only to improve QC.
3. Run:

```bash
linxira-bio fastq qc INPUT.fastq --json
```

When developing in the source repository, run:

```bash
cargo run -p linxira-bio-cli -- fastq qc INPUT.fastq --json
```

Use `--max-cycles N` to change the default 500-cycle preview cap. Use
`--quality-encoding phred+33` or `phred+64` only with supporting metadata.

4. Preserve the capability ID, CLI version, input hash, parameters, warnings,
   and complete JSON result.
5. Reject malformed or truncated records. Do not interpret partial output as a
   completed analysis.

## Interpret

- `read_count`, length metrics, GC, and N percentages describe all accepted
  records.
- `mean_quality`, `q20_percent`, and `q30_percent` use
  `applied_quality_offset`.
- `quality_encoding: ambiguous` means the observed characters fit more than
  one historical encoding. The capability reports a warning and uses
  Phred+33 until explicitly overridden.
- `per_cycle` is bounded by `max_cycles`; a cap warning means later cycles are
  absent from that array, not from aggregate metrics.

These metrics do not detect adapter contamination, duplication, overrepresented
sequences, taxonomic contamination, or instrument-specific artifacts. Do not
turn a Q30 percentage alone into a biological pass/fail conclusion.
