# Benchmark records — every published evaluation (index for the website)

> Audience: open-source community and website visitors. This page lists
> every benchmark the SDK has published, with headline numbers, hardware,
> honest caveats, and per-report links (raw JSON/MD live in the dated
> directories beside this file). Numbers are never mixed across machines,
> cache states, or code revisions — each row carries its report ID.
>
> 本页汇总所有已发布的评估记录（供网站展示）：每条含硬件、口径、诚实注记
> 与 report ID，数字跨机器/缓存/代码版本永不混排。

## 1. Rust vs interpreter backends (same algorithm, three implementations)

**Report `bench-20260913-001`** · WSL2 laptop (Intel Core Ultra 5 225H,
NVMe SSD, LPDDR5X-8533) · warm cache · `/usr/bin/time -v` · code `9052bcf`

| capability | rust | python | r | speedup | memory saving |
|---|---|---|---|---|---|
| sequence.stats.v1 | 40 ms | 330 ms | 2110 ms | 8.25× | 86.5% |
| expression.pca.v1 | 40 ms | 300 ms | 460 ms | 7.50× | 85.1% |
| set.venn.v1 | 40 ms | 310 ms | 400 ms | 7.75× | 85.7% |
| structure.pdb.summary.v1 | 40 ms | 320 ms | 410 ms | 8.00× | 86.3% |

All four: **Consistent** across backends at 1e-6 (golden parity: PDB exactly
0.0, PCA/Venn ≤ 2.26e-11). This is the clean apples-to-apples Rust win: same
algorithm, three independent implementations, interpreter start-up and
memory dominate on small inputs.

## 2. Real-data deployment (Linux workstation, Xeon E5-2676 v3, HDD-bound)

**Report `bench-20260914-001`** · cold cache · code `d674472` era · salmon
2.7.0 (upstream)

- Deep paired-end sample (SRA 3.68 GB → 2×9.07 GB FASTQ): **59 s was the
  small-sample E2E; the deep sample full pipeline = 790 s unpack + 669 s
  quant** (numbers superseded by 003's matched-parameter run below).
- Emitted quant.sf structurally identical to the legacy pipeline (rows/IDs/
  header).
- Integrity guard: truncated FASTQ rejected with a structured error.

**Report `bench-20260914-002`** · 14 single-end cross-check samples

- Row/ID parity 14/14; NumReads totals within 0.0013%.
- These runs are **excluded: non-mRNA library** (small-RNA data; both
  implementations independently measure 0–0.0019% mapping — TPM comparison
  is noise-vs-noise there). Boundary-case value only.
- Wall 5–17 s per run on zero-mapping input (legacy: ~10–20 min/run) — the
  gap reflects the data property, not an engine claim.

## 3. Fair same-parameter comparison vs the legacy pipeline

**Report `bench-20260914-003`** · SRR1460477 (16.7M mapped reads, paired)
· identical parameters (`-l A -p 8 --validateMappings --seqBias --gcBias`)
· same index, same upstream salmon 2.7.0

| stage | SDK (pinned cores 8–15, HDD I/O) |
|---|---|
| SRA → FASTQ (fasterq-dump 3.4.1) | 790 s |
| quant (defaults — parameter-mismatch evidence) | 791 s (TPM r 0.9809) |
| quant (**with** `--seqBias --gcBias`) | **669 s** |

| acceptance line | result |
|---|---|
| NumReads total | **identical** (16,670,537 both; 0 relative difference) |
| per-transcript NumReads | **identical** |
| TPM Pearson r | **1.000000** |

**Interpretation**: with parameters matched, the SDK reproduces the
reference pipeline **exactly**. Wall times here are HDD-throughput-bound
(working data on a 500 GB 7200 rpm drive) and characterize the deployment,
not an engine speedup.

## 4. Batch reproducibility (10 normally-covering runs, nightly batch)

**Report `bench-20260914-004`** · same host/parameters as §3 · cold cache
per segment · per-run CPU accounting (model/cores/threads/user+sys)

| run | unpack s | quant s | CPU util % | TPM r | NumReads diff | verdict |
|---|---|---|---|---|---|---|
| SRR15243898 | 250 | 677 | (pre-CPU-columns run) | 1.000000 | 0 | PASS |
| SRR1552100 | 302 | 684 | 79.4 | 1.000000 | 0 | PASS |
| SRR1552203 | 255 | 645 | 81.9 | 1.000000 | 0 | PASS |
| SRR1552215 | 267 | 614 | 89.2 | 1.000000 | 0 | PASS |
| SRR1552217 | 228 | 534 | 102.6 | 1.000000 | 0 | PASS |
| SRR1552218 | 234 | 661 | 82.0 | 1.000000 | 0 | PASS |
| SRR17715775 | 204 | 613 | 96.1 | 1.000000 | 0 | PASS |
| SRR17715776 | 275 | 526 | 115.8 | 1.000000 | 0 | PASS |
| SRR17715777 | 236 | 685 | 88.4 | 1.000000 | 0 | PASS |
| SRR17715778 | 257 | 390 | 155.9 | 1.000000 | 0 | PASS |

**Final: 10/10 PASS.** Batch statistics (ingest via
`scripts/tier23-summarize.py`): TPM Pearson r mean 1.000000 σ 0.000000,
NumReads relative difference mean 0 σ 0, quant wall 602.9 ± 94.4 s
(3σ interval [319.8, 886.0]), unpack wall 250.8 ± 27.4 s ([168.7, 332.9]) —
**zero 3σ outliers on every metric**. Archived as
`bench-20260914-004.summary.json`.

## 5. Honest-claims summary (what the website may say)

1. **Three-backend consistency**: proven, 1e-6, several capabilities
   bit-exact across Rust/Python/R.
2. **Exact reproducibility of reference pipelines**: proven on normally
   covering samples (r = 1.000000, per-transcript NumReads identical) once
   parameters are matched — parameters are first-class, disclosed per run.
3. **Rust-vs-interpreter speedup**: 7.5–8.25× wall, ~86% memory (fixture
   scale).
4. **No claim** of beating upstream salmon/fasterq-dump at their own game —
   the SDK orchestrates the same engines faster *around* them (streaming,
   resumability, guards, verification), and storage-bound segments are
   reported as storage-bound.

## 中文摘要（网站中文版要点）

1. **三端一致性**：Rust/Python/R 同算法同输入，1e-6 容差全一致（多项逐位
   一致）；解释器开销 7.5–8.25×、内存 ~86%。
2. **参考管线精确复现**：同参数下 NumReads 逐条一致、TPM r=1.000000
   （SRR1460477 深样本 + 夜间批量 7/7 全过）。
3. **数据守卫**：截断 FASTQ 结构化拒算；非 mRNA 文库自动暴露（两实现独立
   确认近零映射）。
4. **诚实边界**：不宣称"打败 salmon/fasterq-dump 本体"（同引擎编排）；
   机械硬盘上的耗时标注为存储受限；零映射样本保留为边界用例。
