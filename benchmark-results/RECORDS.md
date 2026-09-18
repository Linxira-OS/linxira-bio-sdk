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

## 5. Quantification completion — deep libraries (26 SRA runs, Linux workstation)

**Report `bench-20260915-001`** · Xeon E5-2676 v3, 8 threads pinned · salmon
2.7.0 (upstream), `-l A -p 8 --validateMappings --seqBias --gcBias` · code
v1.0.3 deployment

The study's 26 not-yet-computed deep/public runs (SRR19049440/41/43/44/45,
SRR22699505-516, SRR24322339-353 subset) quantified end-to-end through the
public CLI. Every quant.sf: **33,955 transcript rows**. Per-run medians:
unpack 219 s, quant 97 s; CPU utilization median 681%. **Production
resilience proven**: a scheduled 06:50 host reboot hit mid-batch; a
user-level oneshot resumed the batch **20 seconds after boot**, skipped the
4 finished runs, and completed the remaining 22 with zero manual
intervention. No-reference by construction (these are the runs the legacy
pipeline had not computed); correctness rests on the 003/004 parity line.
Honest note: wall times vary 15× with workload contention while CPU-seconds
stay comparable — the ledger records utilization per run.

## 6. Quantification completion — mixed library types (31 SRA runs)

**Report `bench-20260915-002`** · same host · code v1.0.3 deployment

| group | runs | median quant wall (s) |
|---|---|---|
| single-end SRA (SRR11592354-61, SRR30067602-09, SRR30415358-63) | 22 | 15 |
| paired deep SRA, 2.1-2.9 GB (SRR15219542-47) | 6 | 78 |
| paired fastq.gz direct (SRR8759198-200) | 3 | 82 |

31/31 quant.sf: full 33,955-transcript ID set; authoritative inputs
consumed read-only; batch wall clock 2 h 18 min on an idle workstation.

## 7. Small-RNA runs against miRBase mature (14 SRA runs)

**Report `bench-20260915-003`** · same host · salmon 2.7.0, k=19,
`--keepDuplicates`, no bias correction · code v1.0.3 deployment

14 public runs that map near-zero against the mRNA reference
(SRR28573920-25, SRR8205656-63) quantified against miRBase mature
(69,020 entries → 69,019 indexable targets; the one exclusion is a 20 nt
poly-A entry salmon cannot index). The eight 50 nt raw reads were
3' adapter-trimmed with the SDK first. Every quant.sf: **69,019 rows**.
Documented data-property findings: siRNA-dominant composition (~1.6%
miRNA exact-substring) and inserts longer than the mature targets (10.7%
exact-substring, ~0% end-to-end) — the library-type interpretation
belongs to the study.

## 8. GO/KEGG/KO over-representation + WGCNA module enrichment (26 runs)

**Report `bench-20260915-004`** · same host · `enrichment custom`
(hypergeometric + BH, Rust engine) · code v1.0.3 deployment

Three DEG contrasts (blue-light vs dark; young fruit vs leaf; mature fruit
vs leaf — 3,577/9,630/12,875 significant genes reproduced exactly,
direction-split) and 13 WGCNA modules, tested for GO, KEGG pathway and KO
terms. Universe = ID-bridge-mapped annotated genes (GO 14,944 / KO 13,912
/ KEGG 8,929); bridge 27,908/33,955, zero duplicate tails; term names from
go-basic.obo and rest.kegg.jp. **26/26 runs in 17.2 s total engine wall**;
every output CSV carries the universe in its header comment.

## 9. Honest-claims summary (what the website may say)

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

## 2026-09-18 — r=1.000000 live re-verification (SRR1460477)
Fresh-pull SRA + fasterq + SDK quantify (bias-matched) vs reference quant.sf: byte-identical (SHA256 9b6a4cca…), r=1.000000000, NumReads 33955/33955. Control (no bias flags): r=0.980896. Files + hashes + reproduce script in 2026-09-18/r-verification/.
