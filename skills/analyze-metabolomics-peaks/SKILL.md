---
name: analyze-metabolomics-peaks
description: Parse mzML mass-spectrometry files locally and detect centroid peaks (m/z, intensity, retention time) for research-only metabolomics profiling.
---

# Analyze Metabolomics Peaks

Inspect imported files before execution. Use the Rust capability; do not
reimplement mzML parsing or peak picking in Python or R.

## Choose a capability

- Use `medical.metabolomics.v1` with an mzML file (optionally gzip) to decode
  m/z and intensity arrays and produce a local-maximum centroid peak table
  plus per-spectrum MS-level summaries.

## Execute

```bash
linxira-bio medical metabolomics SAMPLE.mzML PEAKS.tsv --json
```

## Interpret

Report the peak table (spectrum index, retention time, m/z, intensity) with
`ms1_count`/`ms2_count`. Peaks are centroid candidates only: no isotopic
deconvolution, feature alignment, or quantification is performed. Keep
clinical metabolomics data local.

## Caveats

Local-maximum detection without a noise threshold; m/z and intensity arrays
only. Results depend on the acquisition profile (centroid vs profile).
