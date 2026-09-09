# Linxira Bio SDK — 三语言统一输出 · 双后端绘图 · Benchmark 蓝图

> 决策依据（用户已拍板）：
> 1. **交付边界** = 完整蓝图 + 分批落地（每阶段可独立验证、单独验收）。
> 2. **三语言对齐机制** = **三份独立实现**：每个能力 Rust / Python / R 各自实现算法，
>    仅共享输入/输出 JSON schema，靠 benchmark 交叉验证数值一致。
> 3. **Benchmark 是核心重点**：4TB 真实数据、Linux 服务器跑分、与主流原生工具做速度对比，
>    结果回传仓库与主站，输出"数据优势点"报告。

---

## 1. Summary（目标一句话）

把 Linxira Bio SDK 从"Rust 单一实现、JSON 单一输出"升级为**三语同构平台**：
Rust / Python / R 对同一能力各有独立实现，产物既有一致的结构化 JSON，也有与
BWA / samtools / bcftools / DESeq2 / matplotlib / ggplot2 等原生工具**完全相同的传统格式**；
绘图保留 Rust SVG 并补齐 matplotlib / ggplot2 原生后端；新增跨平台路径解析、工作区分类
输出、时间戳防覆盖、图片查看器，以及支撑 4TB 数据跑分和主站数据优势展示的
**benchmark 框架**。整项工作按 6 个阶段分批交付。

---

## 2. Current State Analysis（现状盘点）

### 2.1 已具备、可直接复用的底座

| 底座 | 位置 | 关键能力 | 现状 |
|---|---|---|---|
| 协议层 | [lib.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-protocol/src/lib.rs) | V1/V2 envelope、`OutputArtifact`、`BioDataFormat`、`ProvenanceV2`(含 `started_at/finished_at/input_sha256/software/command`) | 已有，但**时间字段从未被填充** |
| 导出层 | [export/lib.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-export/src/lib.rs) | CSV/TSV/JSON/JSONL/XLSX 原子导出、`Table::from_json` 规整化 | 已有，但**只支持 5 种表格格式，缺生信传统格式(FASTA/BED/VCF/SAM/GFF/SVG/PNG)** |
| 绘图层(Rust) | [scientific_visualization.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-core/src/scientific_visualization.rs) | volcano / motif-logo / synteny / annotation-structure / domain-architecture / enrichment 六个 SVG | 已有，保留 |
| 绘图预览(GUI) | [visualization.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/apps/linxira-bio-ui/src/visualization.rs) | bar/line/scatter/heatmap 能力感知图表 | 已有，是"表格类结果"的内嵌预览，**不是图片查看器** |
| 工作流执行 | [worker/workflow.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-worker/src/workflow.rs) | R/Python pack：manifest 契约、SHA256 校验、resume、输出目录解析、跨平台 `canonical_existing_input` | 已有，**runtime 只有 R/Python 两个，无 Java/benchmark** |
| 工作流 packs | [workflows/](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/workflows) | deseq2(R) / wgcna(R) / survival(R) / biopython(Py) / rdkit(Py) | 已有 5 个 pack，**覆盖面极小** |
| CLI | [cli/main.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-cli/src/main.rs) | 手写 dispatch，`--json` 开关 | 已有，无 benchmark 命令 |
| CI | [ci.yml](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/.github/workflows/ci.yml) | win/debian/arch 三平台 + 真实 E2E(DESeq2/survival/biopython/rdkit) + license-audit + publish | 已有，**需要叠加 benchmark 与绘图后端测试** |
| 主站 | `F:\Linxira-OS\Linxira-OS.github.io\public\bio-sdk\` | 官网（index.html 之前已增强） | 已有，**缺 benchmark 结果页** |

### 2.2 能力盘点（语言/格式/绘图三张表）

> 全量能力在 [catalog.json](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/capabilities/catalog.json)，
> available ≈ 90，planned 2（AF2/AF3）。

**(1) 语言落实情况**：当前 Rust ≈ 85 个能力；**Python 仅 2**（biopython 转换、rdkit 描述符）；
**R 仅 3**（deseq2、wgcna、survival）；**Java 0**。即"Rust 写的分析方法，对应 Python/R
实现几乎全空"。

**(2) 输出格式情况**：Rust 能力全部只输出 JSON（V2 envelope）；传统格式（FASTA/FASTQ/
BED/GFF3/VCF/SAM）仅在少数能力内部临时生成，**没有统一的传统格式写出器**；Python/R
pack 也只输出 JSON 与 CSV 家族。

**(3) 绘图情况**：仅 6 个 Rust SVG 能力；GUI 有表格内嵌预览；**无 matplotlib/ggplot2 后端、
无图片查看器、无统一绘图参数（PlotSpec）**。

### 2.3 待补充数据方法总清单（按领域 · 全量缺口）

> **数据类别**（用于 benchmark 归类与后端偏好）：
> **SRR**=原始测序/需解压 · **注释**=GFF/GTF/BED/VCF · **矩阵**=表达/计数矩阵 ·
> **结构**=PDB/mmCIF · **序列**=FASTA/FASTQ · **样本表**=临床/分组表 · **其他**=化学/质谱等。
> **优先级**：🔴 首批(M4)落地 → 🟠 二批 → 🟡 三批。**绘图** 列标注是否需 PlotSpec 双后端出图。

#### 🔴 首批（阶段 4 落地，benchmark 主战场）

| 能力 | 生态对标 | 实现(Rust/Py/R) | 数据类别 | 绘图 |
|---|---|---|---|---|
| 转录定量 featureCounts/salmon/kallisto | HTSeq/salmon | Rust 编排+原生 quant / Py / R | SRR+BAM | — |
| 单细胞 RNA：质控/降维/聚类/marker | Seurat(R)/Scanpy(Py) | **R + Py 双实现** | 矩阵(H5ad) | ✅UMAP/散点 |
| peaks calling + ChIP/ATAC 富集 | MACS2/SEACR | 封装原生 MACS2 | SRR+BAM | ✅峰型图 |
| 甲基化 BS-seq / DMR 检测 | Bismark/MethylKit | Py + R | SRR(BS-seq) | ✅DMR图 |
| 从头组装 de novo | SPAdes/miniasm | 封装原生 SPAdes | SRR | — |
| 组装质控 N50/完整性/污染 | QUAST/CheckM/BUSCO | Rust 或 Py | SRR组装 | ✅统计条图 |
| 表达定量归一化(TPM/FPKM/RPKM) | 自研 | Rust / Py / R | 矩阵 | — |
| 差异可变剪接/异构体 | DEXSeq/rmats | **R** | SRR+BAM | ✅剪接图 |

#### 🟠 二批

| 能力 | 生态对标 | 实现(Rust/Py/R) | 数据类别 | 绘图 |
|---|---|---|---|---|
| 16S 扩增子 DADA2/ASV/分类 | QIIME2/DADA2 | **R(DADA2)** | SRR(16S) | ✅bar/树 |
| 宏基因组 组装/分箱/功能谱 | metaSPAdes/MaxBin/HUMAnN | Py | SRR | ✅丰度图 |
| 群体遗传 Fst/PCA/STRUCTURE | plink/adegenet | **R(adegenet)** | 变异 | ✅PCA/结构 |
| 结构变异 SV/CNV 检测 | Delly/CNVkit | 封装原生 | SRR+BAM | ✅断点图 |
| CNV 检测(靶向/外显子) | CNVkit/GATK | Py/R | BAM | ✅CNV图 |
| GWAS + 曼哈顿/QQ图 | plink/GAPIT | **R** | 变异+样本表 | ✅曼哈顿 |
| Hi-C 接触矩阵/TAD 检测 | HiC-Pro/HiCExplorer | Py | SRR(Hi-C) | ✅热图/染色质 |
| 蛋白信号肽/跨膜/亚细胞 | Phobius/TMHMM/DeepLoc | Py | 序列 | ✅拓扑图 |
| 蛋白家族/结构域功能注释 | InterProScan/Pfam | 封装原生 | 序列/结构 | ✅域图 |
| 通路图叠加(富集映射) | pathview/KEGG | **R** | 富集结果 | ✅通路图 |

#### 🟡 三批

| 能力 | 生态对标 | 实现(Rust/Py/R) | 数据类别 | 绘图 |
|---|---|---|---|---|
| 引物设计(Tm/GC/二级结构) | Primer3 | Rust + Primer3 | 序列 | — |
| 密码子使用/偏好 | EMBOSS cusp | Rust | 序列 | ✅bar |
| 两两序列比对(SW/NW/贪心) | parasail/EMBOSS | Rust | 序列 | ✅点图 |
| 通用统计检验(t/卡方/ANOVA/相关) | scipy.stats | Py | 样本表/矩阵 | — |
| 基因集富集(超几何/卡方)补充 | clusterProfiler | **R** | 富集 | ✅bar |
| 生存分析/COX/Lasso 蛋白组学 | survival/glmnet | **R** | 样本表 | ✅KM曲线 |
| 代谢组学归一化/差异代谢物 | XCMS/MetaboAnalyst | R/Py | 矩阵(质谱) | ✅火山/箱线 |
| 质谱肽段鉴定/定量 | MaxQuant/DIANN | 封装原生 | 其他(RAW/mzML) | ✅色谱 |
| 药物基因组注释(星等位基因) | PharmGKB | Py/R | 变异 | — |
| 空间转录组聚类/去卷积 | Seurat spatial/Squidpy | Py/R | 矩阵(img) | ✅空间图 |
| 三维基因组 A/B 区室/TAD | HiCExplorer/cooler | Py | SRR(Hi-C) | ✅染色质图 |
| RNA 编辑/修饰检测 | REDItools/JACUSA2 | 封装原生 | SRR | ✅位点图 |

> 以上约 30 项为**待补充**方法；叠加现有 ≈90 能力，形成完整版图。每项落地时
> 遵循"四同步"：schema + SKILL + 中英文文档 + E2E 测试。

---

## 3. 核心设计：三大统一机制

### 3.1 统一输出框架（OutputSpec）

**原则**：能力的"权威结果"永远是 V2 JSON envelope（结构规范、便于后续工作流/Agent 消费）；
在此之上，一个**通用写出器**把 JSON 结果落成与原生工具**逐字节对齐**的传统格式。
方向可逆：原生工具的传统输出也能被解析回统一 JSON。

- 新增 crate `engine/crates/linxira-bio-output/`：
  - `OutputSpec`：声明一次分析的"目标产物清单"（角色 → 格式 → 列名/表头 → 坐标轴 → 路径）。
  - `BioDataWriter`：为每个 `BioDataFormat` 提供写出器（见下表），全部遵循原子写（复用
    `linxira-bio-export::write_atomic_bytes` 的临时文件 + persist 模式）。
  - `BioDataReader`：传统格式 → JSON 的解析器（benchmark 反向对照 + 结果回灌）。

| BioDataFormat | 传统写出器对齐目标 | 关键字段（表头/列名可由 OutputSpec 覆盖） |
|---|---|---|
| Fasta/Fastq | seqkit/标准 | 序列头、序列、质量行；宽度折行 |
| Bed | bedtools | chrom/start/end/name/score/strand |
| Gff3/Gtf | gffread | seqid/source/type/start/end/score/strand/phase/attrs |
| Vcf/Bcf | bcftools | CHROM/POS/ID/REF/ALT/QUAL/FILTER/INFO/FORMAT/样本 |
| Sam/Bam | samtools | QNAME/FLAG/RNAME/POS/MAPQ/CIGAR/... |
| Csv/Tsv | 现有 export | 列名可显式指定 + 表头排序 |
| Xlsx/Jsonl/Parquet | 现有 export / pyarrow | 复用 export crate |
| Svg/Png/Pdf | matplotlib/ggplot2 | 见 3.2 PlotSpec |

### 3.2 统一绘图参数（PlotSpec）+ 双后端

**原则**：绘图走**原生内容**（matplotlib / ggplot2），**保留现有 Rust SVG** 用于本地零依赖场景。
README 明确声明："绘图默认使用原生 Python(matplotlib) / R(ggplot2) 后端；Rust SVG 作为
零依赖回退保留，待引入成熟 Rust 绘图包后再补齐 Rust 后端。"（见阶段 1 的 README 改动）

- `PlotSpec`（新增到 protocol 或 output crate）：
  ```yaml
  title / subtitle / x_label / y_label / x_range / y_range / x_log / y_log /
  grid / theme(dark|light|publication) / palette / legend / figure{width,height,dpi} /
  font{family,size} / output{format: svg|png|pdf, interactive:false}
  ```
- **后端分派**：Rust 数据组装好 PlotSpec + 数据 JSON → 按 `output.format` 选择
  matplotlib pack / ggplot2 pack / Rust SVG 渲染；三个后端消费**同一份 PlotSpec**，
  保证"改一处坐标轴、三端一致"。
- **原生格式对齐**：`png/pdf` 优先 matplotlib(位图/矢量出版)；`svg` 三端皆可；
  `--output-format` 未指定时代码按能力默认（图表能力默认 svg，插入 matplotlib 优化）。

### 3.3 Benchmark 框架（核心重点）

**目标**：同一份输入，分别用（A）Linxira Rust 实现、（B）原生 R/Python 实现（三份独立实现之一），
在**同一 Linux 服务器**上测速与比结果，产出可回传仓库、可上主站的量化报告。

> **4TB 数据的角色（关键澄清）**：用户手上的 4TB 数据是**性能验证基准数据集**，
> 不是要回传的内容。它的唯一用途是作为 benchmark 的输入，在 Linux 服务器上分别喂给
> Rust 实现与原生工具（Python/R/native），量化出 **Rust 相对主流方法的性能提升
> （wall-time 加速比、峰值内存节省、输出体积、数值一致性）**。最终**只回传性能指标与
> 加速比数字**（`summary.json` / `summary.md` / 图表），4TB 原始数据与 SRR 归档永不上传。

- 新增能力 `benchmark.run.v1` + CLI 子命令 + GUI 按钮（见阶段 2）。
- **收集维度**（写入 `provenance.started_at/finished_at` 与 benchmark 专有字段）：
  - wall-time（毫秒）、CPU time、峰值 RSS（内存）、磁盘 I/O 读/写字节、输出体积。
  - 数值一致性：结构化字段逐项 diff（浮点容差）、字符串全等、表格 sha256。
- **资源追踪实现**：
  - Rust 侧：`std::time::Instant` + `/usr/bin/time -v`（Linux）或 self-tracking；
    Linux 上优先外部 `/usr/bin/time` 捕获精确 RSS。
  - R/Python 侧：脚本头尾记录时间与（Python `resource` / R `gc()` + `/proc/self/status` 峰值）。
- **数据优势点报告**：对每个能力输出一张对比表（耗时、内存、输出一致、加速比），
  汇总为 `benchmark-results/<date>/summary.json` + `summary.md`，回传仓库并在主站渲染。

### 3.4 默认后端选择（benchmark 结果驱动，用户可以覆盖）

**目标**：benchmark 跑出"谁更快"之后，为**每个能力的每个后端**沉淀一个运行时偏好表，
能力默认按最优后端执行；用户在"开始运算"界面仍可手动切换。

- 新增 `runtime-preferences.json`（仓库内，随 benchmark 结果更新）：
  ```json
  {
    "schema_version": "1",
    "preferences": [
      {
        "capability": "expression.differential.v1",
        "default_backend": "r",
        "measured": { "rust": {"wall_ms": 0, "peak_rss_mb": 0},
                     "python": {"wall_ms": 0, "peak_rss_mb": 0},
                     "r": {"wall_ms": 0, "peak_rss_mb": 0} },
        "sampled_on": "2026-08-07",
        "dataset_class": "bulk-rna"
      }
    ]
  }
  ```
- **选择规则**：`default_backend` = 该能力在同类数据上 wall-time 最快且结果一致的后端；
  `dataset_class` 允许按数据类别（注释 / 表达矩阵 / SRR 原始测序 / 结构）分别记录偏好
  （用户强调"4TB 数据分别适用于不同分析"）。
- **用户覆盖**：GUI 与 CLI 均以 `--backend <auto|rust|python|r>` 覆盖 `auto` 默认。
- 落地位置：`engine/crates/linxira-bio-worker` 读取偏好表；能力分发在无显式指定时
  走 `default_backend`。

### 3.5 输入数据探测与解压（注释 / SRR / 压缩）

**目标**：4TB 数据混杂（注释、普通数据、SRR/SRA 测序数据、各类压缩），统一做
**格式探测 → 按需解压 → 送分析**，用户无感。

- 新增 `engine/crates/linxira-bio-output/src/format_probe.rs`（或并入 path_resolver）：
  - `probe_format(path)`：按魔数 + 扩展名双路探测（不只看扩展名），覆盖：
    - FASTA / FASTQ（含 `.gz`/`.bgz`/`.bz2`/`.xz`/`.zst`）
    - BAM/CRAM(SAM header)、VCF(.vcf/.vcf.gz/.bcf)、BED、GFF3/GTF
    - SRA 归档 `.sra`（魔数 `NCBISRA` / `SRA`）
    - 通用压缩（gzip `1f 8b`、bzip2 `BZh`、xz `fd 37 7a 58 5a 00`、zstd `28 b5 2f fd`、zip `PK`）
  - `ensure_decompressed(path)`：自动流式解压到工作区临时目录（复用现有
    `CompressionFormat` 枚举），SRA 走 `fastq-dump`/`fasterq-dump`（先探测 `prefetch`/`vdb-config`）。
- SRR 数据专项：`sra.unpack.v1`（规划能力）封装 `fastq-dump --split-3`，输出配对 FASTQ，
  再交给 FASTQ 能力与 benchmark。若为 SRA 归档需先 `sra` 工具链，纳入
  `environment` 能力探测与 `native_tools` 白名单。
- 与 3.1 打通：探测出的格式直接送入 `BioDataReader`/各能力，避免"先解压再手工指定格式"。

---

## 4. Proposed Changes（分批落地，6 个阶段）

> 每个阶段独立可验证：均以 `cargo test` + `validate-repository.py` + CI 绿为验收门槛。

### 阶段 0 —— 统一输出框架（OutputSpec）+ 跨平台路径 + 工作区输出

**目标**：打通"JSON 权威 + 传统格式双向"与"规范输出路径"，无新分析功能。

**文件改动**：

0. **根目录 `ROADMAP.md`（正式计划文档，执行阶段首个交付物）**：把本蓝图的
   「长期计划（见 §5）」与「阶段性检查标准（见 §6）」落为仓库根目录的长期计划文件，
   作为唯一项目路线图（与 CAPABILITY_TRACKING.md 配合，后者记录能力级增量）。

1. 新增 `engine/crates/linxira-bio-output/Cargo.toml` + `src/lib.rs`：
   - `OutputSpec`、`BioDataWriter`/`BioDataReader`、`PlotSpec` 类型定义。
   - 传统格式写出器：Fasta/Fastq/Bed/Gff3/Gtf/Vcf/Sam（v1 先做这 7 类 + 复用 export 的 Csv/Tsv/Xlsx）。
   - 原子写复用 `linxira-bio-export`。

2. 新增 `engine/crates/linxira-bio-output/src/path_resolver.rs`：
   - `resolve_input_path`（跨平台）：Windows 盘符/反斜杠、POSIX 绝对/相对、文件名含空格/中文、
     `~` 展开、`file://` 剥离、目录遍历与重复检测。
   - `workspace_root()` 解析：GUI 项目文件 → 工作区；CLI 用 `--workspace` 或当前目录。
   - `classify_output_dir(capability)`：按能力域名映射到 `workspace/analysis/<domain>/<capability>/`
     （如 `expression.differential.v1` → `analysis/expression/expression.differential.v1/`）。
   - `timestamp_suffix()`：`_YYYYMMDD-HHMMSS`；同名再撞时追加 `_2`、`_3`…（防覆盖，符合"同一批数据多次输出区分"）。

3. 改 [worker/workflow.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-worker/src/workflow.rs)：
   - `resolve_output_directory` 增加 `timestamp_suffix` 逻辑与 `classify_output_dir` 挂钩（默认路径）。
   - 填充 `provenance.started_at/finished_at`（现有字段，仅补赋值）。

4. 改 [cli/main.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-cli/src/main.rs)：
   - 新增 `export bio <input.json> <output.fa|.bed|.vcf|.sam|.gff>` 子命令，走 `BioDataWriter`。

5. 新增 `schemas/output-spec.schema.json`、`schemas/plot-spec.schema.json`；补测试 fixture：
   `tests/fixtures/output/`（tiny.fa → bed/vcf 期望输出）。
6. 新增 `engine/crates/linxira-bio-output/src/format_probe.rs`：
   - `probe_format`（魔数+扩展名）与 `ensure_decompressed`（流式解压 gzip/bzip2/xz/zstd/zip，
     SRA `.sra` 先探测 `fastq-dump`/`fasterq-dump`、`prefetch`/`vdb-config`）。
   - CLI 加 `import probe <path>` 供用户查看探测结果。

**验证**：`cargo test -p linxira-bio-output`；`cargo run -p linxira-bio-cli -- export bio` 生成
与 `bedtools`/`samtools` 参照 kwargs 一致的输出；`import probe` 对 `.sra/.gz/.vcf.gz` 样本返回
正确格式与解压路径；`validate-repository.py` 过。

---

### 阶段 1 —— 双后端绘图（matplotlib/ggplot2 packs + 图片查看器）

**目标**：图表能力同时产出原生 matplotlib/ggplot2 图，GUI 内置图片查看器。

**文件改动**：

1. 新增 `workflows/org.linxira.visualization-matplotlib/`（Python pack，manifest + `src/render.py` +
   `requirements.lock`）：消费 PlotSpec JSON → matplotlib 渲染 svg/png/pdf。
2. 新增 `workflows/org.linxira.visualization-ggplot2/`（R pack，`src/render.R` +
   `dependencies.lock.json`）：同一 PlotSpec → ggplot2 渲染。
3. 改 [scientific_visualization.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-core/src/scientific_visualization.rs)：
   让 6 个 Rust SVG 渲染函数接受/输出 PlotSpec 结构（向后兼容，旧参数走默认 PlotSpec）。
4. 改 [apps/linxira-bio-ui/src/visualization.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/apps/linxira-bio-ui/src/visualization.rs) + [main.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/apps/linxira-bio-ui/src/main.rs)：
   - 新增 `ImageGallery`（图片查看器）：翻上一张/下一张、滚轮/按钮放大缩小、适配窗口、PNG/SVG 微浏览。
   - 图表结果页改为：内嵌预览 + "打开图片查看器"按钮。
5. 改 [README.md](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/README.md)：
   在绘图相关段落加一句声明（3.2 的原文措辞）。
6. 新增 `schemas/plot-render-result.schema.json`；绘图 pack 各含测试。

**验证**：`cargo test`；matplotlib/ggplot2 pack 运行冒烟；GUI 手动验证图片查看器翻页/缩放。

---

### 阶段 2 —— Benchmark 框架（核心重点 + Linux 服务器 + 主站）

**目标**：落地 benchmark 全链路，支持 4TB 数据在 Linux 服务器跑分，结果回传仓库与主站。

**文件改动**：

1. 改 [protocol/lib.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-protocol/src/lib.rs)：
   新增 `BenchmarkReport` / `BenchmarkRun`（runtime 字段：wall_ms、cpu_ms、peak_rss_mb、
   disk_read_bytes、disk_write_bytes、output_bytes、一致性 verdict、加速比）。
2. 新增 `engine/crates/linxira-bio-core/src/benchmark.rs`：
   - `run_benchmark(capability, input, backend: Rust|Python|R)` 编排：
     Rust 直接调 core；Python/R 走 workflow pack（复用 `workflow.rs` 执行器，注入 `/usr/bin/time -v`）。
   - Linux 精度：外部 `/usr/bin/time -v` 采 RSS；Windows fallback 用 `Instant + 自估`（benchmark 主战场在 Linux）。
   - 结果一致性 diff：结构化逐项容差 + 表格 sha256。
3. 新增 benchmark 后端 packs：
   - `workflows/org.linxira.benchmark-python/`（包装原生 Python 实现，自带时间/内存埋点）。
   - `workflows/org.linxira.benchmark-r/`（包装原生 R 实现）。
   - 每个"三份独立实现"能力在阶段 3/4 补齐对应脚本。
4. 改 [cli/main.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-cli/src/main.rs)：
   新增 `benchmark run <capability> <input...> --backends rust,python,r --repeat 3 --output dir`。
5. 改 [apps/linxira-bio-ui/src/main.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/apps/linxira-bio-ui/src/main.rs)：
   结果页新增"Benchmark"按钮：选中输入/输出后并行跑原生后端与 Rust 后端，展示对比表。
6. 新增 `schemas/benchmark-report.schema.json`；新增 `capabilities/catalog.json` 注册 `benchmark.run.v1`。
7. 新增 `scripts/run-benchmark-linux.sh`：Linux 服务器一键跑分（读 4TB 数据路径、写
   `benchmark-results/<date>/`、生成 `summary.json` + `summary.md`）。
8. 主站：`F:\Linxira-OS\Linxira-OS.github.io\public\bio-sdk\` 新增 `benchmark.html`
   （或 `index.html` 加 benchmark 页），渲染 summary.json；列出"核心数据优势点"卡片。
9. 新增 `runtime-preferences.json`（仓库根）+ schema `runtime-preferences.schema.json`：
   - [worker/lib.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-worker/src/lib.rs)
     读取偏好表；能力分发无显式 `--backend` 时走 `default_backend`。
   - [cli/main.rs](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/engine/crates/linxira-bio-cli/src/main.rs)
     与 GUI 增加 `--backend auto|rust|python|r` 覆盖项。
   - `scripts/run-benchmark-linux.sh` 跑完后把最优后端写回 `runtime-preferences.json`。
10. SRR 数据专项测速：`scripts/run-benchmark-linux.sh` 支持 `--sra <...>`：先 `sra.unpack.v1`
    解压 SRR → 生成配对 FASTQ → 送入 FASTQ 能力与 benchmark；`benchmark-results/` 记录
    "解压耗时 + 下游分析耗时"分段指标。

**验证**：小样本冒烟（`tests/fixtures`）验证三后端结果一致且计时字段齐全；
服务器实跑验证 4TB 场景（含 SRR 解压 + 分段计时）；`runtime-preferences.json` 正确写回；
仓库 `benchmark-results/` 与主站页面渲染正确。

---

### 阶段 3 —— 三份独立实现（Python/R 补齐，高优先级能力）

**目标**：为高频、生物信息学 R/Python 生态强项的能力补齐 R/Python 独立实现，实现三端一致。

**策略**：只补齐"生态依赖强"的能力，纯 Rust 已高效且无原生对照的能力不重复造轮子
（遵循 `AGENTS.md`：优先成熟原生工具）。

**首批补齐清单（约 8 个）**：

| 能力 | Python pack | R pack | 说明 |
|---|---|---|---|
| expression.differential.v1 | — | 已有 deseq2 | 已三端（Rust 解析 + R 权威）|
| expression.volcano / enrichment.go | matplotlib 绘图 | ggplot2 绘图 | 绘图对齐（复用阶段1）|
| structure.pdb.summary.v1 | biopython PDB | bio3d | 结构三端 |
| sequence.stats.v1 | biopython | seqinr/Biostrings | 序列基础三端 |
| expression.pca.v1 | scikit-learn | prcomp | 降维三端 |
| enrichment.overrepresentation | gseapy | clusterProfiler | 富集三端 |
| set.venn.v1 | matplotlib-venn | VennDiagram | 集合三端 |
| comparative.dotplot | — | dotplot 原生 | 已有 Rust SVG，补 R |

**新增 packs**：`workflows/org.linxira.*/`（manifest + src + tests + lock + NOTICE），
每个 pack 输出与 Rust **同一 JSON schema**（复用已有 `schemas/*.schema.json`）。

**验证**：CI 增加对应 E2E（仿现有 deseq2/survival E2E 形式）；本地 `validate-repository.py`。

---

### 阶段 4 —— 缺失分析功能补齐（竞品缺口）

**目标**：补齐 2.3 的缺失能力，按优先级分批。

**实现原则**：🔴 优先；Rust 负责编排与结果 JSON，算法委托原生工具或 Python/R
（不重复造成熟算法）。每个新能力同步更新：`catalog.json` + schema + SKILL.md + 文档。

**首批（🔴，本轮重点）**：
- `expression.quantify.v1`（featureCounts/salmon 封装）
- `expression.single-cell.v1`（Seurat/Scanpy 双实现）
- `epigenetics.peak-call.v1`（MACS2 封装）
- `epigenetics.methylation.v1`（Bismark 封装）
- `assembly.denovo.v1`（SPAdes 封装）
- `annotation.gene-quant.v1`（与 quantify 一致的表格输出，复用 output 框架）

**验证**：每能力新增单测 + E2E + SKILL.md validator；CI 覆盖对应原生工具冒烟。

---

### 阶段 5 —— CI/GitHub 增强 + 主站数据优势展示

**目标**：加固桌面系统三平台测试，主站落地 benchmark 结果与优势点。

**文件改动**：

1. 改 [.github/workflows/ci.yml](file:///c:/Users/ETPau/Documents/GITHUB/bio-coding/.github/workflows/ci.yml)：
   - 每个平台增加：绘图 pack（matplotlib/ggplot2）冒烟、`export bio` 往返测试、
     benchmark 小样本（3 后端、断言一致性 + 计时字段非空）。
   - 新增专用 `benchmark` job（ubuntu，跑 `scripts/run-benchmark-linux.sh --smoke`）。
   - GUI 相关：`cargo build --release -p linxira-bio-ui` 三平台构建告警即错（已含）。
2. 新增 `tests/python/test_benchmark_*.py`、`workflows/tests/test_output_packs.py`。
3. 主站 `bio-sdk/benchmark.html` + 组织仓库 README 加 benchmark 结果链接。

---

## 5. 长期计划（Long-term Roadmap）

> 长期计划 = 6 个阶段 + 后续愿景，按里程碑分组。每个里程碑有明确"验收门槛"，
> 通过后才进入下一里程碑（见 §6 阶段性检查标准）。**根目录 `ROADMAP.md` 与本表保持一致。**

| 里程碑 | 覆盖阶段 | 主题 | 关键交付物 | 出口标准（DoD） |
|---|---|---|---|---|
| **M0 地基** | 阶段 0 | 统一输出框架 + 跨平台路径 + 工作区输出 + 格式探测/解压 | `linxira-bio-output` crate、`export bio`、`import probe`、`ROADMAP.md` | JSON↔传统双向 + 路径/解压单测绿；CI 三平台绿 |
| **M1 可视化** | 阶段 1 | 双后端绘图 + 图片查看器 | matplotlib/ggplot2 packs、`PlotSpec`、`ImageGallery`、README 绘图声明 | 三端同 PlotSpec 出图；图片查看器翻页/缩放可手测 |
| **M2 基准** | 阶段 2 | Benchmark 全链路 + 默认后端 + SRR 测速 | `benchmark.run.v1`、`runtime-preferences.json`、`run-benchmark-linux.sh`、主站 benchmark 页 | 4TB/SRR 实跑出 `summary.md`；偏好表写回；数据优势点可展示 |
| **M3 补齐** | 阶段 3 | 三份独立实现（高频能力） | 约 8 个 R/Python packs | 每个 pack 与 Rust 结果容差一致，CI E2E 绿 |
| **M4 补功能** | 阶段 4 | 缺失分析能力（竞品缺口） | `expression.quantify` / `single-cell` / `peak-call` / `methylation` / `denovo` 等 | 每能力 schema+SKILL+文档+E2E 四同步 |
| **M5 发布** | 阶段 5 | CI/GitHub 加固 + 主站数据优势展示 | CI 扩增 job、主站 benchmark.html、组织 README 链接 | 三平台 + benchmark job 绿；主站渲染正确；发 release |
| **M6 愿景** | 后续 | MCP 稳定、Python SDK、HPC/云后端、Java/C++ benchmark 驱动加速 | — | 见 §6 长期治理 + benchmark 每能力沉淀最优后端 |

**长期愿景**：当 benchmark 覆盖足够能力后，`runtime-preferences.json` 成为「每种分析默认用最优语言」
的全平台共识；主站公开 benchmark 数据，让用户「按自己的数据类别选后端」。

## 6. 阶段性检查标准（Phase Gate / Definition of Done）

### 6.1 通用门槛（每个阶段都必须满足）

1. **代码质量**：`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
   `cargo test --workspace` 全绿。
2. **仓库契约**：`python scripts/validate-repository.py` 过（schema / fixture / SKILL 校验三者同步）。
3. **CI 三平台**：windows-gnu / debian-bookworm / arch-linux 全绿（+ 相关 job）。
4. **CLI 冒烟**：`cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json` 正常。
5. **文档三同步**：每新增/改动能力必须同步更新 schema、测试 fixture、SKILL.md（用 skill 校验器）。

### 6.2 阶段特定验收（Phase Gate）

- **阶段 0**：`linxira-bio-output` 单测绿；`export bio` 产出与床工具参照一致；`import probe`
  对 `.sra/.gz/.vcf.gz` 探测与解压正确；`ROADMAP.md` 已在根目录落盘。
- **阶段 1**：matplotlib + ggplot2 两 pack 用同一 PlotSpec 出口一致的 svg/png；GUI 图片查看器
  支持放大/缩小/上一张/下一张；README 含绘图原生后端声明。
- **阶段 2（核心）**：`benchmark.run.v1` 对 `tests/fixtures` 三后端结果一致、计时/RSS 字段非空；
  `run-benchmark-linux.sh` 在服务器对 4TB/SRR 实跑产出 `summary.md`；`runtime-preferences.json`
  正确写回并驱动默认后端；主站 benchmark 页渲染"数据优势点"。
- **阶段 3**：每个新增 R/Python pack 与 Rust 结果在 ±1e-6 相对误差内一致；CI 含对应 E2E。
- **阶段 4**：每个新能力有 schema、SKILL、中英文文档、E2E；原生工具（MACS2/SPAdes/Bismark 等）
  在 `native_tools` 白名单 + `environment` 探测到位。
- **阶段 5**：三平台构建含 `linxira-bio-ui`；`benchmark` job 冒烟绿；主站 + 组织 README 链接齐全；
  打 tag 触发 release。

### 6.3 长期治理标准

- **数据治理**：4TB 原始数据、SRR 归档、受控/研究数据**永不进 Git 仓库**；仅回传
  `benchmark-results/<date>/summary.json + summary.md` 与报告图表。
- **后端共识**：每次 benchmark 跑分后，把「最优后端 + dataset_class」写回 `runtime-preferences.json`，
  形成可追溯、可复现的后端选择依据。
- **原生优先**：可分析性优先用成熟原生工具，Rust 仅在与现有依赖不相上下时才自实现（遵 `AGENTS.md`）。
- **不重复造轮子**：三份独立实现仅覆盖「生态强依赖」能力；纯 Rust 已高效且无原生对照者不补。

---

## 7. Assumptions & Decisions

- **权威结果 = JSON**；传统格式是"可逆投影"，非第二事实源。传统→JSON 的解析器只用于
  benchmark 反向对照与结果回灌，不承担权威存储。
- **Rust 是"编排与契约"层**；Python/R 是"生态算法与出版级绘图"层；Java 仅在有明确
  benchmark 收益时引入（当前阶段 Java = 0，不新增）。
- **绘图默认原生**：Rust SVG 保留为本地零依赖回退，不主动用 Rust 重写 matplotlib/ggplot2
  能力（已写入 README）。
- **Benchmark 主战场 = Linux**（用户 4TB 数据、Linux 服务器）。Windows 可跑但计时/RSS 精度降级。
- **4TB 数据与 benchmark 原始大文件不回传 Git 仓库**（体量过大且属受控/研究数据），
  仅回传 `benchmark-results/<date>/summary.json + summary.md` 与报告图表。
- **三份独立实现 = 结果数值可容差一致**，非逐位一致（浮点、随机种子差异允许容差 ±1e-6 相对误差）。

## 8. Verification（验收总纲）

每阶段至少满足：
1. `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings`
   / `cargo test --workspace` 全绿。
2. `cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json` 正常。
3. `python scripts/validate-repository.py` 过（schema/fixture/SKILL 校验）。
4. 对应阶段的 CI job（win/debian/arch）绿。
5. 阶段 2 额外：`benchmark-results/<date>/summary.md` 与主站 benchmark 页渲染正确，
   产出"核心数据优势点"清单（速度加速比、内存、输出一致性）。