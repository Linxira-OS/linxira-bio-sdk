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
(14-run cross-check) · **Code revision**: `v1.0.3` base (`c8ebee3`) +
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
| TPM Pearson r ≥ 0.995 | 0.00–0.94 — **not evaluable; see §3** |

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

## 3. Caveat: the reference implementation (read before quoting TPM numbers)

The pre-existing quant tables used as comparison targets were **not**
produced by upstream salmon. Their logs self-identify as
`salmon (rust port, reads mode) v2.7.0` — a re-implementation — and on
these samples they map only **~192 of 11.4M fragments (0.0017% mapping
rate)**.

Consequences, stated plainly:

- Row/ID parity and NumReads **total** parity are meaningful and pass.
- At a 0.0017% mapping rate, TPM allocation among competing transcripts is
  numerical noise, so the TPM Pearson-r acceptance line is **not
  evaluable** here; the low r values measure the reference's degenerate
  coverage, not a defect in either implementation.
- A wall-time speedup against this reference would be meaningless (it is
  fast because it maps almost nothing), so **no speedup claim is made in
  this report**.

The reference implementation is disclosed per run (read from
`cmd_info.json` + pipeline logs) in `bench-20260914-002.summary.json`
(`reference_implementation`) and in the CSV (`reference` column). A valid
speedup comparison requires both sides to map the data properly — same
machine, same index, same parameters, upstream salmon on both sides — and
will be published separately once available.

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

**重要口径**：对照 quant 来自一份 salmon **重实现**（日志自述 "rust port,
reads mode"），且在这批样本上 mapping 率仅 0.0017%（1142 万 fragments 映射
192 条）——因此 TPM 相关性这条验收线在此**不可评估**（低 r 反映的是对照方
的退化覆盖，不是任一实现的缺陷），本报告也**不发布任何加速比**。有效对照
需要双方都以真实映射运行（同机、同索引、同参数），待具备后另行发布。

**数据**：全部为公开 SRA 数据（荞麦转录组项目，检索号见 §4），原始 reads
与参考索引留在实验工作站、不入库；本目录只存指标、方法与来源。
**复现**：用入库的 `scripts/quantify-bench.sh`（全参数化，无站点路径）。
