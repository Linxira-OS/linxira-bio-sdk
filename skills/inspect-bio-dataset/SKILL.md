---
name: inspect-bio-dataset
description: Identify, validate, and safely preview local biological data with the Linxira Bio `dataset.inspect.v1` capability. Use before analyzing FASTA, FASTQ, CSV, TSV, BED, GFF3, GTF, VCF, SAM, BAM, compressed inputs, unknown files, or recognized formats whose executable support must be confirmed.
---

# Inspect Bio Dataset

Inspect the file before selecting an analysis capability. Do not generate an
ad hoc parser when this capability supports the format.

## Run

1. Keep the file local and record the path supplied by the user.
2. Run:

```bash
linxira-bio dataset inspect INPUT --json
```

When developing in the source repository, run:

```bash
cargo run -p linxira-bio-cli -- dataset inspect INPUT --json
```

3. Read `result.format`, `compression`, `support`, `confidence`, `warnings`,
   `errors`, and the capped preview.
4. Stop if `errors` is non-empty. Report the issue code and line when present.
5. Continue only when `support` is `supported`; then route to an available
   capability that accepts the detected format.

## Interpret

- Content detection overrides a misleading extension and emits
  `format-extension-mismatch`.
- `recognized-unsupported` means the format was identified but this release
  cannot execute an importer. Do not treat it as available.
- Preview data is sampled, capped at 200 records or 10 MiB, and is not proof
  that the remainder of the file is valid.
- Gzip and BGZF are decompressed only for bounded inspection. ZIP archives are
  never extracted; ask the user to extract them locally.
- HDF5 containers need their extension and domain metadata to distinguish
  H5AD, LOOM, and generic HDF5 reliably.

Preserve the capability ID, inspection JSON, warnings, and input identity with
the downstream analysis record.
