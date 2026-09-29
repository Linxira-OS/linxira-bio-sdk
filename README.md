# Linxira Bio SDK

Linxira Bio SDK is a local-first, agent-native bioinformatics execution
toolkit. It combines concise skills with stable CLI, SDK, and tool contracts so
that routine analyses use tested implementations instead of generating a new
script for every run.

Windows is the primary desktop and beginner-facing platform. Debian and Arch
are the supported Linux families for workstation, server, and HPC use. macOS is
not currently a tested or packaged target.

On Windows, WSL Debian is the compatibility provider for older bioinformatics
components, while WSL Arch is the current-platform provider and the future
Linxira WSL foundation. Linxira WSL installation remains planned until a
versioned rootfs and upgrade contract are published.

The repository is the canonical home of executable bioinformatics skills and
their shared runtime. It does not replace `linxira-skills`, which remains the
cross-discipline skill router and installer for research, Linux, HPC, cloud,
browser, and delivery workflows.

## Product Surfaces

- `linxira-bio`: command-line interface for people and workflows
- `skills/`: agent instructions bound to versioned capabilities
- `capabilities/`: machine-readable capability catalog
- `engine/`: Rust runtime and future benchmark-justified C++ kernels
- `skill-pack.json`: import boundary for agent runtimes and `linxira-skills`
- `linxira-bio-ui`: native Rust desktop application without a WebView
- Python SDK: planned after the CLI contract stabilizes
- `linxira-bio-mcp`: stdio JSON-RPC MCP server (initialize / tools list /
  tools call / resources list+read; tool calls execute worker jobs). Point an
  MCP client at the binary; capability ids from `capabilities/catalog.json`
  are the tool names

## Current Capabilities

The current local core audits bioinformatics prerequisites, identifies and
previews common biological files, calculates deterministic FASTA, FASTQ, SAM,
VCF, BED-intersection, expression-matrix, and PDB metrics, and exports
structured result tables as CSV, TSV, JSON, JSONL, or XLSX:

```bash
cargo run -p linxira-bio-cli -- environment audit --json
cargo run -p linxira-bio-cli -- environment plan sequence-search --mode managed-user --json
cargo run -p linxira-bio-cli -- runtime catalog --json
cargo run -p linxira-bio-cli -- workflow packs --json
cargo run -p linxira-bio-cli -- dataset inspect tests/fixtures/data-inspection/variants.vcf --json
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
cargo run -p linxira-bio-cli -- fastq qc tests/fixtures/fastq-qc/valid.fastq --json
cargo run -p linxira-bio-cli -- alignment qc tests/fixtures/alignment-qc/valid.sam --json
cargo run -p linxira-bio-cli -- alignment bam-cram-qc sample.bam alignment-stats.tsv --json
cargo run -p linxira-bio-cli -- alignment coverage sample.bam coverage.tsv --json
cargo run -p linxira-bio-cli -- alignment short-read reference.fa reads.fastq aligned.bam --threads 4 --json
cargo run -p linxira-bio-cli -- variant stats tests/fixtures/variant-stats/mixed.vcf --json
cargo run -p linxira-bio-cli -- interval intersect tests/fixtures/interval-intersect/left.bed tests/fixtures/interval-intersect/right.bed --json
cargo run -p linxira-bio-cli -- expression matrix-qc tests/fixtures/expression-matrix/counts.tsv --json
cargo run -p linxira-bio-cli -- structure pdb tests/fixtures/structure-pdb-summary/alphafold-style.pdb --alphafold-plddt --json
cargo run -p linxira-bio-cli -- export table result.json result.xlsx
```

Environment plans support `local-core`, `scripting`, `managed-runtimes`,
`containers`, `sequence-search`, `genomics-cli`, and `full-local`. They are
read-only. Installation remains a separate, explicitly approved capability.
Set `GITHUB_PROXY` to resolve canonical GitHub release URLs through a trusted
download proxy.

Planning modes are `use-existing`, `managed-user`, `project-isolated`, and
`system-missing-only`. Every plan includes a dry-run transaction boundary;
`environment.apply.v1` remains planned and cannot execute that preview.

Inspect the runtime and capability catalog:

```bash
cargo run -p linxira-bio-cli -- doctor --json
cargo run -p linxira-bio-cli -- capabilities --json
cargo run -p linxira-bio-worker -- tests/fixtures/jobs/sequence-stats.json
cargo run -p linxira-bio-worker -- tests/fixtures/jobs/dataset-inspect.json
cargo run -p linxira-bio-ui
cargo run -p linxira-bio-ui -- tests/fixtures/structure-pdb-summary/alphafold-style.pdb
```

The native GUI provides capability-aware result charts for FASTA, FASTQ, SAM,
BED intersections, expression matrices, VCF, and PDB summaries. It renders
local plain or gzip-compressed PDB/mmCIF coordinates as backbone,
ball-and-stick, or space-filling representations. Structure files stay local
and are bounded to 128 MiB after decompression and 100,000 atoms. The current
view can be exported atomically as a 1600 by 1000 PNG. PDB analysis and
optional explicit AlphaFold pLDDT handling use
`structure.pdb.summary.v1`; mmCIF is currently a viewer input, not an available
analysis capability.

First-party Python and R workflow scripts ship under `workflows/` with their
schemas, tests, locks, and license notices. They are local optional backends:
the release does not bundle third-party interpreters, packages, databases, or
models. `linxira-bio workflow run` verifies each pack before directly invoking
an approved local interpreter. A cataloged pack is not an installed runtime or
an available analysis capability.

Release bundles are staged from `packaging/bundle-manifest.json`, which always
includes the canonical bilingual `docs/` tree, schemas, catalogs, skills, and
license notices. Repository contract checks use the pinned Draft 2020-12
validator in `requirements-ci.txt`:

```bash
python -m venv .venv-ci
# Activate .venv-ci, then run:
python -m pip install --requirement requirements-ci.txt
python scripts/validate-repository.py
python scripts/generate_third_party_notices.py --check-config
python -m unittest discover -s tests/python -p "test_*.py"
python scripts/stage-release.py --check
```

Platform packaging calls the same release staging script with its compiled
binary directory. Staging resolves the locked target-specific release graph
and generates `THIRD_PARTY_DEPENDENCIES.json` plus
`THIRD_PARTY_DEPENDENCIES.txt`; missing, ambiguous, stale, or modified license
text fails the release. See `docs/DEPENDENCY_NOTICES.md`.

## Execution Model

Local execution is the default. Move work to a local GPU, an institutional
scheduler, or approved cloud compute only when measured CPU time, memory, GPU,
database, or storage requirements exceed the local execution envelope.

Browser-only services are connectors, not compute kernels. They require an
explicit user action gate, human-controlled authentication, and compliance with
the service terms. The project never stores or auto-fills account credentials.

See `docs/ARCHITECTURE.md`, `docs/RUNTIME_MANAGEMENT.md`,
`docs/AI_AND_SDK.md`, `docs/DOCUMENTATION_POLICY.md`, and the existing policy
documents for the product boundary, staged scope, supported data formats, and
non-Visual-Studio build direction. The exact read, inspect, analysis, and
export matrix is in `docs/DATA_FORMATS.md`.

## On Rust

This is not a "rewrite everything in Rust" project, and Rust is not an
ideology here. The engine uses Rust because its design language — ownership,
explicit errors, and cargo/clippy as a shared discipline — makes development
and review calmer: whole classes of defects are caught before the program
runs, and deterministic, schema-checked outputs become the default rather
than an aspiration. Rust is chosen where that development and checking
confidence pays for itself.

The boundary is deliberate. Mature native tools (salmon, kraken2, BLAST,
HMMER, minimap2) stay external and are orchestrated, not reimplemented.
Python and R are first-class backends with their own independent
implementations, and plotting defaults to matplotlib and ggplot2. Rust takes
the parts it is good at; everything else keeps the tool it already fits.

## Plotting

Plotting defaults to the native Python (matplotlib) and R (ggplot2) packs
(`plot.render.v1`); the Rust SVG visualizers stay as the zero-dependency
fallback for their dedicated chart families until a mature Rust plotting
stack is adopted. Figures are deterministic: identical PlotSpec inputs render
to identical bytes, and every visual parameter (axes, theme, palette, size,
dpi, format, output filename) is explicit.

## Benchmarks

Every measured performance claim lives under `benchmark-results/` and follows
the archival convention in `benchmark-results/README.md`: a unique report id
(`bench-YYYYMMDD-NNN`), a one-line purpose, machine-checkable data-source
provenance (raw data itself never enters git; repo fixtures are the
exception), and the full environment disclosure — when a run happens inside
WSL, the report also records the Windows host version, machine model, CPU,
logical processors, and physical RAM alongside the guest view. Summaries are
validated against `schemas/benchmark-summary.schema.json` in CI.

The first three-way (rust/python/r) baseline is
`benchmark-results/2026-09-13/` (`bench-20260913-001`): four capabilities
consistent at 1e-6 tolerance, rust ≈ 40 ms median vs python 300–330 ms
(speedup 7.5–8.25x) with ~85% memory saving on small fixtures. Dataset
provenance for real-data runs is tracked in
`docs/BENCHMARK_DATA_SOURCES.md`; implementation iterations (v1/v2
coexistence, algorithm changes) are logged in `docs/IMPLEMENTATION_LOG.md`.

## Source Policy

`GPTomics/bioSkills` is a primary method and example source.
`BioTender-max/awesome-bio-agent-skills` is a discovery index with per-source
license boundaries. Upstream bodies remain research inputs until provenance,
license, scientific correctness, and executable behavior have been reviewed.

The ignored `.research/` directory contains disposable source clones and must
not enter a release.

## Ethical Use

Maintainers publish a non-binding values statement in `ETHICAL_USE.md` — it
is not part of the AGPL-3.0-or-later license, adds no usage restrictions,
and does not affect the project's open-source status. The license alone
governs the code.

## License

Project-owned code, skills, GUI, SDK, worker, and network-facing services are
released under `AGPL-3.0-or-later`. Modified versions offered to users over a
network must provide the corresponding source as required by the AGPL.
Third-party components retain their own notices and terms; see
`THIRD_PARTY.md`.

---

## 简体中文

Linxira Bio SDK 是一个本地优先、面向 Agent 的生物信息学执行工具包。它将
简明的技能与稳定的 CLI、SDK 和工具契约结合，使常规分析使用经过测试的实现，
而不是为每次运行临时生成新脚本。

Windows 是主要的桌面与入门用户平台。Debian 与 Arch 是面向工作站、服务器和
HPC 场景受支持的 Linux 家族。macOS 目前不是经过测试或打包的目标。

在 Windows 上，WSL Debian 是较旧生物信息学组件的兼容性提供者，WSL Arch 是
当前平台的提供者，也是未来 Linxira WSL 的基础。在发布带版本的 rootfs 和升级
契约之前，Linxira WSL 安装仍处于规划状态。

本仓库是可执行生物信息学技能及其共享运行时的规范存放处。它不取代
`linxira-skills`，后者仍是面向研究、Linux、HPC、云、浏览器和交付工作流的
跨学科技能路由器与安装器。

### 产品形态

- `linxira-bio`：面向人员和工作流的命令行界面
- `skills/`：绑定到带版本能力上的 Agent 指令
- `capabilities/`：机器可读的能力目录
- `engine/`：Rust 运行时以及未来有基准数据支撑的 C++ 内核
- `skill-pack.json`：面向 Agent 运行时和 `linxira-skills` 的导入边界
- `linxira-bio-ui`：不依赖 WebView 的原生 Rust 桌面应用
- Python SDK：计划在 CLI 契约稳定之后提供
- `linxira-bio-mcp`：stdio JSON-RPC MCP 服务器（initialize / tools list /
  tools call / resources list+read；工具调用会执行 worker 作业）。将 MCP
  客户端指向该二进制即可；`capabilities/catalog.json` 中的能力 id 即工具名

### 当前能力

当前本地核心可以审计生物信息学前置条件，识别并预览常见生物学文件，计算
确定性的 FASTA、FASTQ、SAM、VCF、BED 交集、表达矩阵和 PDB 指标，并将结构化
结果表导出为 CSV、TSV、JSON、JSONL 或 XLSX：

```bash
cargo run -p linxira-bio-cli -- environment audit --json
cargo run -p linxira-bio-cli -- environment plan sequence-search --mode managed-user --json
cargo run -p linxira-bio-cli -- runtime catalog --json
cargo run -p linxira-bio-cli -- workflow packs --json
cargo run -p linxira-bio-cli -- dataset inspect tests/fixtures/data-inspection/variants.vcf --json
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
cargo run -p linxira-bio-cli -- fastq qc tests/fixtures/fastq-qc/valid.fastq --json
cargo run -p linxira-bio-cli -- alignment qc tests/fixtures/alignment-qc/valid.sam --json
cargo run -p linxira-bio-cli -- alignment bam-cram-qc sample.bam alignment-stats.tsv --json
cargo run -p linxira-bio-cli -- alignment coverage sample.bam coverage.tsv --json
cargo run -p linxira-bio-cli -- alignment short-read reference.fa reads.fastq aligned.bam --threads 4 --json
cargo run -p linxira-bio-cli -- variant stats tests/fixtures/variant-stats/mixed.vcf --json
cargo run -p linxira-bio-cli -- interval intersect tests/fixtures/interval-intersect/left.bed tests/fixtures/interval-intersect/right.bed --json
cargo run -p linxira-bio-cli -- expression matrix-qc tests/fixtures/expression-matrix/counts.tsv --json
cargo run -p linxira-bio-cli -- structure pdb tests/fixtures/structure-pdb-summary/alphafold-style.pdb --alphafold-plddt --json
cargo run -p linxira-bio-cli -- export table result.json result.xlsx
```

环境规划支持 `local-core`、`scripting`、`managed-runtimes`、`containers`、
`sequence-search`、`genomics-cli` 和 `full-local`。规划是只读的。安装仍是
独立的、需要显式批准的能力。可设置 `GITHUB_PROXY`，通过受信任的下载代理解析
规范的 GitHub 发布 URL。

规划模式为 `use-existing`、`managed-user`、`project-isolated` 和
`system-missing-only`。每个规划都包含 dry-run 事务边界；
`environment.apply.v1` 仍处于规划中，无法执行该预览。

查看运行时与能力目录：

```bash
cargo run -p linxira-bio-cli -- doctor --json
cargo run -p linxira-bio-cli -- capabilities --json
cargo run -p linxira-bio-worker -- tests/fixtures/jobs/sequence-stats.json
cargo run -p linxira-bio-worker -- tests/fixtures/jobs/dataset-inspect.json
cargo run -p linxira-bio-ui
cargo run -p linxira-bio-ui -- tests/fixtures/structure-pdb-summary/alphafold-style.pdb
```

原生 GUI 为 FASTA、FASTQ、SAM、BED 交集、表达矩阵、VCF 和 PDB 摘要提供能力
感知的结果图表。它将本地未压缩或 gzip 压缩的 PDB/mmCIF 坐标渲染为主链、球棍
或空间填充表示。结构文件保持本地，解压后上限为 128 MiB、100,000 个原子。
当前视图可以原子方式导出为 1600×1000 的 PNG。PDB 分析和可选的显式 AlphaFold
pLDDT 处理使用 `structure.pdb.summary.v1`；mmCIF 目前只是查看器输入，尚不是
可用的分析能力。

一方 Python 和 R 工作流脚本位于 `workflows/` 下，附带各自的 schema、测试、
锁文件和许可证声明。它们是本地可选后端：发布包不捆绑第三方解释器、软件包、
数据库或模型。`linxira-bio workflow run` 会在直接调用受批准的本地解释器之前
校验每个包。已编目的包不等于已安装的运行时，也不等于可用的分析能力。

发布包从 `packaging/bundle-manifest.json` 暂存，其中始终包含规范的双语
`docs/` 树、schema、目录、技能和许可证声明。仓库契约检查使用
`requirements-ci.txt` 中固定的 Draft 2020-12 验证器：

```bash
python -m venv .venv-ci
# 激活 .venv-ci，然后运行：
python -m pip install --requirement requirements-ci.txt
python scripts/validate-repository.py
python scripts/generate_third_party_notices.py --check-config
python -m unittest discover -s tests/python -p "test_*.py"
python scripts/stage-release.py --check
```

平台打包使用同一发布暂存脚本并传入编译后的二进制目录。暂存流程解析锁定的
目标平台发布依赖图，并生成 `THIRD_PARTY_DEPENDENCIES.json` 与
`THIRD_PARTY_DEPENDENCIES.txt`；缺失、含糊、过期或被修改的许可证文本都会使
发布失败。见 `docs/DEPENDENCY_NOTICES.md`。

### 执行模型

本地执行是默认方式。只有当实测的 CPU 时间、内存、GPU、数据库或存储需求超出
本地执行包络时，才将工作转移到本地 GPU、机构调度器或经批准的云计算。

纯浏览器服务是连接器，不是计算内核。它们要求显式的用户动作闸门、由人控制的
身份验证，并遵守服务条款。本项目从不存储或自动填充账户凭据。

产品边界、分阶段范围、支持的数据格式以及非 Visual Studio 构建方向见
`docs/ARCHITECTURE.md`、`docs/RUNTIME_MANAGEMENT.md`、`docs/AI_AND_SDK.md`、
`docs/DOCUMENTATION_POLICY.md` 及既有政策文档。读取、检查、分析和导出的
精确矩阵见 `docs/DATA_FORMATS.md`。

### 关于 Rust

这不是一个「用 Rust 重写一切」的项目，Rust 在这里也不是一种意识形态。引擎
选择 Rust 是因为它的设计语言——所有权、显式错误处理，以及作为共同纪律的
cargo/clippy——让开发与评审更从容：整类缺陷在程序运行之前就被捕获，确定性
且经 schema 校验的输出成为默认而非愿景。在开发与检查的信心足以回本的地方，
才会选择 Rust。

这条边界是刻意的。成熟的原生工具（salmon、kraken2、BLAST、HMMER、minimap2）
保持为外部工具并被编排，而不是重新实现。Python 和 R 是拥有独立实现的一等
后端，绘图默认使用 matplotlib 和 ggplot2。Rust 承担它擅长的部分；其余一切
保留原本就合适的工具。

### 绘图

绘图默认使用原生 Python（matplotlib）和 R（ggplot2）包（`plot.render.v1`）；
在采用成熟的 Rust 绘图栈之前，Rust SVG 可视化器继续作为其专属图表家族的
零依赖回退。图形是确定性的：相同的 PlotSpec 输入渲染为相同的字节，每个视觉
参数（坐标轴、主题、调色板、尺寸、dpi、格式、输出文件名）都是显式的。

### 基准测试

每一条有实测数据支撑的性能声明都存放在 `benchmark-results/` 下，并遵循
`benchmark-results/README.md` 的归档约定：唯一的报告 id
（`bench-YYYYMMDD-NNN`）、一行式目的说明、机器可校验的数据来源溯源（原始
数据本身永不进入 git；仓库内 fixtures 是例外），以及完整的环境披露——当运行
发生在 WSL 内时，报告还会在客户机视图之外记录 Windows 宿主版本、机型、CPU、
逻辑处理器数和物理内存。摘要会在 CI 中对照
`schemas/benchmark-summary.schema.json` 校验。

第一份三方（rust/python/r）基线是 `benchmark-results/2026-09-13/`
（`bench-20260913-001`）：四项能力在 1e-6 容差下一致，rust 中位数约 40 ms，
对比 python 300–330 ms（加速 7.5–8.25 倍），小 fixture 上内存节省约 85%。
真实数据运行的数据集溯源记录在 `docs/BENCHMARK_DATA_SOURCES.md`；实现迭代
（v1/v2 共存、算法变更）记录在 `docs/IMPLEMENTATION_LOG.md`。

### 来源政策

`GPTomics/bioSkills` 是主要的方法与示例来源。
`BioTender-max/awesome-bio-agent-skills` 是带逐来源许可证边界的发现索引。
上游内容在来源、许可证、科学正确性和可执行行为通过评审之前，始终只是研究
输入。

被忽略的 `.research/` 目录存放一次性来源克隆，绝不能进入发布包。

### 伦理使用

维护者在 `ETHICAL_USE.md` 中发布一份无约束力的价值观声明——它不是
AGPL-3.0-or-later 许可证的一部分，不增加任何使用限制，也不影响项目的开源
状态。代码仅由许可证本身约束。

### 许可证

项目自有代码、技能、GUI、SDK、worker 以及面向网络的服务以
`AGPL-3.0-or-later` 发布。通过网络向用户提供修改版本时，必须按 AGPL 要求
提供相应源代码。第三方组件保留其自身的声明与条款，见 `THIRD_PARTY.md`。
