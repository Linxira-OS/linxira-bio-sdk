# Linxira Bio SDK 路线图（ROADMAP）

> **文档定位**：本文件是仓库根目录的**唯一长期计划文档**，具有契约效力——每个任务的
> 「验收标准」均为可直接执行的命令或可核对的产物，通过 Phase Gate（§11）后才可进入下一里程碑。
> 能力级增量记录走 `CAPABILITY_TRACKING.md`（不入 Git）；详细设计蓝图见
> [.trae/documents/unified-output-and-benchmark-blueprint.md](.trae/documents/unified-output-and-benchmark-blueprint.md)。
> **维护规则**：任务完成时在对应任务行标注 `[x]` 并填入完成日期；验收标准变更需同步修改本文件，
> 禁止"口头验收"。

## 1. 术语与缩写

| 术语             | 定义                                                                                                       |
| -------------- | -------------------------------------------------------------------------------------------------------- |
| 三份独立实现         | Rust / Python / R 对同一能力**各自实现算法**，仅共享输入/输出 JSON schema，靠 benchmark 交叉验证数值一致（浮点与随机性容差 ±1e-6 相对误差）         |
| 权威结果           | V2 JSON envelope（`AnalysisResultV2`）；传统格式（FASTA/BED/VCF…）是"可逆投影"，不是第二事实源                                 |
| OutputSpec     | 一次分析的「目标产物清单」声明：角色 → 格式 → 列名/表头 → 路径（§5.1）                                                               |
| PlotSpec       | 统一绘图参数（标题/坐标轴/主题/调色板/尺寸/输出格式），三后端共同消费（§6.1）                                                              |
| 后端 backend     | 某能力的执行实现之一：`rust` / `python` / `r` / 原生命令行封装                                                             |
| dataset\_class | 数据类别标签（`srr`/`annotation`/`matrix`/`structure`/`sequence`/`sample-table`/`other`），用于 benchmark 归类与后端偏好分档 |
| Phase Gate     | 里程碑出口检查清单（§11），全项通过才可推进                                                                                  |
| 四同步            | 新增/改动能力必须同步：schema + 测试 fixture + SKILL.md + 中英文文档（`docs/capabilities/<id>/{en-US,zh-CN}.md`）            |

## 2. 总体目标与不变量

**目标**：把 SDK 从「Rust 单一实现、JSON 单一输出」升级为三语同构平台——统一输出框架、
双后端绘图、benchmark 驱动的默认后端选择、输入自动探测解压，补齐 §10 约 30 项能力缺口，
并实现「装完即用」（安装器 + PATH 注册）与「AI 可操控」（CLI → Skills → MCP 三层 agent 通路）。

**全周期不变量（任何里程碑不得违反）**：

1. 权威结果 = JSON；传统格式写出器必须可从 JSON 完整重建，且反向解析器能还原等价 JSON。
2. 4TB 原始数据、SRR 归档、受控/研究数据**永不进 Git 仓库**；仅回传
   `benchmark-results/<date>/summary.json + summary.md` 与图表。来源引用见
   [docs/BENCHMARK\_DATA\_SOURCES.md](docs/BENCHMARK_DATA_SOURCES.md)。
3. 原生优先（遵 `AGENTS.md`）：成熟原生工具能做的算法不自研；Rust 承担编排、契约、IO 加速。
4. 三份独立实现仅覆盖「生态强依赖」能力；纯 Rust 已高效且无原生对照者不补。
5. Java 当前为 0 且不新增，除非 benchmark 证明明确收益（C++ 同理，遵 `AGENTS.md` 的 benchmark 前置门槛）。
6. 每个里程碑出口必须满足 §12 通用门槛（fmt/clippy/test 三绿 + `validate-repository.py` + CI 三平台 + CLI 冒烟）。
7. **CLI 是第一公民（agent-first）**：所有新命令必须满足 §10.1 CLI 契约（退出码表、`--json` 纯净输出、stdout/stderr 分离、零交互），先于 GUI 落地；AI 通过 CLI/Skills/MCP 操控软件与用户点 GUI 是同等一等入口。
8. **并发分层选型**（新，见 §9）：异步 IO 编排用 Tokio、多核计算用 Rayon、压缩后台管线用 thread\_io、生信格式读写在确认收益前保持自有解析器；**Rust side 现有解析器同步单线程，正是 M5 要解决的差距**。

## 3. 里程碑依赖图与推进顺序

```text
M0 统一输出框架 ──┬──> M1 双后端绘图（依赖 OutputSpec/PlotSpec 类型）
                 ├──> M2 Benchmark（依赖 provenance 计时字段 + format_probe 解压）
                 │        └──> M7 发布（依赖 M2 的 summary 契约 + M6 的安装器）
                 └──> M3 三份独立实现（依赖 M1 绘图 packs、M2 benchmark 对照）
                          └──> M4 能力补齐（依赖 M0 输出框架 + M3 后端模式）
                                   ├──> M5 并发/IO 引擎选型（依赖 M2 benchmark 基准，为测速定引擎）
                                   └──> M6 CLI 契约 + 安装器 + Agent 通路
                                            └──> M7 发布 ──> M8 愿景（benchmark 数据积累驱动）
```

- 严格顺序：**M0 → M2 → M1 → M3 → M4 → M7**（benchmark 是核心重点，M0 完成后优先 M2；
  M1 可与 M2 并行开发，但其验收依赖 M0 的 PlotSpec 类型）。

- **例外 1**：M6-T1/T2（CLI 契约、doctor）可与 M2 并行先行——每个新命令
  （`benchmark run` / `export bio` / `import probe`）落地时即须遵守 §10.1 契约，避免事后返工；
  安装器与 MCP 发布（M6-T3\~T9）在 M4 后统一落地。

- **例外 2**：**M5（并发/IO 引擎）与 M6 可并行**——它依赖 M2 的 benchmark 基准，是能力侧改造，
  不阻塞安装器/MCP（M6）。建议**先落地 M5-T1**（Tokio 异步运行时 + FASTQ 并发读取立基准），
  否则 M2 的 benchmark 测的就是"单线程同步版"，测速结论会失真。

- **例外 3**：M5 的 Tokio 接入优先落在 M0 的 `format_probe`/解压与 M2 的 benchmark 计时路径
  （IO 密集为主）；不要先改纯算法的 CPU 密集函数（那是 Rayon 的活，且影响 DETERMINISM，见 §9.4）。

- 每个里程碑内部任务按 T 编号串行验收；同一里程碑内无依赖的任务可并行。

## 4. 现状基线（2026-08 盘点，作为对照起点）

| 维度                                   | 基线值                                                                                                                                                                                                                                                                                                             | 证据位置                                                                                                                                                                                       |
| ------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Rust 能力                              | ≈85 个，全部输出 V2 JSON                                                                                                                                                                                                                                                                                              | [capabilities/catalog.json](capabilities/catalog.json)（available ≈90，planned 2）                                                                                                            |
| Python 能力                            | 2（biopython 转换、rdkit 描述符）                                                                                                                                                                                                                                                                                       | `workflows/org.linxira.sequence-conversion-biopython/`、`workflows/org.linxira.chemistry-descriptors-rdkit/`                                                                                |
| R 能力                                 | 3（deseq2、wgcna、survival）                                                                                                                                                                                                                                                                                        | `workflows/org.linxira.bulk-expression-deseq2/` 等                                                                                                                                          |
| 导出格式                                 | 仅 Csv/Tsv/Json/Jsonl/Xlsx 5 种                                                                                                                                                                                                                                                                                   | [export/lib.rs](engine/crates/linxira-bio-export/src/lib.rs) `ExportFormat`                                                                                                                |
| 绘图                                   | 6 个 Rust SVG（volcano/motif/synteny/annotation/domain/enrichment）+ GUI 表格内嵌预览                                                                                                                                                                                                                                    | [scientific\_visualization.rs](engine/crates/linxira-bio-core/src/scientific_visualization.rs)                                                                                             |
| workflow runtime                     | 仅 `WorkflowRuntimeKind::{R, Python}`                                                                                                                                                                                                                                                                            | [protocol/lib.rs](engine/crates/linxira-bio-protocol/src/lib.rs) L421                                                                                                                      |
| provenance 计时                        | `ProvenanceV2` 已有 `started_at/finished_at` 字段但**从未填充**                                                                                                                                                                                                                                                          | [protocol/lib.rs](engine/crates/linxira-bio-protocol/src/lib.rs) L313                                                                                                                      |
| 输出目录解析                               | 仅支持显式 `parameters.output_directory`，无分类/时间戳逻辑                                                                                                                                                                                                                                                                   | [workflow.rs](engine/crates/linxira-bio-worker/src/workflow.rs) `resolve_output_directory` L1339                                                                                           |
| CLI                                  | 手写 argv match dispatch，`--json` 开关，无 export-bio/import-probe/benchmark                                                                                                                                                                                                                                          | [main.rs](engine/crates/linxira-bio-cli/src/main.rs) L243 起                                                                                                                                |
| benchmark                            | 无任何能力/命令/schema                                                                                                                                                                                                                                                                                                 | —                                                                                                                                                                                          |
| MCP server                           | `linxira-bio-mcp` crate **已实现并进发布包**（2026-09-19）：stdio JSON-RPC（initialize/tools/list/tools/call→worker 执行/resources list+read，protocolVersion 2024-11-05）；bundle-manifest 三平台 binaries 已含 mcp，README 状态已更新                                                                                                                           | [mcp/main.rs](engine/crates/linxira-bio-mcp/src/main.rs)；[bundle-manifest.json](packaging/bundle-manifest.json) `binaries` 仅 CLI/worker/UI 三个                                              |
| 安装/PATH 注册                           | 无安装器；无任何平台做 PATH 注册；`scripts/smoke-mcp-server.py` 已有但未进 CI 必跑链                                                                                                                                                                                                                                                  | [stage-release.py](scripts/stage-release.py)（仅打包文件树，不产安装器）                                                                                                                                 |
| CLI agent 契约                         | 退出码分类、错误结构（`action_hint`）、stdout/stderr 分离规则**未成文**                                                                                                                                                                                                                                                             | —                                                                                                                                                                                          |
| 桌面 UI（egui/eframe 0.35，**非 Dioxus**） | 刷新纪律**已正确**：仅在任务运行中 `request_repaint_after(100ms)`，空闲时不请求重绘（egui 休眠至输入事件）；耗时任务（分析/检查/环境审计）已走后台线程 + channel 回传，UI 线程不阻塞；`structure_viewer` 的 5ms sleep 仅存在于测试代码。**待改进**：仅日志查看器用了 `show_rows` 虚拟化（[main.rs:6333](apps/linxira-bio-ui/src/main.rs#L6333)），数据集列表等 6 处 `ScrollArea` 非虚拟化（大量导入时每帧全量布局）；空闲 CPU≈0 无守护测试 | [main.rs:3807-3832](apps/linxira-bio-ui/src/main.rs#L3807-L3832)（条件重绘）、[main.rs:794](apps/linxira-bio-ui/src/main.rs#L794)（后台线程）、[Cargo.toml](apps/linxira-bio-ui/Cargo.toml)（eframe 0.35） |
| CLI/桌面安装分体                           | 无：bundle-manifest 把 CLI/worker/UI 打成一个包，Linux 服务器装桌面包会拖入 X11/Wayland/wgpu 依赖                                                                                                                                                                                                                                    | [bundle-manifest.json](packaging/bundle-manifest.json)（binaries 无 component 字段）                                                                                                            |
| 并发/异步 IO                             | 全部 crate 用**同步单线程**主循环；TOML 依赖里**无 tokio / rayon / thread\_io**（`grep bio` 为误报，worker 依赖未含这些库）；FASTQ/BAM 等解析器均为自研同步实现                                                                                                                                                                                           | [worker Cargo.toml](engine/crates/linxira-bio-worker/Cargo.toml)、[cli Cargo.toml](engine/crates/linxira-bio-cli/Cargo.toml)（仅 serde/sha2/tempfile），核对后无并发栈                                 |

## 5. M0 —— 统一输出框架 + 跨平台路径 + 工作区输出 + 格式探测

**目标**：JSON↔传统格式双向打通、规范输出路径、输入自动探测与解压。无新分析功能。

### 5.1 新 crate `engine/crates/linxira-bio-output/`

| 任务    | 内容                                                                                                                      | 验收                                        |
| ----- | ----------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| M0-T1 | `src/lib.rs` 定义 `OutputSpec`（见下方字段表）、`BioDataWriter`、`BioDataReader`、`PlotSpec`（类型先落，渲染在 M1）                            | `cargo test -p linxira-bio-output` 编译+单测绿 |
| M0-T2 | 传统写出器 v1：**Fasta/Fastq/Bed/Gff3/Gtf/Vcf/Sam** 7 类（对齐目标见 §5.2）；Csv/Tsv/Xlsx/Json/Jsonl/Parquet 直接转发 `linxira-bio-export` | 每类至少 2 个 fixture 往返单测（写→读→JSON 等价）        |
| M0-T3 | 所有写出器走 `linxira_bio_export::write_atomic_bytes`（临时文件 + persist），禁止非原子写                                                  | 代码审查 + 单测断言产物路径直接出现完整文件                   |
| M0-T4 | `BioDataReader`：上述 7 类 → JSON（用于 benchmark 反向对照与结果回灌）                                                                   | 与 M0-T2 同一组 fixture 反向断言                  |

**`OutputSpec`** **字段定义**（同时落 `schemas/output-spec.schema.json`）：

| 字段                             | 类型            | 必填 | 说明                                                                     |
| ------------------------------ | ------------- | -- | ---------------------------------------------------------------------- |
| `schema_version`               | integer `"1"` | ✅  | 整数版本号（教训：禁止 `"1.0"` 字符串，Windows GNU/Arch CI 曾因此失败）                     |
| `capability`                   | string        | ✅  | 如 `expression.differential.v1`                                         |
| `artifacts`                    | array         | ✅  | 产物清单，见下                                                                |
| `artifacts[].role`             | string        | ✅  | 产物角色，如 `sequences`/`variants`/`table`/`figure`                         |
| `artifacts[].format`           | enum          | ✅  | 复用 `BioDataFormat` 的取值（fasta/fastq/bed/gff3/gtf/vcf/sam/csv/tsv/svg/…） |
| `artifacts[].path`             | string        | ✅  | 相对工作区输出目录的路径                                                           |
| `artifacts[].columns`/`header` | array         | ⛔  | 列名/表头覆盖；缺省时由能力默认（如 VCF 固定 8+ 列）                                        |
| `artifacts[].coords`           | enum          | ⛔  | 坐标系覆盖：`bed`（0-based half-open）/`gff`（1-based inclusive）/`vcf`（1-based） |

**坐标约定（写出器实现必须遵守，属"逐字节对齐"的一部分）**：
BED 0-based 半开区间；GFF3/GTF/VCF 1-based 闭区间；JSON 内部统一 1-based 闭区间（与现有能力一致），
写出时按 `coords` 转换并在单测中断言边界值（start=0/1 用例必测）。

### 5.2 传统格式对齐目标（逐字节验收基准）

| 格式          | 对齐参照                                                           | 验收基准文件                                  |
| ----------- | -------------------------------------------------------------- | --------------------------------------- |
| Fasta/Fastq | seqkit `fx2tab` 往返、FastQ 标准 4 行结构                              | `tests/fixtures/output/fasta/`、`fastq/` |
| Bed         | bedtools 输出规范（chrom..strand，tab 分隔，无尾随 tab）                    | `tests/fixtures/output/bed/`            |
| Gff3/Gtf    | gffread 校验通过 + 九列结构 + `###` 分隔                                 | `tests/fixtures/output/gff3/`           |
| Vcf         | `bcftools view` 无告警读取；固定头 `##fileformat=VCFv4.2` + `#CHROM…` 行 | `tests/fixtures/output/vcf/`            |
| Sam         | `samtools view -H` 解析无错；QNAME..QUAL 11 列 + 可选列                 | `tests/fixtures/output/sam/`            |

### 5.3 `src/path_resolver.rs`

| 任务    | 函数                                                                               | 行为规格                                                                                                                                 | 验收                                                                 |
| ----- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------ |
| M0-T5 | `resolve_input_path(raw: &str) -> Result<PathBuf>`                               | 剥离 `file://` 前缀；展开 `~`（Windows 下同时认 `~` 与 `%USERPROFILE%` 字面量）；统一正反斜杠；拒绝 NUL 与非法 UTF-8                                               | 单测覆盖：Windows 盘符、`\\?\` 前缀、POSIX 绝对/相对、含空格/中文名、`~/x`、`file:///c:/x` |
| M0-T6 | `workspace_root(cli_flag: Option<&Path>, gui_project: Option<&Path>) -> PathBuf` | CLI 用 `--workspace`（缺省=当前目录）；GUI 用项目文件目录                                                                                             | 单测 + CLI 手测                                                        |
| M0-T7 | `classify_output_dir(capability: &str) -> PathBuf`                               | `analysis/<domain>/<capability>/`，domain 取能力 id 首段（`expression.differential.v1` → `analysis/expression/expression.differential.v1/`） | 单测至少 5 个不同 domain                                                  |
| M0-T8 | `timestamp_suffix(existing: &Path) -> PathBuf`                                   | `_YYYYMMDD-HHMMSS` 后缀；同秒再撞追加 `_2`、`_3`…（区分同一批数据多次输出）                                                                                 | 单测：模拟同秒两次输出得到不同路径                                                  |

### 5.4 `src/format_probe.rs`

| 任务     | 内容                                                                                                                                                                                                                                                                                                                     | 验收                                                                                                  |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| M0-T9  | `probe_format(path) -> ProbeResult{format: BioDataFormat, compression: CompressionFormat, confidence: magic\|extension\|both}`。魔数表：gzip `1f 8b`、bzip2 `BZh`、xz `fd 37 7a 58 5a 00`、zstd `28 b5 2f fd`、zip `PK\x03\x04`、**7z** **`37 7a bc af 27 1c`**、BAM `BAM\x01`、BGZF gzip+BC extra、SRA `.sra` 头、SAM `@HD`/`@SQ` 首行 | 对 `tests/fixtures/output/probe/` 全部样本探测正确（含伪装扩展名样本：`.fa` 实为 gzip → 报 gzip+fasta，`.gz` 实为 7z → 报 7z） |
| M0-T10 | `ensure_decompressed(path, workspace_tmp) -> PathBuf`：gzip/bzip2/xz/zstd/zip/**7z** 流式解压到工作区临时目录（复用 `CompressionFormat` 枚举）；文件 >100MB 时流式、不整读内存。**7z 优先走原生** **`7z`/`7zz`（多线程）**，缺失时回退自研                                                                                                                               | 单测（小样本）+ 大文件流式（内存峰值断言，用 `#[ignore]` 标注的大样本测试）                                                       |
| M0-T11 | SRA：探测 `fastq-dump`/`fasterq-dump`/`prefetch`/`vdb-config` 是否在 PATH（纳入 `native_tools` 白名单 + `environment` 能力探测项）；在则走 `fasterq-dump --split-3`，不在则报可行动错误（提示安装 sra-tools）                                                                                                                                                | 环境探测单测（mock PATH）；有 sra-tools 的 CI 上 E2E                                                            |
| M0-T12 | CLI：`import probe <path>`（输出格式/压缩/置信度/建议解压路径 `--json` 可选）；`export bio <input.json> <output> --spec <spec.json>`                                                                                                                                                                                                        | 见验收命令 §11.2                                                                                         |

### 5.5 对既有代码的修改

| 任务     | 文件                                                                                             | 改动                                                                                            | 验收                        |
| ------ | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ------------------------- |
| M0-T13 | [workflow.rs](engine/crates/linxira-bio-worker/src/workflow.rs) `resolve_output_directory`     | 接入 T7/T8：无显式 `output_directory` 时走 `classify_output_dir` + `timestamp_suffix`（显式指定时行为不变，向后兼容） | 现有 worker 测试全绿 + 新增默认路径单测 |
| M0-T14 | [workflow.rs](engine/crates/linxira-bio-worker/src/workflow.rs)                                | 填充 `ProvenanceV2.started_at/finished_at`（RFC3339 UTC）；`output_directory` 记录进 provenance       | 单测断言两个字段非空且合法             |
| M0-T15 | [Cargo.toml](Cargo.toml) workspace members + `deny.toml` 若新增依赖须过 license 审计                    | `cargo build --workspace` 绿；CI license-audit job 绿                                            | <br />                    |
| M0-T16 | `schemas/output-spec.schema.json` + `schemas/plot-spec.schema.json` + `tests/fixtures/output/` | `python scripts/validate-repository.py` 过；schema\_version 用整数 `"1"`                           | validate 脚本绿              |

## 6. M1 —— 双后端绘图 + 图片查看器

**目标**：图表能力同时产出原生 matplotlib/ggplot2 图；GUI 内置图片查看器。

### 6.1 PlotSpec 字段（落 `schemas/plot-spec.schema.json`，类型定义在 `linxira-bio-output`）

| 字段                  | 类型                                            | 缺省                    | 说明                          |
| ------------------- | --------------------------------------------- | --------------------- | --------------------------- |
| `title`/`subtitle`  | string                                        | 空                     | 标题                          |
| `x_label`/`y_label` | string                                        | 按 `input_filename` 推导 | 用户不设置时由导入文件名生成（用户需求）        |
| `x_range`/`y_range` | \[number, number]                             | 自动                    | 坐标范围                        |
| `x_log`/`y_log`     | boolean                                       | false                 | 对数轴                         |
| `grid`              | boolean                                       | true                  | 网格                          |
| `theme`             | enum `dark\|light\|publication`               | light                 | 主题                          |
| `palette`           | string                                        | `set2`                | 调色板名（三端各自映射到原生色系）           |
| `legend`            | boolean                                       | true                  | 图例                          |
| `figure`            | {width, height, dpi}                          | {800, 600, 150}       | 尺寸                          |
| `font`              | {family, size}                                | {空=平台默认, 12}          | 字体（GUI 已带 NotoSansSC，中文名可用） |
| `output`            | {format: `svg\|png\|pdf`, interactive: false} | {svg, false}          | 输出                          |
| `data`              | object                                        | ✅                     | 图数据（点列/矩阵），由能力组装，三端只读       |

### 6.2 任务清单

| 任务    | 内容                                                                                                                                                                      | 验收                                                           |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| M1-T1 | 新增 `workflows/org.linxira.visualization-matplotlib/`（manifest + `src/render.py` + `requirements.lock` + tests）：读 PlotSpec JSON → matplotlib 渲染 svg/png/pdf              | pack 冒烟测试绿（CI 三平台）；同一 PlotSpec 三次运行输出 sha256 稳定（固定 dpi/无时间戳） |
| M1-T2 | 新增 `workflows/org.linxira.visualization-ggplot2/`（`src/render.R` + `dependencies.lock.json` + tests）：同一 PlotSpec → ggplot2                                              | 同上；svg 输出与 matplotlib 版**数据一致**（图内点数/坐标范围断言，不做像素级对比）         |
| M1-T3 | 改 [scientific\_visualization.rs](engine/crates/linxira-bio-core/src/scientific_visualization.rs)：6 个 `render_*_svg_path` 函数新增接受 `PlotSpec` 的入口（旧签名保留转发默认 PlotSpec，向后兼容） | 现有测试不改动即绿 + 新增 PlotSpec 覆盖单测                                 |
| M1-T4 | 改 [apps/linxira-bio-ui](apps/linxira-bio-ui/src/main.rs)：新增 `ImageGallery`（滚轮/按钮缩放、上一张/下一张、适配窗口、PNG/SVG 微浏览）                                                            | GUI 手测清单（缩放/翻页/SVG 渲染）逐项确认；键盘 ←/→ 翻页                         |
| M1-T5 | 图表结果页：内嵌预览 + 「打开图片查看器」按钮                                                                                                                                                | GUI 手测                                                       |
| M1-T6 | 改 [README.md](README.md)：加入声明「绘图默认使用原生 Python(matplotlib)/R(ggplot2) 后端；Rust SVG 作为零依赖回退保留，待引入成熟 Rust 绘图包后再补齐 Rust 后端」                                                  | 文案存在且与实际行为一致                                                 |
| M1-T7 | `schemas/plot-render-result.schema.json`（含产物路径、后端、PlotSpec sha256 用于溯源）                                                                                                 | validate 脚本绿                                                 |

**后端分派规则**：`output.format=png/pdf` → 优先 matplotlib pack；`svg` → 三端皆可，按 `runtime-preferences.json`（M2）或回退 Rust SVG（M2 前的缺省）。`--backend` 覆盖项在 M2-T7 落地。

## 7. M2 —— Benchmark 框架（核心重点）

**目标**：同一输入，三后端同机跑分，产出可回传仓库与主站的量化报告，并驱动默认后端选择。

### 7.1 方法学（严谨性要求，实现必须遵守）

| 规则         | 规格                                                                                                                                                                                 |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 重复次数       | 每后端 ≥3 次（`--repeat` 可调），报告**中位数** wall-time 与峰值 RSS，附 min/max/±IQR                                                                                                                 |
| 预热         | 每后端先 1 次预热运行（不计入统计），排除解释器/JIT/页缓存首次加载                                                                                                                                              |
| 冷/热缓存标注    | 报告标注 `page_cache: warm`（预热后的同机重复）；4TB 大文件跑分时允许 warm-cache 口径，但**必须在 summary.md 披露口径**，不得与 cold-cache 数字混排                                                                          |
| 计时         | Linux：外部 `/usr/bin/time -v` 包裹（采 wall/CPU/峰值 RSS `Maximum resident set size`）；Rust 进程内用 `std::time::Instant` 交叉校验（偏差 >5% 时在报告标注）。Windows：`Instant` + 自估降级，标注 `precision: degraded` |
| 磁盘 I/O     | 优先读 `/proc/<pid>/io` 的 `read_bytes/write_bytes`；不可得时标 `null`，禁止伪造                                                                                                                  |
| 环境 pinning | 报告必须记录：OS/kernel、CPU 型号、内存总量、Rust/Python/R 版本、关键依赖版本（如 DESeq2 版本）、是否容器内                                                                                                            |
| 一致性判据      | 结构化字段逐项 diff：数值相对误差 ≤1e-6 判 `consistent`；字符串全等；表格类 sha256 相等（排序规范化后）。任一超差 → `inconsistent` 并记录差异字段清单                                                                               |
| 加速比定义      | `speedup = median_wall(native) / median_wall(rust)`；内存节省 `= 1 - peak_rss(rust)/peak_rss(native)`。原生基准 = 该能力生态对标工具（如 DESeq2），不是自写脚本                                                 |
| 失败隔离       | 单个能力 benchmark 失败不阻塞整批；`summary.json` 记 `status: failed` + stderr 摘要（截断至 4KB）                                                                                                      |

### 7.2 任务清单

| 任务     | 内容                                                                                                                                                                                                                                                                                                                                                                                                | 验收                                                                         |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| M2-T1  | [protocol/lib.rs](engine/crates/linxira-bio-protocol/src/lib.rs) 新增 `BenchmarkRun`（backend、wall\_ms、cpu\_ms、peak\_rss\_mb、disk\_read\_bytes、disk\_write\_bytes、output\_bytes、consistency、repeats、env 快照）与 `BenchmarkReport`（capability、dataset\_class、runs\[]、speedup、memory\_saving、verdict）                                                                                                     | 类型单测 + `schemas/benchmark-report.schema.json`                              |
| M2-T2  | 新增 `engine/crates/linxira-bio-core/src/benchmark.rs`：`run_benchmark(capability, inputs, backends, repeat)` 编排；Rust 直调 core；Python/R 走 workflow pack（复用 [workflow.rs](engine/crates/linxira-bio-worker/src/workflow.rs) 执行器，注入 `/usr/bin/time -v`）；一致性 diff 引擎                                                                                                                                     | 小样本（`tests/fixtures`）三后端一致 + 计时/RSS 字段非空                                   |
| M2-T3  | 新增 `workflows/org.linxira.benchmark-python/`、`workflows/org.linxira.benchmark-r/`：包装原生实现（对标工具直跑，如 DESeq2 直跑 vs Rust 编排跑），脚本头尾埋点（Python `resource` / R `/proc/self/status` 峰值）                                                                                                                                                                                                                     | pack 各自冒烟测试                                                                |
| M2-T4  | CLI：`benchmark run <capability> <input...> --backends rust,python,r --repeat 3 --output <dir> --dataset-class <class>`                                                                                                                                                                                                                                                                            | 冒烟命令见 §12.2；遵守 §10.1 契约                                                    |
| M2-T5  | GUI：结果页新增「Benchmark」按钮——选中输入/输出后并行跑原生后端与 Rust 后端，展示对比表（耗时/内存/加速比/一致性）                                                                                                                                                                                                                                                                                                                             | GUI 手测                                                                     |
| M2-T6  | `scripts/run-benchmark-linux.sh`：读数据清单（路径 + dataset\_class 映射表 `benchmark-datasets.json`，本地不入库），写 `benchmark-results/<date>/summary.json` + `summary.md`（对比表 + 数据优势点清单 + 环境披露 + §7.1 口径披露）                                                                                                                                                                                                        | 服务器实跑产出完整 summary；对 4TB 数据的每类至少 1 个能力有数字                                   |
| M2-T7  | `runtime-preferences.json`（仓库根）+ `schemas/runtime-preferences.schema.json`：`{schema_version:1, preferences:[{capability, dataset_class, default_backend, measured:{rust/python/r 各 {wall_ms, peak_rss_mb}}, sampled_on, speedup, consistency}]}`；[worker](engine/crates/linxira-bio-worker/src/lib.rs) 读取偏好表，无显式 `--backend` 时走 `default_backend`；CLI/GUI 增加 `--backend auto\|rust\|python\|r` 覆盖 | 单测：偏好表命中/缺失/auto 回退路径；`run-benchmark-linux.sh` 跑完把最优后端（一致且 wall-time 最快）写回 |
| M2-T8  | SRR 专项：`run-benchmark-linux.sh --sra <sra-list>`：先解压（M0-T11）→ 配对 FASTQ → 送 FASTQ 能力；`summary.json` 记录「解压耗时 + 下游分析耗时」**分段指标**                                                                                                                                                                                                                                                                      | 至少 1 个 SRR 样本分段数字齐全                                                        |
| M2-T9  | 主站 `F:\Linxira-OS\Linxira-OS.github.io\public\bio-sdk\benchmark.html`：渲染 summary.json，卡片列出数据优势点（加速比/内存/一致性），每张卡片附数据来源链接（取自 [docs/BENCHMARK\_DATA\_SOURCES.md](docs/BENCHMARK_DATA_SOURCES.md) §5）                                                                                                                                                                                                 | 页面手测：数字与 summary.json 一致、来源链接可点                                            |
| M2-T10 | `capabilities/catalog.json` 注册 `benchmark.run.v1`（available）；SKILL.md `skills/run-bio-benchmark/` + 四同步                                                                                                                                                                                                                                                                                           | validate 脚本绿                                                               |

## 8. M3 / M4 —— 三份独立实现 + 能力补齐

### 8.1 M3：三份独立实现（首批 8 项）

每个 pack 交付物固定五件套：`workflows/org.linxira.<pack-id>/`（manifest + src + tests + lock + NOTICE），
输出复用既有 `schemas/*.schema.json`，**不新建 schema**（三端同一 schema 是对齐锚点）。

| # | 能力                                    | Python pack          | R pack            | 对标基准工具 | 验收补充                       |
| - | ------------------------------------- | -------------------- | ----------------- | ------ | -------------------------- |
| 1 | expression.differential.v1            | —                    | deseq2（已有）        | DESeq2 | Rust vs R 数值 ±1e-6         |
| 2 | expression.volcano / enrichment.go 绘图 | matplotlib（复用 M1-T1） | ggplot2（复用 M1-T2） | —      | 同 PlotSpec 三端图数据一致         |
| 3 | structure.pdb.summary.v1              | biopython            | bio3d             | —      | 三端汇总字段一致                   |
| 4 | sequence.stats.v1                     | biopython            | Biostrings        | seqkit | 长度/GC/计数一致                 |
| 5 | expression.pca.v1                     | scikit-learn         | prcomp            | —      | 奇异值符号翻转需规范化（取绝对值最大分量正号）后对比 |
| 6 | enrichment.overrepresentation         | gseapy               | clusterProfiler   | —      | p 值容差 ±1e-6                |
| 7 | set.venn.v1                           | matplotlib-venn      | VennDiagram       | —      | 集合基数一致                     |
| 8 | comparative.dotplot                   | —                    | 原生 R              | —      | —                          |

CI：每个 pack 增加对应 E2E job（仿现有 deseq2/survival E2E 形式，跑 `tests/fixtures` 小样本）。

### 8.2 M4：能力补齐（首批 🔴 6 项）

实现模式统一：**Rust 编排 + 结果 JSON**，算法委托原生工具（进 `native_tools` 白名单 + `environment` 探测）；
每个能力四同步 + `benchmark-datasets.json` 登记其 dataset\_class 映射（供 M2 跑分）。

| # | 能力 id                      | CLI 子命令                   | 原生工具                       | 输入                      | 输出（OutputSpec 角色）                                 | 绘图        |
| - | -------------------------- | ------------------------- | -------------------------- | ----------------------- | ------------------------------------------------- | --------- |
| 1 | expression.quantify.v1     | `expression quantify`     | featureCounts/salmon       | SRR 解压后 FASTQ/BAM + GTF | `counts`(tsv) + `summary`(json)                   | —         |
| 2 | expression.single-cell.v1  | `expression single-cell`  | Scanpy(Py) + Seurat(R) 双实现 | H5ad 矩阵                 | `clusters`(tsv) + `markers`(tsv) + `umap`(figure) | ✅ UMAP/散点 |
| 3 | epigenetics.peak-call.v1   | `epigenetics peak-call`   | MACS2                      | BAM + 对照                | `peaks`(bed) + `summary`(json)                    | ✅ 峰型图     |
| 4 | epigenetics.methylation.v1 | `epigenetics methylation` | Bismark + methylKit(R)     | BS-seq FASTQ + 参考       | `calls`(tsv) + `dmr`(bed)                         | ✅ DMR 图   |
| 5 | assembly.denovo.v1         | `assembly denovo`         | SPAdes                     | FASTQ                   | `contigs`(fasta) + `stats`(json)                  | —         |
| 6 | annotation.gene-quant.v1   | `annotation gene-quant`   | 复用 M0 输出框架                 | 任意 counts               | `table`(tsv/csv/xlsx 可选)                          | —         |

二批（🟠 10 项）与三批（🟡 12 项）见 §10 总清单；每项落地前须先补一行到 §10 表格并标注排期里程碑，禁止"清单外"能力先行动工。

## 9. M5 —— Rust 并发/IO 引擎选型与落地（Tokio + Rayon + thread\_io）

**背景**：Python/R 的瓶颈不在"C 启动器"，而在控制流最终回到 GIL / 单线程解释器。Rust 的正确姿势是**分层选型而非一个运行时打天下**。**——但核查发现当前仓库恰恰没做到，性能优势和 Python/R 确实拉不开。见下。**

### 9.0 性能差距核查结论（2026-08，硬证据）🔴 严重

对 `engine/` 全量核查得出以下事实（非假设）：

| #  | 核查事实                                                                                                                   | 证据                                                                                                              | 影响                                                                |
| -- | ---------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| P1 | **全 Rust 后端零并发**：全 `engine/` 无 `rayon`/`tokio`/`par_iter`/`thread::spawn`/`spawn_blocking`                             | grep 全仓 0 命中；[core Cargo.toml](engine/crates/linxira-bio-core/Cargo.toml) 依赖仅 csv/flate2/rand/serde/serde\_json | 所有 CPU 密集（kmer/比对/qc）**吃不满多核**                                    |
| P2 | **gz 解压同步 + 不重叠**：所有 `.gz` 走 `flate2::read::MultiGzDecoder::new(File)`，在**同一调用线程同步解压**，解压完成才进入主计算，解压(CPU 密集)与计算**零重叠** | [fastq.rs:254](engine/crates/linxira-bio-core/src/fastq.rs) 等 10+ 处                                             | **4TB gz 数据的最大吞吐瓶颈**；而 Python 的 `gzip` 也是单线程——这一项直接和 Python 拉不开差距 |
| P3 | **无 SIMD**、无显式向量化加速热算法                                                                                                 | 依赖清单无 `libdeflater`/SIMD 栈                                                                                      | 编辑距离/比对热核是标量循环                                                    |
| P4 | 少量 `read_to_end` 全量加载（mmCIF/domain/functional）                                                                         | [coordinate.rs:321](engine/crates/linxira-bio-core/src/coordinate.rs) 等                                         | 大文件内存峰值风险（多数文件不大，需按数据类别复核）                                        |
| P5 | 流式 IO 基础良好：统一 `BufReader`+`MultiGzDecoder`，无整文件读入热路径                                                                   | [fastq.rs](engine/crates/linxira-bio-core/src/fastq.rs) 等                                                       | 这是唯一没有被拖后腿的底子，M5 在此之上叠加即可                                         |

**结论**：当前 Rust 的唯一优势是 AOT + 无 GIL + 流式 IO 写得规范；但**吞吐吃不满多核、解压不重叠**，
在 4TB gz 数据上与原生 Python/R 无法拉开差距——**M5 是整个 benchmark 卖点的前置关键**，不是锦上添花。

**修复优先级**（决定 M5 任务顺序）：

1. **P2 解压 + P1 多核** → 并行解压引擎（BGZF/libdeflater）+ Rayon，最高优先；
2. P2 解压与计算重叠 → thread\_io 流水线化；
3. P3 SIMD（libdeflater）随并行解压一起上；
4. P4 read\_to\_end 按数据类别复核改流式。

### 9.1 分层技术选型（已定的默认栈，非待议）

| 层           | 技术                                                                                                                                 | 定位                                 | 直击缺陷                        |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | --------------------------- |
| 并行解压引擎      | **BGZF→`noodles-bgzf::MultithreadedReader`**；普通 gzip→**`libdeflater`**(SIMD zlib 兼容) 或 `seq_io::parallel` 分块并行；原生 `flate2` 保留为串行基线 | gz 解压这个最大瓶颈吃满多核                    | P1/P2/P3                    |
| 压缩后台线蔚      | thread\_io                                                                                                                         | 解压放后台线程 + channel 与主计算流水线化，解压与计算重叠 | P2                          |
| 多核计算        | Rayon（独立全局线程池）                                                                                                                     | FASTAQ 解析、k-mer、比对、SIMD 等 CPU 密集   | P1                          |
| 异步运行时       | Tokio（`rt-multi-thread`，默认按 CPU 核起 worker）                                                                                         | 海量文件/网络/进程 IO 的并发编排                | P1（IO 并发）                   |
| 生信格式 IO     | **自有解析器（现状保留）**；仅在确认收益后评估外部库                                                                                                       | BAM/CRAM/VCF/FASTA/FASTQ           | 见 §9.3"外部生信库评估门"            |
| 极致文件 IO（可选） | Glommio（Linux + io\_uring，内核 5.8+）                                                                                                 | 百万级小文件极限                           | 仅在 §7.2 `benchmark` 证明收益后进入 |

> **为什么不先引入 noodles/bio 的全部异步 IO**：仓库自研解析器已过所有 E2E，外部库在跨平台
> (Windows GNU)、license(BIOSL)、不确定的 async feature 兼容上风险高。**但并行解压（BGZF/libdeflater）
> 这一项收益明确、改动面小，例外地优先做**（见 9.4 M5-T1）。原则：解压并行先上马得收益，其余格式 IO
> 仍按 §9.3 评估门走。

### 9.2 核心模式：Tokio + Rayon 双引擎（写进 `engine` 公共约定）

```rust
#[tokio::main]
async fn main() -> std::io::Result<()> {
    // IO 密集：Tokio 异步读
    let mut f = tokio::fs::File::open("sample.fastq.gz").await?;
    let mut buf = Vec::new(); f.read_to_end(&mut buf).await?;
    // CPU 密集：交给 Rayon 并行
    let results: Vec<_> = (0..n).into_par_iter().map(|i| process_chunk(&buf, i)).collect();
    // 回到 Tokio 异步写
    tokio::fs::write("out.bam", serialize(&results)).await?;
    Ok(())
}
```

**约定**：

- **解压永远后台/并行，不与主计算同线程串行**（直击 P2）：BGZF 用 `MultithreadedReader`，普通 gzip 用
  libdeflater/分块并行；解压与主计算通过 channel 流水线重叠。

- IO 边界（读文件/网络/子进程/写文件）一律 `await` 在 Tokio 上；计算热区（解析/计数/比对）包成无
  `await` 的纯函数交给 Rayon；两者用 `sync_channel`/`buffer` 衔接，避免"读完整个文件才计算"的内存峰值。

- 热路径一律流式，禁止 `read_to_end` 整读大文件（复核 P4 存量）。

### 9.3 外部生信库评估门（noodles / bio / needletail / seq\_io / debruijn / coitrees / triple\_accel）

| 门             | 要求                                                               |
| ------------- | ---------------------------------------------------------------- |
| G1 立基准        | 先以自有解析器在真机（Linux 4TB 数据）按 §7.1 口径采 wall/RSS 基准                   |
| G2 对口测        | 候选库在**同一数据、同一口径**跑，且**结果与自有解析器数值一致**（一致性判据同 §7.1）                |
| G3 跨平台        | 在 windows-gnu CI 全量测试通过（BAM async feature 在 Windows 的表现尤为关键）     |
| G4 license/集计 | license 审计通过（BIOSL 等需评审）、不引入调度冲突                                 |
| G5 写结论        | 收益 >0 且显著才替换；结果记入 `docs/engine-evals/<lib>-<date>.md` 并 `[x]` 标注 |

**当前建议（写入决策）**：

- **FASTQ 并发读取**：优先用 **seq\_io / needletail**（极速 FASTX，纯 Rust）做 IO，若满足 G1–G5；

- **近阶段不引入** noodles 的 async BAM/CRAM 直到为它单独立门（改动面大、Windows 兼容待证）；

- **不引入** debruijn / coitrees / triple\_accel，除非目标能力（组装/区间/编辑距离）benchmark 证明需要。

### 9.4 M5 任务清单（落地顺序与验收，按修复优先级重排）

| 任务     | 内容                                                                                                                                                                                                                                                                                                                             | 直击       | 验收                                                                                                                      |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- | ----------------------------------------------------------------------------------------------------------------------- |
| M5-T1  | **并行解压引擎**（最高优先）：新增 `linxira-bio-io` 的 `decompress` 模块——BGZF 输入用 `noodles-bgzf::MultithreadedReader`（多核解 BGZF block）；普通 gzip 用 `libdeflater` 或 `seq_io::parallel` 分块并行；`flate2` 保留为 `--serial` 基线对照                                                                                                                            | P1/P2/P3 | `cargo test -p linxira-bio-io` 绿；真机上 ≥64MB gz FASTQ：并行解压 wall < 串行 50%（smoke 断言）；输出字节与串行 flate2 逐字节一致                   |
| M5-T1a | **7z 集成（并行解压/归档另一条腿）**：`decompress` 对 `.7z`/`.zip`/`.tar.*` 归档优先调原生 `7z`/`7zz`（多线程，Linux 尤其，真机上常显著快于系统自带 `tar`/`gzip`）；未安装 `7z` 时回退自研/`flate2`。补齐 [tools/catalog.json](tools/catalog.json) 缺失的压缩工具项（当前无 7z/gzip/bzip2/xz/zstd 条目）——新增 `7z`（含 Linux `p7zip`/`7zip` 与 `7zz` 别名探测、Windows 常见安装路径 `C:\Program Files\7-Zip\7z.exe`） | P1/P2    | `tools/catalog.json` 含 `7z` 项且 `probe` 多平台可命中；≥64MB `.7z`/`.tar.xz`：7z 解压 wall < 自研串行（smoke）；`7z` 缺失时自动回退自研且单测绿         |
| M5-T2  | 解压与主计算**流水线重叠**：thread\_io 后台解压线程 + channel，让解析在解压进行时同步处理，不再"解压完再算"                                                                                                                                                                                                                                                            | P2       | 对同一 FASTQ：流水线 wall < 顺序解压再解析（smoke 断言）；数据逐字节一致                                                                          |
| M5-T3  | **Rayon 并行计算层**：把 fastq\_qc、kmer 计数、比对、sequence.stats 等 CPU 密集热路径包成无 `await` 纯函数 + `into_par_iter` 分块（固定线程数/分块/归约顺序保 DETERMINISM）                                                                                                                                                                                              | P1       | 结果与串行版逐字节一致；`sequence.stats` 多核 wall 明显下降                                                                               |
| M5-T4  | M0 的 `format_probe`/解压路径切换到 `linxira-bio-io`（IO 密集，同步行为不变）；`ensure_decompressed` 走并行解压                                                                                                                                                                                                                                         | P2       | M0-T9/T10 单测仍绿 + 并发解压 smoke 降时断言                                                                                        |
| M5-T5  | M2 的 benchmark 计时路径接入 `linxira-bio-io`：让 benchmark 跑的是并发版 IO + 并行解压，而非单线程同步版                                                                                                                                                                                                                                                   | P1/P2    | M2 收到的 wall/RSS 仍字段齐全；`summary.md` 标注 `io: async-threaded` + `dctx: bgzf-multithreaded\|libdeflater\|7z\|flate2-serial` |
| M5-T6  | **复核 P4 存量**：coordinate/domain/functional 的 `read_to_end` 按数据类别改流式，大文件不可整读                                                                                                                                                                                                                                                     | P4       | 大样本冒烟内存峰值下降；小文件结果不变                                                                                                     |
| M5-T7  | 生信库评估记录：对 seq\_io/needletail/noodles-bgzf/libdeflater 立 G1/G2 基准，输出 `docs/engine-evals/` 结论，标注 `[x]` 或「未达标准」。**benchmark 对比口径明确化**：Rust 并行解压 vs **7z 并行解压**（原生，非单线程系统工具），避免"Rust 并行 vs 单线程"的失衡对比；7z 版本记录进环境 pinning                                                                                                            | —        | 文档存在且 G1–G5 每项有结论；`docs/engine-evals/7z-<date>.md` 记录 Rust-bgzf/libdeflater vs 7z 的 wall/RSS/一致性                        |
| M5-T8  | CLI 并发 IO 冒烟 + CI 新增解析/解压并发冒烟步骤（含 7z 路径：可用时走 7z、缺失时回退）                                                                                                                                                                                                                                                                         | —        | CI 三平台绿；不回归现有 E2E                                                                                                       |
| M5-T9  | 文档：在 [AGENTS.md](AGENTS.md)/`docs/performance.md` 写明"解压并行（BGZF/libdeflater/7z、优先 7z 原生多线程）、计算用 Rayon、IO 用 Tokio+thread\_io、生信格式 IO 按 §9.3 门引入"；README 增声明「压缩/解压优先复用原生多线程 7z，Rust 并行引擎作为兜底与基准」                                                                                                                                  | —        | validate 脚本绿；README 增「并发能力」说明                                                                                           |

> **DETERMINISM 红线**：所有 benchmark 数值比较用同一输入、同一线程数、同一分块顺序；多线程归约
> 若产生非确定性位移，必须固定 reduce 顺序，否则按 §7.1 一致性判据会误报 `inconsistent`。

## 10. M6 —— CLI 契约加固 · 安装器与环境变量 · Agent 打通

**目标**：让「**装完即用**」（安装器 + PATH 注册，Windows 首位、Debian/Arch 同步）与
「**AI 可操控**」成立。Agent 通路分四层，按落地难度递增：

| 层  | 通路                                                                                          | 现状                   | 定位                                   |
| -- | ------------------------------------------------------------------------------------------- | -------------------- | ------------------------------------ |
| L1 | **CLI**（AI 直接调 shell + `--json`）                                                            | 已有命令但契约未成文           | **最简单、最先落地的 agent 通路**（用户已拍板）        |
| L2 | **Skills**（[skills/](skills/) + [skill-pack.json](skill-pack.json) 导入 agent 运行时）            | 已有 ≈38 个 skill       | agent 指令层，随新命令同步增补                   |
| L3 | **MCP server**（[linxira-bio-mcp](engine/crates/linxira-bio-mcp/src/main.rs)，stdio JSON-RPC） | 已进发布包（2026-09-19）；客户端注册片段待 M6-T8 文档 | 面向 Claude Desktop / Cursor 等 MCP 客户端 |
| L4 | Python SDK                                                                                  | 规划中                  | 归入 M8 愿景                             |

### 10.1 CLI 契约（agent 友好的硬性规定，全命令适用，写入 `docs/AI_AND_SDK.md`）

| 规则                      | 规格                                                                                                                                                                                                                                                                                                                                              |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 退出码                     | `0` 成功；`2` 用法错误（参数不合法）；`3` 能力执行失败（数据/算法错误）；`4` 环境缺失（原生工具/运行时不在 PATH）；`5` 未预期内部错误                                                                                                                                                                                                                                                                |
| 输出分离                    | 结果走 stdout；日志/诊断走 stderr；`--json` 开启时 stdout **只含一个合法 JSON 文档**（V2 envelope），禁止混排人类可读文本                                                                                                                                                                                                                                                         |
| 零交互                     | 任何命令不得等待 stdin 确认；需确认的场景提供 `--yes` 或直接失败并给可行动错误                                                                                                                                                                                                                                                                                                 |
| 错误结构                    | `--json` 下错误为 `{code, message, action_hint}`；`action_hint` 必须给下一步动作（如「安装 sra-tools 后重试」）                                                                                                                                                                                                                                                        |
| 可发现性                    | `--version` / `--help` 每命令必有；`capabilities list --json` 输出全量能力与命令模板（MCP tools schema 的生成源）                                                                                                                                                                                                                                                      |
| 幂等路径                    | 输入路径来自 agent 时可能带引号/`file://`/`~`，一律先过 M0-T5 `resolve_input_path`                                                                                                                                                                                                                                                                               |
| **参数语法（防 shell 打架）**    | (a) 只用 POSIX 风格 `--long-flag` / `-f`，**禁止** Windows `/x` 风格（PowerShell 歧义）；(b) `--flag value` 与 `--flag=value` **两种形式必须都接受**（PowerShell 下 `=` 形式是单 token，最不易被拆坏，文档示例一律用 `=` 形式）；(c) 支持 `--` 终止符与 `-`=stdin；(d) 多词值（含空格/引号）一律改走 `--request <job.json>` 或 stdin JSON，**禁止**依赖内联引号转义；(e) 全局旗标统一：`--json --threads N --output <dir>`，语义跨命令一致，不搞同名异义 |
| **`--request`** **逃生舱** | 每个能力命令额外接受 `--request <job-request.json>`（直接复用 V2 `JobRequestV2`）：agent 生成复杂参数时写一个 JSON 文件即可，**完全绕开 shell 引号问题**——这是给 AI 的最稳通路，比 20 个内联旗标可靠得多                                                                                                                                                                                                   |
| **shell 引号回归矩阵**        | CI 新增：同一条命令分别在 bash / zsh / cmd / Windows PowerShell 5.1 / PowerShell 7 下执行，断言 argv 完全一致（专测 `--flag=value`、带空格路径、`--` 终止符三类）——防"AI 生成命令在 PowerShell 打架"回归                                                                                                                                                                                       |

### 10.2 任务清单

| 任务                | 内容                                                                                                                                                                                                                                                                                                                                                                                                 | 验收                                                                                                                                                                      |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M6-T1（可提前至 M2 并行） | CLI 契约落地：审计 [main.rs](engine/crates/linxira-bio-cli/src/main.rs) 现有命令的退出码并统一到 §10.1 表；新增 `capabilities list [--json]`；落实参数语法规则（`=`/空格双形式、`--`、`-`=stdin）与 `--request` 逃生舱                                                                                                                                                                                                                          | 单测断言 5 类退出码路径各至少 1 例；`docs/AI_AND_SDK.md` 增契约章节；引号回归矩阵（bash/zsh/cmd/PS5.1/PS7）进 CI 并断言 argv 一致                                                                          |
| M6-T2（可提前）        | `doctor` / `environment audit` 扩展：校验 (a) `linxira-bio` 在 PATH；(b) workflows 根可解析；(c) R/Python 运行时；(d) M0/M2 引入的原生工具（sra-tools、**7z** 等，7z 经 [tools/catalog.json](tools/catalog.json) `probe`，缺失时提示「安装 p7zip/7-Zip 以启用多线程解压」）；输出结构化 JSON + `action_hint` 修复建议                                                                                                                                       | 复用 \[system.doctor.v1] / environment.audit.v1 能力；`doctor --json` 三平台 CI 冒烟（含缺失 7z 的 action\_hint 断言）                                                                    |
| M6-T3             | **Windows 安装器（单文件 EXE，定案）**：用 **NSIS** 生成单个自解压 `LinxiraBio-Setup-<ver>.exe`——装到 `%LOCALAPPDATA%\Programs\LinxiraBio\`；追加 `HKCU\Environment\Path`（安装时记录原值快照，仅追加安装目录）；广播 `WM_SETTINGCHANGE`；开始菜单可选；同 exe 提供卸载。**明确不走 MSI/MSIX**（用户定案：怎么简单怎么来）。**组件化安装**：Core（CLI+worker+MCP，必装）与 Desktop UI（egui 桌面端，可选、默认勾选）两个 NSIS Section——Windows 服务器/CI 场景可只装 CLI。**7z 作为可选内置/提示项**：检测到系统无 7-Zip 时给出安装建议，不做强制绑定 | 全新 Windows 虚拟机双击 Setup.exe → 安装完成后**新开终端** `linxira-bio --version` 直接可用；勾掉 Desktop UI 只装 Core 后 CLI/doctor/MCP 全可用且体积明显更小；卸载后 PATH 仅移除自己追加的条目（用户其他条目不动）                 |
| M6-T4             | **Debian 双包拆分（Linux 服务器友好）**：`linxira-bio`（**无头包**：CLI+worker+MCP+workflows/schemas/skills/docs，进 `/usr/bin` + `/usr/share/linxira-bio/`，**零 GUI 依赖**——不拖 X11/Wayland/wgpu/GL）+ `linxira-bio-desktop`（**桌面包**：仅 `linxira-bio-ui` 二进制，`Depends: linxira-bio` + GUI 运行库）。`p7zip-full` 作 headless 包的 recommends（非硬依赖），Rust 自研解压兜底                                                                     | 干净**无图形栈** Debian 容器 `apt install ./linxira-bio_<ver>.deb` 后 `linxira-bio --version` + `sequence stats` 冒烟通过且 `apt rdepends` 无任何 GUI 库；`linxira-bio-desktop` 安装后 UI 可启动 |
| M6-T5             | **Arch 双包拆分**：与 Debian 同策略——`linxira-bio`（无头）+ `linxira-bio-desktop`（optdepends 可选依赖，Rust 解压兜底）                                                                                                                                                                                                                                                                                                    | 干净 Arch 容器 `makepkg -i` 无头包后同上冒烟；无图形栈环境不引入 mesa/libx11                                                                                                                  |
| M6-T6             | [bundle-manifest.json](packaging/bundle-manifest.json) `binaries` 三平台均补 `linxira-bio-mcp`，并给每个 binary 增加 `"component": "core"\|"desktop"` 字段（M6-T3/4/5 分体安装的数据源）；[stage-release.py](scripts/stage-release.py) 适配                                                                                                                                                                                   | `--check` 通过；staging 产物含 4 个二进制且 component 标注正确；MCP 能在包内定位同目录 worker（`find_worker_binary` 已支持 exe 同目录查找）                                                                |
| M6-T7             | **MCP server 补齐**：(a) `tools/call` 支持新命令（`export bio` / `import probe` / `benchmark run`）；(b) `protocolVersion` 升至当前 MCP 规范版本并在 result 声明；(c) 无 command 模板的能力不出现在 `tools/list`；(d) [smoke-mcp-server.py](scripts/smoke-mcp-server.py) 扩展并纳入 CI 必跑链                                                                                                                                                 | smoke 三平台 CI 绿；至少一个 MCP 客户端（Claude Desktop 或 Cursor）手工接入冒烟通过                                                                                                            |
| M6-T8             | Agent 接入文档：`docs/AI_AND_SDK.md` 增「四层通路」章节——L1 CLI 契约、L2 skills 导入（[skill-pack.json](skill-pack.json)）、L3 MCP 各客户端配置片段（Claude Desktop / Cursor / 通用 stdio JSON）、L4 SDK 规划；[README.md](README.md) 的「MCP server: planned」改为实际状态                                                                                                                                                                       | validate 脚本绿；文档内配置片段可直接复制使用                                                                                                                                             |
| M6-T9             | skills 同步：[route-bio-analysis](skills/route-bio-analysis/SKILL.md) 等路由 skill 增补新命令入口（export bio / import probe / benchmark run / capabilities list / doctor）                                                                                                                                                                                                                                       | skill 校验器绿                                                                                                                                                              |
| M6-T10            | **UI 性能守卫（保持 egui，不迁 Dioxus——定案）**：核查结论已确认 egui 刷新纪律正确（空闲不重绘、任务期 100ms 轮询、耗时任务全在后台线程），迁移声明式框架无性能收益、只有重写成本。落地三件事：(a) 数据集列表等 6 处非虚拟 `ScrollArea` 改 `show_rows` 虚拟化（大量导入时防每帧全量布局）；(b) 空闲 CPU≈0 守护测试（启动 UI、无任务静置 N 秒、断言无 repaint 请求）；(c) `docs/performance.md` 写明 egui 刷新纪律与后台线程模式为 UI 性能基线                                                                                                            | 大列表（≥10k 行）滚动帧时间不随行数线性增长；空闲守护测试三平台 CI 绿；文档完成                                                                                                                            |

### 10.3 平台安装与 PATH 策略对照（含 CLI/桌面分体）

| 平台              | 安装位置                                             | PATH 机制                                                         | CLI/桌面分体                                                             | 卸载清理                                 |
| --------------- | ------------------------------------------------ | --------------------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------ |
| Windows（首要桌面平台） | `%LOCALAPPDATA%\Programs\LinxiraBio\`            | `HKCU\Environment\Path` 追加 + `WM_SETTINGCHANGE` 广播（新终端即生效，无需重启） | 单 EXE 双组件：Core（CLI+worker+MCP，必装）+ Desktop UI（可选默认勾选）                | 仅移除自己追加的条目（安装时快照回滚）；**绝不写 HKLM 系统级** |
| Debian          | `/usr/bin/`（二进制）+ `/usr/share/linxira-bio/`（资源树） | 无需注册（`/usr/bin` 天然在 PATH）                                       | 双包：`linxira-bio`（无头，零 GUI 依赖，服务器友好）+ `linxira-bio-desktop`（UI，依赖无头包） | dpkg 自动                              |
| Arch            | 同 Debian 布局                                      | 同上                                                              | 同 Debian 双包策略                                                        | pacman 自动                            |

> **Windows 安装器形态（定案）**：单文件自解压 **NSIS EXE**（`LinxiraBio-Setup-<ver>.exe`），仅引入 NSIS 一个可选构建依赖；**不走 MSI/MSIX/WiX**（用户明确：怎么简单怎么来）。MSI/MSIX 仅在将来有企业级分发诉求（域控/GPO 自动部署）时再评估。
>
> **分体逻辑（定案）**：Linux 服务器场景 = 只装无头包（无 X11/Wayland/wgpu/GL 依赖树）；桌面用户 = 无头包 + 桌面包；Windows 单安装器勾选组件即等效。**CLI 与 UI 共享同一 worker/协议/资源树**，只是二进制分发面不同——不存在"CLI 版功能阉割"，只有"有没有 UI 壳"的差别。

> Windows PATH 安全红线：只操作当前用户（HKCU）；写入前快照；卸载精确回滚；
> CI 在全新虚拟机验证安装→新终端可用→卸载→PATH 干净三步。

## 11. 待补充数据方法总清单（能力缺口，按优先级）

> **数据类别**：SRR=原始测序/需解压 · 注释=GFF/GTF/BED/VCF · 矩阵=表达/计数矩阵 · 结构=PDB/mmCIF ·
> 序列=FASTA/FASTQ · 样本表=临床/分组表 · 其他=化学/质谱等。
> **实现** 列加粗 = 该语言为权威实现；「原生」= 封装成熟命令行工具。
> **状态**：`未开始` / `排期 M4` 等；完成时改 `[x]` + 日期。

### 🔴 首批（M4 落地，benchmark 主战场）

| 能力                                 | 生态对标                 | 实现(Rust/Py/R)             | 数据类别        | 绘图       | 状态    |
| ---------------------------------- | -------------------- | ------------------------- | ----------- | -------- | ----- |
| 转录定量 featureCounts/salmon/kallisto | HTSeq/salmon         | Rust 编排+原生 quant / Py / R | SRR+BAM     | —        | 排期 M4 |
| 单细胞 RNA：质控/降维/聚类/marker            | Seurat(R)/Scanpy(Py) | **R + Py 双实现**            | 矩阵(H5ad)    | ✅UMAP/散点 | 排期 M4 |
| peaks calling + ChIP/ATAC 富集       | MACS2/SEACR          | 原生 MACS2                  | SRR+BAM     | ✅峰型图     | 排期 M4 |
| 甲基化 BS-seq / DMR 检测                | Bismark/MethylKit    | Py + R                    | SRR(BS-seq) | ✅DMR图    | 排期 M4 |
| 从头组装 de novo                       | SPAdes/miniasm       | 原生 SPAdes                 | SRR         | —        | 排期 M4 |
| 组装质控 N50/完整性/污染                    | QUAST/CheckM/BUSCO   | Rust 或 Py                 | SRR组装       | ✅统计条图    | 未开始   |
| 表达定量归一化(TPM/FPKM/RPKM)             | 自研                   | Rust / Py / R             | 矩阵          | —        | 未开始   |
| 差异可变剪接/异构体                         | DEXSeq/rmats         | **R**                     | SRR+BAM     | ✅剪接图     | 未开始   |

### 🟠 二批

| 能力                     | 生态对标                     | 实现(Rust/Py/R)   | 数据类别      | 绘图      | 状态  |
| ---------------------- | ------------------------ | --------------- | --------- | ------- | --- |
| 16S 扩增子 DADA2/ASV/分类   | QIIME2/DADA2             | **R(DADA2)**    | SRR(16S)  | ✅bar/树  | 未开始 |
| 宏基因组 组装/分箱/功能谱         | metaSPAdes/MaxBin/HUMAnN | Py              | SRR       | ✅丰度图    | 未开始 |
| 群体遗传 Fst/PCA/STRUCTURE | plink/adegenet           | **R(adegenet)** | 变异        | ✅PCA/结构 | 未开始 |
| 结构变异 SV 检测             | Delly                    | 原生              | SRR+BAM   | ✅断点图    | 未开始 |
| CNV 检测(靶向/外显子)         | CNVkit/GATK              | Py/R            | BAM       | ✅CNV图   | 未开始 |
| GWAS + 曼哈顿/QQ图         | plink/GAPIT              | **R**           | 变异+样本表    | ✅曼哈顿    | 未开始 |
| Hi-C 接触矩阵/TAD 检测       | HiC-Pro/HiCExplorer      | Py              | SRR(Hi-C) | ✅热图/染色质 | 未开始 |
| 蛋白信号肽/跨膜/亚细胞           | Phobius/TMHMM/DeepLoc    | Py              | 序列        | ✅拓扑图    | 未开始 |
| 蛋白家族/结构域功能注释           | InterProScan/Pfam        | 原生              | 序列/结构     | ✅域图     | 未开始 |
| 通路图叠加(富集映射)            | pathview/KEGG            | **R**           | 富集结果      | ✅通路图    | 未开始 |

### 🟡 三批

| 能力                    | 生态对标                   | 实现(Rust/Py/R)  | 数据类别         | 绘图     | 状态  |
| --------------------- | ---------------------- | -------------- | ------------ | ------ | --- |
| 引物设计(Tm/GC/二级结构)      | Primer3                | Rust + Primer3 | 序列           | —      | 未开始 |
| 密码子使用/偏好              | EMBOSS cusp            | Rust           | 序列           | ✅bar   | 未开始 |
| 两两序列比对(SW/NW/贪心)      | parasail/EMBOSS        | Rust           | 序列           | ✅点图    | 未开始 |
| 通用统计检验(t/卡方/ANOVA/相关) | scipy.stats            | Py             | 样本表/矩阵       | —      | 未开始 |
| 基因集富集(超几何/卡方)补充       | clusterProfiler        | **R**          | 富集           | ✅bar   | 未开始 |
| 生存分析/COX/Lasso 蛋白组学   | survival/glmnet        | **R**          | 样本表          | ✅KM曲线  | 未开始 |
| 代谢组学归一化/差异代谢物         | XCMS/MetaboAnalyst     | R/Py           | 矩阵(质谱)       | ✅火山/箱线 | 未开始 |
| 质谱肽段鉴定/定量             | MaxQuant/DIANN         | 原生             | 其他(RAW/mzML) | ✅色谱    | 未开始 |
| 药物基因组注释(星等位基因)        | PharmGKB               | Py/R           | 变异           | —      | 未开始 |
| 空间转录组聚类/去卷积           | Seurat spatial/Squidpy | Py/R           | 矩阵(img)      | ✅空间图   | 未开始 |
| 三维基因组 A/B 区室/TAD      | HiCExplorer/cooler     | Py             | SRR(Hi-C)    | ✅染色质图  | 未开始 |
| RNA 编辑/修饰检测           | REDItools/JACUSA2      | 原生             | SRR          | ✅位点图   | 未开始 |

> 约束重申：§11 是**封闭清单**——任何新能力先入表、标排期，再动工；动工即触发四同步 + `benchmark-datasets.json` 登记。

## 12. Phase Gate（阶段性检查标准）

### 12.1 通用门槛（每个里程碑必须全绿）

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
python scripts/validate-repository.py
```

外加：CI 三平台（windows-gnu / debian-bookworm / arch-linux）全绿；改动过的每个 `skills/` 文件夹过 skill 校验器。

### 12.2 里程碑特定验收（逐条可勾选）

- [ ] **M0**：`cargo test -p linxira-bio-output` 绿；`export bio` 产物被 bedtools/bcftools/samtools/gffread 参照校验通过（§5.2 基准文件）；`import probe` 对 `.sra/.gz/.vcf.gz` 样本返回正确格式与置信度；`resolve_input_path` 全平台用例绿；`ROADMAP.md` 已在根目录。

- [ ] **M1**：matplotlib + ggplot2 两 pack 同一 PlotSpec 出图，图数据断言一致；GUI 图片查看器缩放/翻页手测通过；README 含绘图原生后端声明；`plot-render-result` 含 PlotSpec sha256。

- [ ] **M2（核心）**：`benchmark run` 对 `tests/fixtures` 三后端 `consistency=consistent` 且 wall\_ms/peak\_rss\_mb 非空；`run-benchmark-linux.sh` 对 4TB/SRR 实跑产出 `summary.md`（含 §7.1 全部披露项）；`runtime-preferences.json` 正确写回且 worker `--backend auto` 路径单测绿；SRR 分段计时齐全；主站 benchmark 页数字与 summary.json 一致、每卡片带数据来源链接。

- [ ] **M3**：每个 pack 与 Rust 结果 ±1e-6 相对误差内一致（PCA 符号规范化后）；CI 含对应 E2E。

- [ ] **M4**：每个新能力 schema+SKILL+中英文文档+E2E 四同步；原生工具入 `native_tools` 白名单 + `environment` 探测；`catalog.json` 状态 `available`。

- [ ] **M5（并发/IO 引擎）**：`cargo test -p linxira-bio-io` 绿；并行解压 vs `flate2` 串行 wall 下降且输出逐字节一致；解压与计算流水线 wall 下降；`sequence.stats`/fastq\_qc Rayon 多核 wall 明显下降且与串行逐字节一致（DETERMINISM 保真）；benchmark 路径标注 `io: async-threaded` + `dctx: …`；P4 `read_to_end` 存量已复核改流式；生信库评估记录 `docs/engine-evals/` 存在且 G1–G5 逐项有结论。

- [ ] **M6**：三平台安装后**新开终端** `linxira-bio --version` 直接可用（Windows 验证 PATH 注册、卸载精确回滚、Core-only 组件可选；Debian/Arch 无头包在无图形栈容器冒烟且 `apt rdepends` 零 GUI 库）；退出码契约 5 类路径单测绿；引号回归矩阵（bash/zsh/cmd/PS5.1/PS7）CI 绿；`--request` JSON 逃生舱可用；`linxira-bio-mcp` 进入发布包且 `smoke-mcp-server.py` 三平台 CI 绿；至少一个 MCP 客户端接入冒烟通过；`docs/AI_AND_SDK.md` 四层通路文档完成；README 的 MCP 状态已更正；UI 大列表虚拟化 + 空闲 CPU≈0 守护测试绿（M6-T10）。

- [ ] **M7**：CI 含绘图 pack 冒烟、`export bio` 往返、benchmark 小样本 job（三后端一致性断言）+ 专用 ubuntu `benchmark` job（`--smoke`）；三平台 `cargo build --release -p linxira-bio-ui` 零告警；主站 + 组织 README 链接齐全；安装器产物（NSIS 单文件 EXE、.deb、PKGBUILD）进 release 资产；打 tag 触发 release。

### 12.3 长期治理

- **数据治理**：见 §2 不变量 2；来源引用与展示规则见 [docs/BENCHMARK\_DATA\_SOURCES.md](docs/BENCHMARK_DATA_SOURCES.md) §6（`summary.json` 每条记录带 `dataset` + `source_url`）。

- **后端共识**：每次跑分后把「最优后端 + dataset\_class」写回 `runtime-preferences.json`（含 measured 原始数字，可追溯可复现）。

- **Windows 降级透明**：Windows 上 benchmark 精度降级必须标注 `precision: degraded`，不与 Linux 数字混排对比。

- **Agent 契约冻结**：§10.1 CLI 契约一经发布即视为对外承诺（agent 脚本会依赖退出码与 JSON 结构），变更需升版本并在 `AI_AND_SDK.md` 记录变更说明。

## 13. M7 / M8 —— CI 加固、发布与愿景

**M7 任务**：

1. [.github/workflows/ci.yml](.github/workflows/ci.yml)：每平台增加（a）matplotlib/ggplot2 pack 冒烟、（b）`export bio` 往返测试、（c）benchmark 小样本（断言一致性 + 计时非空）、（d）`smoke-mcp-server.py` 必跑；新增 ubuntu 专用 `benchmark` job 跑 `scripts/run-benchmark-linux.sh --smoke`。
2. 新增 `tests/python/test_benchmark_*.py`、`workflows/tests/test_output_packs.py`。
3. 主站 `bio-sdk/benchmark.html` + 组织仓库 README 加 benchmark 结果链接；随每次跑分更新 `benchmark-results/`。
4. 发布物：三平台安装器产物（M6-T3/4/5）+ `stage-release.py` 打包的 zip 均挂 release 资产，附 sha256。

**M8 愿景**：MCP server 稳定（L3 通路成熟）、Python SDK 正式化（L4 通路）、HPC/云后端、按 benchmark 数据引入 C++/Java 内核（须先有 §7.1 口径的 benchmark 证据）。长期目标：`runtime-preferences.json` 成为「每种分析默认用最优语言」的全平台共识，主站公开数据让用户按自己的数据类别选后端。

## 14. 风险与缓解

| 风险                                                                 | 影响                   | 缓解                                                                                                                                  |
| ------------------------------------------------------------------ | -------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| 原生工具三平台不可得（MACS2/SPAdes 等在 Windows 难装）                             | M4 CI 阻塞             | 原生工具能力在 Windows 上走 WSL Debian 兼容层（README 既定策略）；CI 对应步骤仅 Debian/Arch 启用                                                              |
| 逐字节对齐困难（如 VCF 头部版本差异）                                              | M0 验收扯皮              | §5.2 每格式固定基准文件 + 参照工具校验命令，对齐目标以基准文件为准                                                                                               |
| 三端数值差异超容差（随机种子/BLAS 实现）                                            | M3 验收失败              | 固定随机种子；PCA 符号规范化；超差字段列入 `inconsistent` 清单并人工裁定                                                                                      |
| 4TB 跑分周期长                                                          | M2 迭代慢               | `--smoke` 小样本先行；大文件按 dataset\_class 分批跑，增量写 summary                                                                                 |
| `schema_version` 字符串化导致 CI 失败（历史教训）                                | 全里程碑                 | 所有新 schema 一律整数 `"1"`，validate 脚本加断言                                                                                                |
| benchmark 结果被质疑不公                                                  | 主站可信度                | §7.1 方法学强制披露（环境、口径、版本、原生基准=生态对标工具本身）                                                                                                |
| **7z 缺失 vs 可用导致解压性能口径不一**                                          | benchmark 口径漂移       | §9.4 M5-T5/T7 强制：`dctx:` 记录实际解压器；7z 缺失时自动回退自研并**在 summary 标注** **`dctx: flate2-serial`**，不得混排 7z 数字；环境 pinning 记录 7z 版本             |
| **7z 是原生工具，复用它不算 Rust 的并发卖点**                                      | 主站业绩水分               | README/§9.2 措辞冷静：7z 用于**消除"Rust 并行 vs 单线程系统工具"的失衡对比**，Rust BGZF/libdeflater 与其同口径对标；benchmark 同时发布 Rust-并行 vs 7z vs 串行三组数字，避免只挑有利口径 |
| **Rust 并行解压可能反而不如 7z**（libdeflater/SIMD 或 BGZF 收益不稳）               | M5 验收失利              | 接受并记录：`docs/engine-evals/7z-<date>.md` 如实标注胜负；benchmark 卖点转移到「Rust 在解压+解析+下游管线整体编排」上而非单纯解压，避免为虚荣牺牲诚实                                |
| **AI 生成命令在 PowerShell 下引号/参数打架**（PS 5.1 拆坏 `--flag value`、空格路径被截断） | agent 通路可用性受损、用户信任下降 | §10.1 参数语法硬规则（双形式 + `=` 文档示例 + `--request` JSON 逃生舱）+ CI 引号回归矩阵（bash/zsh/cmd/PS5.1/PS7 断言 argv 一致）；文档给 PowerShell 专用示例              |
| **无头包误拖 GUI 依赖**（wgpu/GL 链接进 `linxira-bio` 无头 deb）                 | Linux 服务器安装体积/兼容性翻车  | M6-T4 验收含 `apt rdepends` 断言零 GUI 库；stage-release 按 bundle-manifest `component` 字段分流产物，CI 在无图形栈容器安装冒烟                                |
| Windows 安装器写坏用户 PATH                                               | 用户机器受损、口碑风险          | 只操作 HKCU（绝不写 HKLM 系统级）；写入前快照原值；卸载精确回滚自己追加的条目；CI 在全新虚拟机验证「安装→新终端可用→卸载→PATH 干净」三步                                                     |
| MCP 协议演进快（tools schema / protocolVersion 变更）                       | M6-T7 返工             | MCP 只承诺 stdio JSON-RPC 传输层稳定；协议版本升级通过 `smoke-mcp-server.py` 回归兜底；CLI（L1）始终是保底 agent 通路，不因 MCP 变更而破坏                                 |
| 用户终端装完不重启导致 PATH 未生效                                               | 「装完即用」被质疑            | 安装器广播 `WM_SETTINGCHANGE` + 安装完成页明示「新开终端」；`doctor` 命令可自检 PATH 是否包含安装目录并给 action\_hint                                                |
| 生信库（noodles 等）跨平台/async 兼容性                                        | M5 CI 阻塞             | §9.3 评估门强制 G3 全量 windows-gnu 测试后才准进入；近阶段保留自有解析器，不欠依赖                                                                                |
| 多线程导致 benchmark 数值非确定（归约顺序抖动）                                      | M5-T4 一致性误报          | 固定线程数/分块顺序/归约顺序（§9.4 DETERMINISM 红线）；before vs after 并行版做逐字节对照                                                                      |
| Tokio/Rayon 引入增加体积与初次构建时间                                          | 桌面包膨胀                | 依赖默认 feature 裁剪；仅 `linxira-bio-io` 承载并发栈，核心算法 crate 不强依赖并发运行时                                                                       |
| license 审计卡壳（BIOSL 等外部生信库）                                         | M5-T5 评估中断           | G4 gate 前置 license 审计；不过关则该库标「不采用」并给替代（seq\_io/needletail 均为 MIT/Apache 双许）                                                         |

