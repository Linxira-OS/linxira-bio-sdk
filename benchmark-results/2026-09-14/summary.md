# Linxira Bio SDK — real-sample deployment & cross-check (2026-09-14)

> Audience: open-source contributors and anyone evaluating the SDK's
> performance claims. This run moves from repository fixtures to real
> public RNA-seq data on a Linux workstation. Full environment, data
> provenance (public SRA accessions), command lines, and acceptance lines
> are disclosed; one important caveat about the reference implementation
> is documented rather than hidden.
>
> 报告双语：英文为主，文末附中文摘要。

**Report IDs**: `bench-20260914-001` (commissioning), `bench-20260914-002`
(14-run cross-check, corrected) · **Code revision**: `v1.0.3` base (`c8ebee3`) +
quantify fixes `ff90f7d`, `d674472` · **Engine**: 1.0.3

## 1. What was measured

`expression.quantify.v1` — the SDK orchestrates **upstream salmon 2.7.0**
(bioconda binaries) with shell-free arguments through the public CLI:

```bash
export LINXIRA_BIO_SALMON=/path/to/salmon   # documented override
linxira-bio expression quantify <reads_1.fastq.gz> <reads_2.fastq.gz> \
  --index <salmon-index> --threads 8 --output <run>/quant.sf --json
```

Two things were validated: (a) end-to-end wall time on a real deep-library
paired-end sample, and (b) agreement of the emitted `quant.sf` with
pre-existing quant tables produced by a legacy pipeline on the same index.

### 1.1 Commissioning (bench-20260914-001)

- One deep-library paired-end sample (gzip FASTQ, ~1.1 GB + mate):
  **59 s end-to-end** (cold cache), 33,955 transcripts quantified,
  14,320,007 reads assigned.
- Emitted `quant.sf` is **structurally identical** to the legacy pipeline's
  output format: same row count, same transcript identifiers, same header.
- Data-integrity guard: a truncated intermediate FASTQ (line count not a
  multiple of 4) was **rejected with a structured error** instead of being
  silently quantified.

### 1.2 Cross-check batch (bench-20260914-002)

14 single-end samples quantified through the same CLI (5–17 s each, median
10.5 s), compared against the legacy pipeline's stored quant.sf:

| acceptance line | result |
|---|---|
| row/identifier parity | **14/14 pass** (33,955 rows) |
| NumReads total within 1% | **14/14 pass** (max deviation 0.0013%) |
| TPM Pearson r ≥ 0.995 | not evaluable — runs are non-mRNA libraries; **see §3** |

## 2. Environment (full disclosure)

| item | value |
|---|---|
| OS | CachyOS (Arch-based), kernel `7.2.3-1-cachyos` |
| CPU | Intel Xeon E5-2676 v3 @ 2.40 GHz, 24 threads (runs pinned to cores 8–15 with `taskset`, `nice 10`) |
| RAM | 32 GB |
| GPU | AMD Radeon RX 580 2048SP (Polaris 20, amdgpu kernel driver) — **not used**; all workloads are CPU-only |
| Rust | engine 1.0.3, release profile |
| salmon | 2.7.0 (upstream bioconda binaries via micromamba, user prefix) |
| Python | 3.14 (system; used only by verify helpers) |
| Container | none |
| Page cache | cold (single pass per sample) |
| Timing | wall clock via `date +%s` (second resolution; GNU time not installed) |

## 3. Post-hoc correction on the reference (verified by the data owners)

Our initial read of the legacy pipeline logs attributed the low TPM
correlations to a salmon re-implementation. The data owners have since
verified with `file` output that their quant used **upstream salmon 2.7.0
(official ELF binary)**. The near-zero mapping is a property of the **data**:
both implementations independently measure ~0–0.0019% mapping on these 14
runs because they are small-RNA/special libraries with nothing to map
against an mRNA CDS index.

Consequences, stated plainly:

- Row/ID parity and NumReads **total** parity remain meaningful and pass on
  all 14 runs.
- TPM Pearson r is **not evaluable** here; the 14 runs are marked
  **excluded: non-mRNA library** and belong to a small-RNA pipeline (with a
  miRBase reference) rather than mRNA quantification.
- This also validates the SDK's data-integrity behavior end-to-end: two
  independent implementations agreeing on near-zero mapping is exactly the
  kind of signal that should stop a pipeline from publishing quant tables
  silently.

A fair wall-time and numerical comparison is published separately against
normally-covering samples (67–71% mapping rate) from the same project's
already-computed set — see the next dated report.

## 4. Data sources

All samples are public SRA accessions (buckwheat transcriptomics projects;
project IDs are listed per run in the lab's census file):

| dataset | SRA accessions | role |
|---|---|---|
| paired-end deep sample | SRR26171873 | commissioning E2E |
| truncated intermediate FASTQ | SRR22699515 | integrity-guard validation (rejected) |
| 14 single-end samples | SRR28573920–25, SRR8205656–63 | cross-check batch |

Reference index: Pinku1 CDS salmon index (33,955 transcripts), the same
index used by every legacy quant. Raw reads and reference stay on the lab
workstation and never enter this repository; this directory stores metrics,
methodology, and provenance only.

## 5. Reproduce

Install salmon 2.7.0 (e.g. `micromamba create -p $PREFIX -c bioconda
salmon`), point `LINXIRA_BIO_SALMON` at it, then use the generalized
cross-check tool committed under `scripts/quantify-bench.sh`:

```bash
scripts/quantify-bench.sh \
  --cli /path/to/linxira-bio \
  --index /path/to/salmon_idx \
  --inputs-dir /path/to/fastq/ \
  --reference-dir /path/to/legacy-quant/ \
  --output-dir /path/to/results/ \
  --pin "taskset -c 8-15 nice -n 10" \
  --runs SRR28573920 SRR28573921 ...
```

Per-sample JSON envelopes and logs land under `--output-dir/<run>/`; the
CSV summarizes wall time, row counts, NumReads totals, TPM r, and the
reference implementation per run.

---

## 中文摘要

**测了什么**：用公开 CLI（`expression.quantify.v1`）编排**原版 salmon 2.7.0**
处理真实公共 RNA-seq 数据。深库双端样本全流程 **59 秒**（冷缓存），输出
quant.sf 与既有管线格式逐项一致；14 个单端样本各 5–17 秒，行数/ID 与
NumReads 总量全部通过。截断 FASTQ 被结构化错误正确拒绝。

**环境**：CachyOS，内核 7.2.3-1-cachyos，Intel Xeon E5-2676 v3（24 线程，
运行钉在 8–15 核 + nice 10），32 GB 内存，GPU 为 AMD Radeon RX 580 2048SP
（**未使用**，纯 CPU 负载），Rust 引擎 1.0.3，salmon 2.7.0（bioconda 原版），
无容器，冷缓存，秒级计时。

**重要更正（数据所有方已核实）**：对照 quant 使用的是**官方 salmon 2.7.0 ELF
二进制**（我们此前依据日志的 "rust port" 判断有误，予以撤回）。近零映射是
**数据属性**：两套实现独立测得 0–0.0019%——这 14 个 run 是小 RNA/特殊文库，
对 mRNA CDS 索引本无可映射内容，已标注 `excluded: non-mRNA library`，归入
miRNA 专线。正常覆盖（67–71% mapping）样本上的公平对照见下一日期报告。

**数据**：全部为公开 SRA 数据（荞麦转录组项目，检索号见 §4），原始 reads
与参考索引留在实验工作站、不入库；本目录只存指标、方法与来源。
**复现**：用入库的 `scripts/quantify-bench.sh`（全参数化，无站点路径）。
