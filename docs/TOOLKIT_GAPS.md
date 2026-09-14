# Toolkit gap assessment: pipelines, three-way coverage, Rust crate sufficiency

> Purpose: answer three planning questions — (1) does the toolkit support
> multi-step chained analyses today and what is missing; (2) which of the
> 117 available capabilities still lack Python/R implementations; (3) is the
> Rust bioinformatics ecosystem sufficient for the SDK's value chain.
>
> 回答三个规划问题：联动分析现状与缺口、三端覆盖缺口清单、Rust 生信生态
> 是否够用。

## 1. Chained analysis (联动分析): status and gap

**Status**: every capability is single-step. Multi-step work happens through
skills (`skills/route-bio-analysis`), agents, or manual CLI chaining — there
is **no in-engine pipeline composition**: no DAG runner, no parameter
forwarding between steps, no intermediate artifact lifecycle, no resumable
batch state.

**Real chains our users run** (observed in the buckwheat project and
standard practice):

```
SRR → unpack → fastq QC → quantify → count matrix → normalize → PCA/QC
     → differential → volcano/enrichment → plots
VCF → stats → filter → normalize → annotate → compare
assembly-free variant chain, annotation → density/position → plots
```

**Gap to close (proposed `pipeline.run.v1`)**:

- recipe JSON: ordered steps, each step = (capability, inputs-from,
  parameters); edges declare artifact roles passed forward
- engine-managed intermediates (content-addressed, per-run directory,
  resumable — same semantics the lab batch already proved out)
- batch dimension: N samples × M steps, serial or bounded-parallel
- acceptance hooks: a step can carry the cross-check validators we already
  built (row parity / NumReads / TPM-r lines) so a chain fails loudly
- curated recipes shipped as skills: `bulk-rnaseq-from-sra`,
  `variant-qc-chain`, `annotation-profile-chain`

Value: this is the difference between "a box of tools" and "a toolkit" —
and it directly industrializes what we hand-ran on bio-lab this week.

## 2. Three-way coverage: 112 of 117 capabilities are Rust-only

Python/R packs exist for: `sequence.stats.v1`, `expression.pca.v1`,
`set.venn.v1`, `structure.pdb.summary.v1`,
`enrichment.overrepresentation.v1` (all parity-proven, several bit-exact).
R-side only: `expression.differential.v1` (DESeq2 pack).

Priority for the next three-way batches (by value × portability):

| tier | capabilities | rationale |
|---|---|---|
| 1 | `fastq.qc.v1`, `variant.stats.v1`, `expression.matrix.qc.v1`, `table.manipulate.v1`, `interval.intersect.v1` | pure-Rust algorithms; Python (stdlib) and R (base) ports are mechanical; high user frequency |
| 2 | `expression.normalize.v1`, `sequence.kmer.count.v1`, `set.upset.v1`, `genome.gene-density.v1`, `annotation.gxf.stats.v1` | same, slightly more surface |
| 3 | orchestrators (`similarity.*`, `msa.*`, `phylogeny.*`, `metagenomics.*`, `expression.quantify.v1`) | pack = "call the same native tool"; parity is about result parsing, lower statistical value but completes the claim |
| defer | `medical.*` research-use entrypoints, GUI-only viewers | wrappers, not analyses |

## 3. Is the Rust bioinformatics ecosystem sufficient?

Verdict: **yes for the SDK's value chain (~85% of shipped capability is
already Rust-native)**, with named gaps — most of which we already filled
ourselves:

| domain | ecosystem state | our position |
|---|---|---|
| format I/O (FASTQ/FASTA/BAM/CRAM/VCF/BIGWIG, gzip) | **strong**: needletail, noodles, bigtools, flate2(zlib-ng), gzp, niffler | adopt (see `RUST_NATIVE_ROADMAP.md` tier 1) |
| SRA | **weak/young**: ncbi-vdb-sys (Arc Institute), FDA sra-rust; no pure-Rust decoder | FFI + streamed pipeline (planned `--sra-decoder native`) |
| statistics/algorithms we ship | **partial**: rust-bio covers basics; no DESeq2-class NB GLM, no WGCNA, no GSEA null model in Rust | **already self-implemented** (deterministic ports, parity-proven vs R) |
| alignment/search science | none competitive (BLAST/DIAMOND/HMMER/minimap2) | keep external (tier 3) |
| phylogenetics / MSA / RNAfold / kraken2 / salmon | none competitive | keep external |

Conclusion: the ecosystem is sufficient as a **base**, insufficient as a
**substitute** for the science cores — which matches the SDK design: Rust
engine for deterministic statistics and I/O, controlled delegation for
domain tools, and our own ports where the ecosystem is silent.

## 中文摘要

**联动分析**：当前 117 项能力均为单步，无 DAG 编排器——链式分析靠 skill/agent
手工串。缺口：`pipeline.run.v1`（recipe JSON + 引擎托管中间产物 + 可断点续跑
+ N 样本×M 步批量 + 内置交叉验收钩子）与配套 recipe skills。这是"工具箱"变
"工具包"的关键一步，也是本周 bio-lab 手工批量直接工业化的对象。

**三端缺口**：117 项里仅 5 项三端齐 + 1 项 R 端（DESeq2）。优先级：纯算法类
先移（fastq.qc/variant.stats/matrix.qc/table/interval），编排类后移（对同一
原生工具做三端调用，验收价值低），medical 包装类缓行。

**Rust 生态充分性**：格式 I/O 强（needletail/noodles/bigtools/gzp）；SRA 弱
但已现 FFI 方案；统计科学核（DESeq2 类/WGCNA/GSEA）生态缺失——我们已自研并
与 R 逐位对齐；对齐/搜索/进化类无竞争力等价物——保持外调。结论：**作为底座
够用，作为科学核替代不够**，与 SDK"Rust 确定性引擎 + 受控外调 + 自研补位"
的架构判断一致。
