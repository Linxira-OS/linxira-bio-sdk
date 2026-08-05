---
name: process-fastq-reads
description: Trim, deduplicate, subsample, and preprocess local FASTQ reads with Linxira Bio executable capabilities. Use for plain, gzip, or BGZF FASTQ 3' quality trimming, minimum-length filtering, sequencing adapter removal, exact sequence deduplication, strict UMI-aware deduplication, reservoir-based read subsampling, and producing a new FASTQ artifact with `fastq.trim.v1`, `fastq.adapter.v1`, `fastq.deduplicate.v1`, or `fastq.subsample.v1`.
---

# Process FASTQ Reads

Use the tested local Rust capabilities instead of writing ad hoc FASTQ parsers.
These capabilities read plain, gzip, or BGZF FASTQ and write normalized four-line
FASTQ without overwriting an existing output path.

## Choose The Capability

- Use `fastq.trim.v1` for 3' quality-threshold trimming plus minimum-length
  filtering.
- Use `fastq.adapter.v1` for exact 3' adapter or partial-adapter clipping plus
  minimum-length filtering.
- Use `fastq.deduplicate.v1` for case-insensitive exact sequence keys, or exact
  sequence-plus-UMI keys when the UMI is a read-name suffix or sequence prefix.
- Use `fastq.subsample.v1` for reservoir-based read subsampling by target count
  or fraction, with a reproducible seed.
- Use `analyze-fastq-quality` before or after this skill when the user needs
  read quality metrics, Q20/Q30 summaries, or per-cycle QC.

## CLI

```bash
linxira-bio fastq trim INPUT.fastq OUTPUT.fastq \
  --min-quality 20 --min-length 20 --quality-encoding phred+33 --json

linxira-bio fastq adapter-trim INPUT.fastq OUTPUT.fastq \
  --adapter AGATCGGAAGAGC --min-overlap 8 --min-length 20 --json

linxira-bio fastq deduplicate INPUT.fastq OUTPUT.fastq \
  --header-umi-delimiter : --json

linxira-bio fastq subsample INPUT.fastq OUTPUT.fastq \
  --target-count 10000 --seed 42 --json

linxira-bio fastq subsample INPUT.fastq OUTPUT.fastq \
  --fraction 0.1 --seed 42 --json
```

When developing from the repository:

```bash
cargo run -p linxira-bio-cli -- fastq trim INPUT.fastq OUTPUT.fastq --json
cargo run -p linxira-bio-cli -- fastq adapter-trim INPUT.fastq OUTPUT.fastq --json
cargo run -p linxira-bio-cli -- fastq deduplicate INPUT.fastq OUTPUT.fastq --json
cargo run -p linxira-bio-cli -- fastq subsample INPUT.fastq OUTPUT.fastq --target-count 10000 --json
```

## Worker Contract

For v1 jobs, provide one input role named `fastq` and string
`parameters.output`.

- `fastq.trim.v1` parameters: `output`, optional `min_quality`,
  `min_length`, and `quality_encoding` (`phred+33` or `phred+64`).
- `fastq.adapter.v1` parameters: `output`, optional `adapter` or
  `adapters`, `min_overlap`, and `min_length`.
- `fastq.deduplicate.v1` parameters: `output`, and at most one of
  `header_umi_delimiter` or `sequence_prefix_umi`.
- `fastq.subsample.v1` parameters: `output`, and exactly one of
  `target_count` or `fraction`, plus optional `seed`.

For v2 jobs, use input role `fastq`. The output artifact role is `fastq`,
kind `domain-file`, format `fastq`, media type `text/x-fastq`.

## Result Handling

1. Preserve the capability ID, command or worker request, input hashes, output
   path, and JSON result.
2. Report read counts, discarded or duplicate reads, trimmed reads,
   input/output bases, and quality- or adapter-trimmed bases.
3. Treat warnings such as all reads being discarded as scientifically important.
4. Keep the original FASTQ unless the user explicitly asks to delete it.

## Boundaries

- Adapter matching is exact and 3'-oriented in this capability version; it does
  not perform error-tolerant matching, paired-end synchronization, or
  poly-G/poly-X trimming.
- Deduplication keeps the first read for each exact key. UMI-aware modes do not
  perform edit-distance correction, directional clustering, or paired-end
  synchronization.
- Quality trimming only removes trailing low-quality bases; it does not perform
  sliding-window or paired-end overlap correction.
- Use maintained external tools through an approved workflow when those features
  are required.
