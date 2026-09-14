# expression.quantify.v1

Run `salmon quant` over single- or paired-end reads against a pre-built
index and summarize the resulting transcript table. The engine orchestrates
the native tool with controlled, shell-free arguments; the full `quant.sf`
table is preserved as the artifact and reduced to a machine-readable
summary for cross-run comparison (e.g. against an existing traditional
pipeline's quant.sf, row counts and totals).

## Purpose

Quantify transcript abundance from reads with salmon and keep both the raw
quant table and a comparable summary, so Rust-orchestrated runs and legacy
pipeline outputs can be checked field by field.

## Inputs

- One single-end read file (`fasta`/`fastq`, optionally gzipped) or exactly
  two paired-end mate files.
- A pre-built salmon index directory (`--index`); the index is not built by
  this capability.

## Parameters

- `--index DIR` (required): salmon index directory.
- `--lib-type A` (default `A`, automatic detection): salmon library type.
- `--threads N` (default 1): salmon parallelism.
- `--no-validate-mappings`: disable `--validateMappings` (selective
| `--seq-bias` / `--gc-bias` | off | salmon `--seqBias`/`--gcBias` bias modeling; the reference paired-end pipelines enable both |
  alignment mode is the default).
- `--json`: emit the standard result envelope.
- The salmon binary resolves via `LINXIRA_BIO_SALMON` or `PATH`.

## Outputs

- `--output quant.sf` (default `quant.sf`): the verbatim quant.sf table
  (Name, Length, EffectiveLength, TPM, NumReads).
- Summary fields: tool, lib_type, thread_count, transcript_count,
  expressed_transcript_count, total_tpm, total_num_reads, top_transcripts
  (top 5 by TPM with NumReads fractions), output_bytes, warnings.

## Examples

```bash
linxira-bio expression quantify r1.fq.gz r2.fq.gz --index salmon_idx --threads 8 --output run1/quant.sf --json
```

## Interpretation

- `transcript_count` must equal the number of transcripts in the index;
  comparing it against a legacy pipeline's quant.sf row count is the first
  cross-check.
- `total_num_reads` is the estimated assigned reads; differences against a
  traditional run reflect salmon parameters (lib type, validateMappings),
  which are disclosed in the summary.

## Caveats

- Requires salmon installed locally; the engine never builds indices or
  downloads references.
- gzipped FASTQ is passed through to salmon directly.

## Runtime Dependencies

- salmon (any recent 1.x); resolve with `LINXIRA_BIO_SALMON` or PATH.
- No Python/R runtime is involved.

## Citations

- Patro, R. et al. (2017). Salmon provides fast and bias-aware quantification
  of transcript expression. Nature Methods, 14, 417–419.

## Troubleshooting

- `salmon` not found: install salmon or set `LINXIRA_BIO_SALMON`.
- quant.sf row-count mismatch vs a legacy run: compare index builds and
  salmon versions before blaming the orchestrator.
