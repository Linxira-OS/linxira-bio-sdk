# Newcomer Frontier Analysis Fast Track (Proposal)

> Status: proposal document, not yet wired into any skill or capability. Target
> location if adopted: `docs/fast-track/` (a standing newcomer-facing document
> separate from the capability pages). Audience: graduate students and junior
> engineers in academia and industry who want to do frontier analysis from
> first principles instead of treating tools as black boxes.

## 0. The one-line pitch

Every capability in this SDK is deterministic and reproducible — fixed seeds,
versioned result schemas, re-runnable fixtures. That engineering promise is
not just QA hygiene; it is the fastest way to *learn* computational biology:
when your result changes, you changed something, and finding out what is the
actual skill.

## 1. Layer 1 — Application: pick capabilities, run pipelines

Entry points:

- `AGENTS.md` — the skill routing table: each line maps a question ("I have
  FASTQ reads from an ONT run") to a skill folder.
- `skills/<name>/SKILL.md` — concise procedures with real commands and
  interpretation guidance.
- `docs/capabilities/<capability-id>/en-US.md` (and `zh-CN.md`) — the ten
  fixed sections: Purpose, Inputs, Parameters, Outputs, Examples,
  Interpretation, Caveats, Runtime Dependencies, Citations, Troubleshooting.

The rule of thumb: **ask AGENTS.md which skill, read the SKILL.md for the
procedure, read the capability page for the semantics.**

### Example chain A — from synthetic reference to variant calls (fully local, no downloads)

```bash
# 1. Make a reproducible toy reference and reads (fixed seed => byte-identical)
linxira-bio simulate sequence ref.fa --count 2 --length 20000 --gc 45 --seed 7
linxira-bio simulate reads ref.fa reads.fq --read-length 150 --coverage 15 --seed 7

# 2. QC the reads
linxira-bio fastq qc reads.fq --json

# 3. Align (minimap2 -x sr + samtools sort inside the capability)
linxira-bio alignment short-read ref.fa reads.fq aligned.bam --threads 4

# 4. Flag duplicates (collate/fixmate/sort/markdup inside the capability)
linxira-bio alignment markdup aligned.bam marked.bam --stats

# 5. Call variants (bcftools mpileup+call inside the capability)
linxira-bio variant call marked.bam ref.fa calls.vcf --min-mq 20 --min-bq 13

# 6. Summarize
linxira-bio variant stats calls.vcf --json
linxira-bio alignment coverage marked.bam coverage.tsv
```

Every step prints a versioned JSON envelope (`capability`, `status`,
`warnings`) — read them; the warnings teach the data's failure modes.

### Example chain B — similarity → annotation → enrichment

```bash
# Find homologs (native BLAST+ orchestration)
linxira-bio similarity blast query.fasta reference.fasta hits.tsv --program blastn

# Normalize GO annotations into association tables
linxira-bio annotation go annotations.tsv associations.tsv

# Over-representation and preranked GSEA on custom/GO/KEGG sets
linxira-bio enrichment custom genes.txt associations.tsv --json
linxira-bio enrichment gsea ranked.tsv gene_sets.tsv --json
```

### Example chain C — synthetic benchmark round-trip

```bash
linxira-bio simulate sequence ref.fa --count 1 --length 100000 --seed 42
linxira-bio simulate reads ref.fa reads.fq --read-length 150 --coverage 30 --paired --error-rate 0.5 --seed 42
linxira-bio alignment short-read ref.fa reads.fq aligned.bam
linxira-bio variant call aligned.bam ref.fa calls.vcf
# Compare calls.vcf against the known ground truth you control — the whole
# point of simulation: you know the answer before you run the pipeline.
```

## 2. Layer 2 — Hardware: gpu-lab as the teaching vehicle

`gpu-lab/` is a standalone Cargo workspace (excluded from the main workspace
and CI) that exists precisely so newcomers can break things safely:

1. **`probe`** — enumerate adapters, drivers, and limits (M5-G0). Run it on
   your own machine; the output is a hardware ledger entry.
2. **CPU baselines** (`bench`) — deterministic CPU kernels with SplitMix64
   seeded inputs and runtime SIMD tier detection, AVX-512/AVX2/scalar
   (M5-G1). This is where you learn that *a baseline is a contract, not a
   formality*: the benchmark gate refuses kernels without one.
3. **wgpu/WGSL kernels** (`gpu-bench`) — port the baseline to WGSL, phase the
   timing (upload/compute/readback), medians of ≥3 runs (M5-G2). Read
   `docs/engine-evals/gpu-wgpu-2026-09-30.md` first: it is the format
   template, including how precision tolerances are declared per metric.
4. **Vendor-stack comparisons** (M5-G3, see
   `docs/proposals/m5-g3-vendor-track-plan.md`) — same kernels through
   cutile-rs / cudarc-musa / sycl-rs with the fallback-to-wgpu trait layer.

Growth path = the milestone ladder itself: M5-G0 → G3. Your lab notebook is
`docs/engine-evals/` — measurement discipline (median of ≥3, phased timing,
explicit tolerance) is the skill being taught.

## 3. Layer 3 — Upstream: early Rust-GPU communities

The vendor Rust stacks are days-to-months old; first reports from real
workloads get maintainer attention and build visible track record:

- **cudarc-musa** (Moore Threads, official fork) — first-developer window
  issues (see proposal §2.2).
- **cutile-rs** (NVIDIA official) — consumer-GPU sm_89 workload reports.
- **sycl-rs** (Intel oneapi-rs) — Windows untested upstream; your Windows
  experiment is itself the contribution.

One well-documented issue with a minimal reproducer beats ten starred repos —
and it is the cheapest visible artifact a newcomer can produce.

## 4. First principles, translated into practices

| SDK practice | What you learn by following it |
| --- | --- |
| Fixed seeds (`--seed`) | Reproducibility is a feature; bugs become bisectable |
| Versioned result schemas (`*.v1`) | Interfaces are contracts; version bumps are semantic acts |
| Re-runnable fixtures (`tests/fixtures/`) | Golden files teach exact expected behavior |
| Benchmark gate (no CPU baseline, no kernel) | Performance claims start from a measured reference, not intuition |
| Dual implementation rule (CPU+GPU) | An accelerator without a trustworthy fallback is a liability |
| Median-of-3, phased timing ledgers | Measurement noise is part of the data, not an inconvenience |

## 5. Suggested first week

1. Run `linxira-bio doctor` and the environment audit; install one missing
   native tool via the suggested profile.
2. Work through example chain A end to end; read every JSON envelope.
3. Run `gpu-lab probe` and `bench` on your machine; write a two-paragraph
   ledger entry in the engine-evals format.
4. Pick one SKILL.md and improve one sentence — first upstream contribution.

---
*Chinese version: see `zh-CN.md` in this folder.*
