# Metabolomics Peak Detection

## Purpose

Parse an mzML mass-spectrometry file locally, decode m/z and intensity arrays (base64, 32/64-bit floats, optional zlib), and detect centroid peaks by local-maximum picking for research-only metabolomics profiling.

## Inputs

An mzML file (optionally gzip-compressed) containing one or more spectra with binary m/z and intensity arrays.

## Parameters

No parameters are required.

## Outputs

A TSV peak table with columns `spectrum_index`, `retention_time_min`, `mz`, `intensity`. JSON output reports `spectrum_count`, `ms1_count`, `ms2_count`, `peak_count`, and the full `peak_table`.

## Examples

```bash
linxira-bio medical metabolomics sample.mzML peaks.tsv --json
```

## Interpretation

Each peak is a local maximum in the intensity array (positive intensity, greater than both neighbors) with its m/z and retention time. Peaks are the centroid candidates for feature grouping; the MS level is taken from the spectrum CV terms (MS:1000511).

## Caveats

Research-use-only. Peak picking is a simple local-maximum threshold-free detector; no isotopic deconvolution, feature alignment, or quantification is performed. Only m/z (MS:1000514) and intensity (MS:1000515) arrays are decoded; other array types are ignored.

## Runtime Dependencies

None — pure local Rust capability (gzip and zlib support built in).

## Citations

mzML 1.1.0 format specification (HUPO-PSI Mass Spectrometry standards).

## Troubleshooting

If parsing fails, confirm the file is valid mzML XML with `<binary>` arrays, that the base64 data is intact, and that arrays are declared 32-bit (MS:1000523) or 64-bit (MS:1000521) floats. gzip-compressed inputs are auto-detected.
