# Long-Read Alignment

## Purpose

Align long sequencing reads (ONT or PacBio) to a reference genome using
minimap2 with configurable presets.

## Inputs

A reference genome in FASTA format and long reads in FASTQ format.

## Parameters

`--preset` selects the alignment preset: `map-ont`, `map-pb`, or `map-hifi`
(default `map-ont`). `--threads` sets the number of threads (default 1).

## Outputs

A SAM file with aligned reads. JSON result wraps the native tool execution
metadata including aligned and unaligned read counts.

## Examples

```bash
linxira-bio alignment long-read reference.fa reads.fastq output.sam --preset map-ont --threads 4 --json
```

## Interpretation

Review the alignment rate and mapping quality distribution. Low mapping rates
may indicate incompatible preset, low-quality reads, or distant reference.

## Caveats

Requires minimap2 to be installed. The preset should match the sequencing
technology. SAM output can be large; consider piping to samtools for BAM
conversion.

## Runtime Dependencies

minimap2 executable. Set `LINXIRA_BIO_MINIMAP2` to override the binary path.

## Citations

Li H. Minimap2: pairwise alignment for nucleotide sequences. Bioinformatics.
2018;34(18):3094-3100.

## Troubleshooting

If minimap2 is not found, install it or set `LINXIRA_BIO_MINIMAP2`. Verify
the preset matches your sequencing platform. Existing output files are never
overwritten.