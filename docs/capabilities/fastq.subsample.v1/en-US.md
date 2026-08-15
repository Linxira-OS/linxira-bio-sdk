# FASTQ Subsample

## Purpose

Randomly subsample reads from a FASTQ file by target count or fraction using
reservoir sampling for memory-efficient processing.

## Inputs

A plain or gzip FASTQ file with one or more reads.

## Parameters

`--target-count` sets the exact number of reads to retain. `--fraction`
sets the fraction of reads to retain (0.0 to 1.0). `--seed` sets the
random seed for reproducibility (default 42).

## Outputs

A subsampled FASTQ file. JSON result includes input read count, output
read count, and the sampling method used.

## Examples

```bash
linxira-bio fastq subsample input.fastq output.fastq --target-count 10000 --seed 42 --json
```

## Interpretation

Verify that the output read count matches the requested target or fraction.
Reservoir sampling ensures each read has an equal probability of selection.

## Caveats

Target count exceeding input count produces a warning and outputs all reads.
Subsampling by fraction is approximate due to integer rounding.

## Runtime Dependencies

Local Rust only; no Python, R, Java, or external executable is required.

## Citations

Vitter JS. Random sampling with a reservoir. ACM Trans Math Softw. 1985;11(1):37-57.

## Troubleshooting

Ensure the input is valid FASTQ format with complete read records. Existing
output files are never overwritten.