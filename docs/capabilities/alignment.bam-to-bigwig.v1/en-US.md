# BAM/CRAM to BigWig

## Purpose

Create a local BigWig coverage track from BAM or CRAM alignment data.

## Inputs

An indexed BAM or CRAM alignment.

## Parameters

`--threads` controls the native tool worker count.

## Outputs

A BigWig coverage track and optional JSON result envelope.

## Examples

```text
linxira-bio alignment bam-to-bigwig reads.bam coverage.bw --json
```

## Interpretation

The track represents coverage according to the installed native tool defaults.

## Caveats

Normalization and binning settings are not configurable in this initial wrapper.

## Runtime Dependencies

Local deepTools `bamCoverage`, configurable with `LINXIRA_BIO_BAMCOVERAGE`.

## Citations

Cite deepTools and the alignment method used to produce the input.

## Troubleshooting

Ensure the alignment is indexed and that `bamCoverage` is available.
