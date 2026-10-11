# 对标分析方法的差距清单 + 典型耗时与租赁窗口测算（2026-10-04）

> 结论：**可以提交成绩**，但要先把对标方要求的分析方法做好（Rust + Python + R 三实现），
> 成绩晚点再交。本文给出①提交机制 ②方法差距清单 ③典型耗时评估 ④租赁窗口测算。

## 一、"我们能向他们提交成绩吗？"

| 平台 | 能否提交 | 机制 |
| --- | --- | --- |
| OpenBenchmarking / PTS | **能** | 结果由 PTS 客户端上传到公开结果库；自研 test profile 经其包体系分发（官方明文支持新增测试） |
| GA4GH / GIAB + hap.py | **不是"提交"，而是"评估"** | 用第三方真值集评估自己的 VCF，数字自己公布；真值与评估器均为第三方，故结论可外引 |
| nf-core | **不能提交** | 只是拿它的第三方流程做同输入对照 |
| Benchmarks Game | 能（需先核对现行入口与许可） | 社区维护的语言级基准 |
| MLPerf | 现阶段不适配 | 无对应任务类别 |

**顺序纪律（用户 2026-10-04 定）**：先对标补齐分析方法（三语言实现），成绩与发文后置。

## 二、对标所需分析方法 vs 我们现状（差距清单）

对标对象：`nf-core/rnaseq`、`nf-core/sarek`、GIAB 变异评估链。

| 流程步骤 | 对标工具 | 我们的能力 | 语言覆盖 | 缺口/动作 |
| --- | --- | --- | --- | --- |
| 读段 QC / 去接头 / 修剪 | fastp、FastQC | `fastq.qc.v1`、`fastq.trim.v1`、`fastq.adapter.v1` | QC 三语言✅；trim/adapter Rust | trim/adapter 补 Py/R 对照实现 |
| 比对 + 排序 | bwa-mem、minimap2、samtools | `alignment.short-read.v1`、`alignment.long-read.v1` | Rust（编排原生） | 保留编排；登记"生态强依赖，不补三实现"例外 |
| 去重 / 窗口深度 / 插入片段 | samtools markdup、mosdepth | `alignment.markdup.v1`、`alignment.window-depth.v1` | Rust（编排原生） | 插入片段指标（ROADMAP 已排期）补齐 |
| 变异检出 | bcftools、GATK | `variant.call.v1` | Rust（编排原生） | **多样本联合分型**（ROADMAP 已排期）补齐 |
| 变异过滤/归一化/统计 | bcftools norm/filter、vcftools | `variant.filter/normalize/stats/compare/to-table.v1` | stats 三语言✅ | — |
| 变异注释 | VEP、snpEff | `variant.annotate.v1` | Rust（编排原生） | 核对后端与离线库可用性 |
| 表达定量 | salmon、featureCounts、kallisto | `expression.quantify.v1` | Rust（编排原生） | kallisto/featureCounts 对照实现（ROADMAP 有记） |
| 矩阵 QC / 归一化 / 降维 / 聚类 | scikit-learn、prcomp、limma | `expression.matrix.qc/normalize/pca/cluster.v1` | PCA 三语言✅ | cluster/normalize 补 Py/R 对照 |
| 差异表达 | DESeq2、limma、PyDESeq2 | `expression.differential.v1` | **仅 R** | **补 Python（PyDESeq2）→ 三语言** |
| 富集（ORA/GSEA） | clusterProfiler、gseapy、fgsea | `enrichment.overrepresentation.v1`、`enrichment.go/kegg/gsea.v1` | ORA 三语言✅ | GSEA 补 Py/R 对照 |
| 聚合报告 | MultiQC | ❌ | — | **报告层一期（ROADMAP 已登记）** |
| 参考/注释数据管理 | iGenomes、参考索引 | ❌（手工） | — | **参考基因组管理（ROADMAP 批次 H）** |
| 绘图后端 | matplotlib、ggplot2 | `plot.render.v1` | 三语言✅（+Rust SVG 回退） | 补 Rust 原生后端（ROADMAP 有记） |

统计：**三语言齐全的能力 10/132**（`fastq.qc`、`expression.pca`、`expression.batch-correct`、
`enrichment.overrepresentation`、`sequence.stats`、`sequence.ssr`、`set.venn`、
`structure.pdb.summary`、`variant.stats`、`plot.render`）。优先补齐顺序：
① `expression.differential` 补 Python（PyDESeq2）→ 覆盖对标最核心的 DE 步骤；
② `fastq.trim/adapter` 补 Py/R 对照；③ GSEA 补 Py/R；④ 报告层一期；⑤ 联合分型 + 插入片段指标。

## 三、典型耗时评估（对标方给出的时间量级）

> 口径：30x WGS ≈ 3 Gbp 基因组、约 100 GB FASTQ；WES ≈ 5-8 GB FASTQ。
> 标注 **[pub]** 为公开资料，**[est]** 为按吞吐推算的工程估计（须在窗口实测替代）。

| 步骤 | 30x WGS（CPU） | WES | 来源/说明 |
| --- | --- | --- | --- |
| 读段 QC + 修剪（fastp 类） | 10-30 min | 1-3 min | [est] 单遍流式，IO 主导 |
| 比对（bwa-mem，16-32 线程） | 4-8 h | 20-40 min | [est] 依线程与磁盘 |
| 排序 + 去重（samtools） | 1-2 h | 5-10 min | [est] |
| 变异检出（bcftools mpileup+call，16 线程） | 6-12 h | 40-90 min | [est] |
| 变异检测（GATK HaplotypeCaller） | 10-20 h | 1-3 h | [pub] 云端基线 |
| 注释（VEP/snpEff 离线） | 1-3 h | 10-30 min | [est] |
| **整链合计（CPU）** | **20-40 h** | **2-4 h** | [pub] OCI 基线 30x WGS ">20 h"；NVIDIA 对比口径 CPU ≈30 h |
| 整链（GPU 加速，Parabricks 类） | ~10 min | — | [pub] 与我们无关（不自研 GPU 比对） |
| hap.py 真值评估（16-32 线程） | 1-2 h | 10-20 min | [est] |
| RNA-seq 定量（salmon，3000 万读） | 5-20 min | — | [est] |
| 富集/GSEA（毫秒-秒级输入） | < 1 min | < 1 min | 已有实测（本轮 R4 台账） |

## 四、租赁窗口测算

| 目标 | 数据/配置 | 预估机时 | 建议窗口 |
| --- | --- | --- | --- |
| **准确性基线（推荐起步）** | GIAB HG002 **WES**（2-4 h 整链）+ hap.py | 3-5 h | **6 h 窗口，16-32 核** |
| 准确性（WGS，分染色体并行） | 30x WGS 按 chr 切分，32-64 核 | 4-8 h | 8-12 h 窗口，RAM ≥64 GB |
| 准确性（WGS，单机整链） | 30x WGS 不切分，16-32 核 | 20-40 h | **24-48 h 窗口**（贵，建议先做 WES） |
| 性能 profile（PTS 类） | 种子化输入，单档 1-10 min | 1-2 h（含环境） | 3 h 窗口 |
| 三语言对照复跑 | 10 项三语言能力的多后端跑分 | 1-2 h | 与上面任一窗口合并 |

**结论与建议**：
1. 起步不要租 WGS 整链窗口——先用 **WES（3-5 h）** 把"准确性 + 三语言对照"两件事做完；
2. WGS 走**按染色体切分**（把 20-40 h 压到 8-12 h），并记录切分口径；
3. 性能 profile 与三语言对照可搭在同一窗口（成本几乎不增加）；
4. 每个窗口的实测值必须回填替代本文的 [est] 值，并声明数据规模与线程数。

## 五、待办

- [ ] `expression.differential` 补 Python（PyDESeq2）→ 三语言
- [ ] `fastq.trim` / `fastq.adapter` 补 Py/R 对照；GSEA 补 Py/R
- [ ] 报告层一期（MultiQC 类聚合）；插入片段指标；联合分型
- [ ] GIAB WES 数据 + 真值下载与版本冻结（先 WES 后 WGS）
- [ ] 租用窗口：WES 整链 + hap.py + 三语言对照（6 h）
