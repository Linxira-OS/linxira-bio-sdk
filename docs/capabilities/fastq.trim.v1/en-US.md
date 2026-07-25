# fastq.trim.v1

## Purpose

Trim trailing low-quality FASTQ bases locally and write a new FASTQ artifact for
downstream alignment, assembly, or QC.

## Inputs

- One FASTQ file: plain text, gzip, or BGZF.

## Parameters

- `min_quality`: trailing bases below this Phred score are removed. Default: 20.
- `min_length`: reads shorter than this after trimming are discarded. Default: 20.
- `quality_encoding`: `phred+33` or `phred+64`. Default: `phred+33`.
- `output`: required FASTQ output path. Existing files are not overwritten.

## Outputs

- A normalized four-line FASTQ file.
- JSON summary with input/output read counts, discarded reads, trimmed reads,
  input/output bases, quality-trimmed bases, and warnings.

## Examples

```bash
linxira-bio fastq trim reads.fastq.gz reads.trimmed.fastq \
  --min-quality 20 --min-length 20 --quality-encoding phred+33 --json
```

## Interpretation

Compare `input_read_count`, `output_read_count`, `discarded_read_count`,
`quality_trimmed_bases`, and `output_bases` before using the trimmed FASTQ.
Run `fastq.qc.v1` before and after trimming when quality evidence is needed.

## Caveats

This version performs 3' threshold trimming only. It does not implement
sliding-window trimming, poly-G/poly-X trimming, UMI processing, paired-end
synchronization, or duplicate removal.

## Runtime Dependencies

Pure local Rust capability. No Python, R, Java, or external FASTQ tool is
required.

## Citations

FASTQ quality trimming is a standard preprocessing method. Preserve the command,
parameters, input hash, output path, and result JSON for reproducibility.

## Troubleshooting

- If all reads are discarded, lower `min_length` or inspect the input quality.
- If quality values look wrong, verify `quality_encoding`.
- If output creation fails, choose a path that does not already exist.
