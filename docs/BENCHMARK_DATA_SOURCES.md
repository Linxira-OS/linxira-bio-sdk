# Benchmark 4TB 验证数据集来源（Data Sources / Provenance）

> 用途：本文件是 benchmark 与主站展示**数据指标 / 性能指标**时的**数据来源引用清单**。
> 凡在仓库或主站展示某批 benchmark 数据的性能数字（wall-time 加速比、峰值内存、输出体积、
> 数值一致性等），都必须能通过本文件追溯到数据来源平台的链接与检索号，做到「数字可复现、
> 来源可查证」。

## 1. 这批数据是什么（角色）

- **唯一用途**：作为 benchmark 输入，在 Linux 服务器上分别喂给 Linxira Rust 实现与主流原生工具
  （Python / R / 原生命令行），量化 Rust 相对主流方法的性能提升。
- **数据治理**：4TB 原始数据、SRR/SRA 归档、受控或研究数据**永不进 Git 仓库**；仅回传
  `benchmark-results/<date>/summary.json + summary.md` 与报告图表。
- **回传物**：性能指标（加速比、内存、输出体积、一致性），不含原始数据本体。
- **既有基线**：仓库 fixture 上的三端（Rust/Python/R）一致性基线见
  `benchmark-results/2026-09-13/`（summary.json + summary.md + 各能力完整报告，
  含 Windows 宿主机/WSL 环境披露）。该基线使用仓库内置小样本，**不得**与本文件的
  4TB 真实数据跑分混排对比。

## 2. 数据类别（用于 benchmark 归类与后端偏好）

| 类别 | 含义 | 典型格式 / 示例 |
|---|---|---|
| SRR | 原始测序 / 需解压 | SRA 归档（`.sra`）、FASTQ（含 `.gz/.bz2/.xz/.zst/.7z/.zip`）、BAM/CRAM |
| 注释 | 基因/变异注释 | GFF3 / GTF / BED / VCF / BCF |
| 矩阵 | 表达 / 计数矩阵 | TSV / CSV / H5ad / mtx |
| 结构 | 三维结构 | PDB / mmCIF / AlphaFold |
| 序列 | 序列文件 | FASTA / FASTQ |
| 样本表 | 临床 / 分组 / 元数据 | TSV / CSV |
| 其他 | 化学 / 质谱等 | SDF / mzML / RAW / SMILES |

## 3. 数据来源引用格式（每一项 dataset 必填字段）

| 字段 | 说明 | 示例 |
|---|---|---|
| 数据集名称 | 数据集/研究的正式名称 | 1000 Genomes Project Phase 3 |
| 数据类别 | 见 §2 | SRR / 注释 / 矩阵 / 结构 / 序列 / 样本表 / 其他 |
| 来源平台 | 托管该数据的平台 | NCBI SRA / ENA / GEO / Ensembl / RCSB PDB |
| 来源链接 | 可访问的稳定 URL | `https://www.ncbi.nlm.nih.gov/sra` |
| 检索号 / Accession | 唯一检索号 | `SRRxxxxxxx` / `GSEyyyyyy` / `PRJNA……` |
| 许可 / 使用条款 | 数据许可或引用要求 | 公开 / CC0 / 需引用原文 |
| 规模 | 该数据集的体量 | `~50 GB` / `~2M reads` |
| 对应能力 / 分析 | 该数据用于验证哪些能力 | `fastq.qc.v1` / `expression.differential.v1` |
| 备注 | 其他说明 | 配对端 / 单端 / 是否需解压 |

## 4. 常见来源平台（稳定公开数据库入口）

> 具体数据集级链接待用户转发原始数据清单后，在 §5 逐项补齐。以下为常见托管平台入口，
> 供引用与复核。

| 平台 | 类型 | 入口链接 |
|---|---|---|
| NCBI SRA | 原始测序（SRR/SRA） | https://www.ncbi.nlm.nih.gov/sra |
| EBI ENA | 原始测序（Reads） | https://www.ebi.ac.uk/ena |
| NCBI GEO | 表达/微阵列/测序数据集 | https://www.ncbi.nlm.nih.gov/geo |
| ArrayExpress | 表达数据集 | https://www.ebi.ac.uk/biostudies/arrayexpress |
| Ensembl | 基因组注释 GFF/GTF/FASTA | https://www.ensembl.org |
| GENCODE | 基因注释 GTF/FASTA | https://www.gencodegenes.org |
| UCSC Genome Browser | 注释 / 基因组 | https://genome.ucsc.edu |
| NCBI RefSeq | 参考序列 / 注释 | https://www.ncbi.nlm.nih.gov/refseq |
| dbSNP | 变异 | https://www.ncbi.nlm.nih.gov/snp |
| ClinVar | 临床变异 | https://www.ncbi.nlm.nih.gov/clinvar |
| gnomAD | 人群变异频率 | https://gnomad.broadinstitute.org |
| RCSB PDB | 蛋白结构 PDB | https://www.rcsb.org |
| AlphaFold DB (EBI) | 蛋白结构预测 | https://alphafold.ebi.ac.uk |
| UniProt | 蛋白序列 / 注释 | https://www.uniprot.org |
| InterPro | 蛋白结构域 / 功能 | https://www.ebi.ac.uk/interpro |
| GTEx | 组织表达矩阵 | https://gtexportal.org |
| TCGA (NCI GDC) | 癌症多组学 | https://portal.gdc.cancer.gov |
| Human Cell Atlas | 单细胞数据 | https://www.humancellatlas.org |
| 10x Genomics Datasets | 单细胞 / 空间转录组示例数据 | https://www.10xgenomics.com/resources/datasets |
| MetaboLights | 代谢组学 | https://www.ebi.ac.uk/metabolights |
| PRIDE | 蛋白质组学 / 质谱 | https://www.ebi.ac.uk/pride |

## 5. 数据来源清单（按数据类别 · 待补齐）

> **状态：待补充。** 用户将转发原始数据清单文档，届时按 §3 的字段格式逐项填写以下各表，
> 并把对应数据集的实际来源链接 / 检索号填充进「来源链接」与「检索号」两列。

### 5.1 SRR（原始测序 / 需解压）

| 数据集名称 | 来源平台 | 来源链接 | 检索号 | 许可 | 规模 | 对应能力 | 备注 |
|---|---|---|---|---|---|---|---|
| 待补充 | — | — | — | — | — | — | — |

### 5.2 注释（GFF/GTF/BED/VCF）

| 数据集名称 | 来源平台 | 来源链接 | 检索号 | 许可 | 规模 | 对应能力 | 备注 |
|---|---|---|---|---|---|---|---|
| 待补充 | — | — | — | — | — | — | — |

### 5.3 矩阵（表达 / 计数）

| 数据集名称 | 来源平台 | 来源链接 | 检索号 | 许可 | 规模 | 对应能力 | 备注 |
|---|---|---|---|---|---|---|---|
| 待补充 | — | — | — | — | — | — | — |

### 5.4 结构（PDB / mmCIF）

| 数据集名称 | 来源平台 | 来源链接 | 检索号 | 许可 | 规模 | 对应能力 | 备注 |
|---|---|---|---|---|---|---|---|
| 待补充 | — | — | — | — | — | — | — |

### 5.5 序列（FASTA / FASTQ）

| 数据集名称 | 来源平台 | 来源链接 | 检索号 | 许可 | 规模 | 对应能力 | 备注 |
|---|---|---|---|---|---|---|---|
| 待补充 | — | — | — | — | — | — | — |

### 5.6 样本表（临床 / 分组）

| 数据集名称 | 来源平台 | 来源链接 | 检索号 | 许可 | 规模 | 对应能力 | 备注 |
|---|---|---|---|---|---|---|---|
| 待补充 | — | — | — | — | — | — | — |

### 5.7 其他（化学 / 质谱）

| 数据集名称 | 来源平台 | 来源链接 | 检索号 | 许可 | 规模 | 对应能力 | 备注 |
|---|---|---|---|---|---|---|---|
| 待补充 | — | — | — | — | — | — | — |

## 6. 引用规范（如何在本仓库 / 主站引用）

1. 任何 benchmark 结果页 / 数据优势点卡片，展示某批数据的性能指标时，须附该数据集的
   「来源平台 + 来源链接」（取自 §5），不可只给数字不给来源。
2. `benchmark-results/<date>/summary.json` 的每条记录应携带 `dataset` 字段与 `source_url`，
   与 §5 清单一一对应。
3. 数据许可：若某数据集要求引用原文或限制商业使用，须在对应表格「许可」列注明，并在展示处同步披露。