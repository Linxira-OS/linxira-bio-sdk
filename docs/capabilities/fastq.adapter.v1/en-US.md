# fastq.adapter.v1

## Purpose

Remove exact 3' sequencing adapter matches from FASTQ reads locally and write a
new FASTQ artifact.

## Inputs

- One FASTQ file: plain text, gzip, or BGZF.

## Parameters

- `adapter`: one adapter sequence.
- `adapters`: array of adapter sequences. Use either `adapter` or
  `adapters`, not both.
- `min_overlap`: minimum suffix overlap for partial adapter clipping. Default:
  8.
- `min_length`: reads shorter than this after clipping are discarded. Default:
  20.
- `output`: required FASTQ output path. Existing files are not overwritten.

## Outputs

- A normalized four-line FASTQ file.
- JSON summary with input/output read counts, discarded reads, trimmed reads,
  input/output bases, adapter-trimmed bases, and warnings.

## Examples

```bash
linxira-bio fastq adapter-trim reads.fastq.gz reads.no-adapter.fastq \
  --adapter AGATCGGAAGAGC --min-overlap 8 --min-length 20 --json
```

## Interpretation

`adapter_trimmed_bases` and `trimmed_read_count` indicate how much sequence was
removed. Run `fastq.qc.v1` after clipping when downstream quality evidence is
needed.

## Caveats

This version performs exact 3' adapter or partial-adapter clipping. It does not
perform error-tolerant adapter matching, automatic adapter discovery, paired-end
synchronization, UMI parsing, quality trimming, or duplicate removal.

## Runtime Dependencies

Pure local Rust capability. No Python, R, Java, cutadapt, fastp, or external
FASTQ tool is required.

## Citations

Adapter clipping is a standard read preprocessing method. Preserve the command,
parameters, adapter sequence, input hash, output path, and result JSON.

## Troubleshooting

- If no bases are trimmed, verify the adapter orientation and `min_overlap`.
- If too many reads are discarded, lower `min_length`.
- If output creation fails, choose a path that does not already exist.
