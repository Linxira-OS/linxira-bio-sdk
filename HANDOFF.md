# Linxira Bio SDK 交接说明

生成时间：2026-08-04
分支：`main`；已验证的实现改动已提交为 `6d16b77 feat: complete verified local bio capability suite`（136 个文件），**未推送**。本文档后续补充内容将单独再提交一次。

## 项目目标

### 使命

构建一个 **本地优先（local-first）、可被 Agent 直接调用的生物信息学 SDK**。发布的能力必须让用户和 Agent 直接用现成实现完成常规分析，而不是每次重新写脚本。

每个发布能力必须满足：

- 版本化能力 ID、稳定的命令/SDK 契约和结构化输出；
- 确定性实现，或锁定并校验的维护后端；
- 代表性本地夹具和错误用例；
- 等价性或 golden-result 测试；
- Windows GNU 验证（不使用 Visual Studio/MSVC）以及 Debian/Arch Linux 验证；
- 可追溯的 provenance、许可、数据治理与执行模式元数据；
- 简洁、不夸大生物学结论的 Agent 技能。

### 产品边界

本仓库拥有“可执行的生物信息学叶子能力 + 调用这些能力的技能”，通过同一套版本化能力契约服务四类使用者：

1. 研究者：使用 `linxira-bio` CLI。
2. 工作流：使用 v1/v2 结构化 job 请求与结果。
3. Python 程序：使用规划中的原生绑定（初期为子进程客户端）。
4. Agent：使用 `skill-pack.json` 和规划中的 MCP server。

`linxira-skills` 仍是跨学科的通用技能路由与安装控制面；本仓库只发布可执行技能体，不复制通用路由/安装器。

### 执行模型

- 默认本地执行；只有实测 CPU、内存、GPU、数据库或存储超出本地包络时，才转向本地 GPU、机构调度器或经批准的云端。
- 浏览器服务是“受门控的连接器”，不自动填写密码、MFA、验证码，不接受服务条款。
- AlphaFold 2 可本地部署时走本地；AlphaFold 3 等需要账号/浏览器的场景走受控云端连接器，均尚未实现。
- 安装、文件系统越界写入、云端成本、数据上传和认证浏览器使用必须先经用户明确批准。

### 平台与运行时可执行性

- Windows 是首要桌面与新手平台；Debian 与 Arch 是支持的 Linux 家族；macOS 当前不测试、不打包。
- Windows 上 WSL Debian 作为旧组件兼容提供者，WSL Arch 作为当前平台提供者与未来 Linxira WSL 基础；Linxira WSL 安装仍为 planned。
- 桌面 GUI 用 Rust + egui，原生渲染，不引入 WebView；Java 只作为托管分析运行时，不做 UI。
- Python/R/Java 环境默认用户隔离或项目隔离，不修改全局 PATH、JAVA_HOME 或默认解释器；R 默认跟随当前稳定版，允许多版本并存。
- 项目代码使用 AGPL-3.0-or-later，第三方组件保留各自许可证与声明。

### 非目标

- 不因“某个技能被索引”就宣称完整覆盖。
- 不重写 samtools、bcftools、bedtools、minimap2、Salmon、Kraken2、MMseqs2、Foldseek 等成熟工具，除非有基准证明 Rust/C++ 替代合理。
- 不做临床决策、不自动发表文章、不隐式开通付费资源。
- 不要求 GUI 才能使用 CLI/SDK/Worker/Agent；不把浏览器运行时作为 GUI 实现。

## 仓库内容总览

### 根目录关键文件

| 文件 | 作用 |
| --- | --- |
| `README.md` | 产品入口：目标、产品面、当前能力、执行模型、许可 |
| `AGENTS.md` | Agent 路由规则、能力索引、仓库规则与验证命令 |
| `HANDOFF.md` | 本交接文档 |
| `Cargo.toml` / `Cargo.lock` | Rust workspace（edition 2024，rust-version 1.92） |
| `capabilities/` | 公开能力注册表与 v1 覆盖清单 |
| `schemas/` | job/result/artifact/manifest 等机器契约（Draft 2020-12） |
| `skills/` | 28 个 Agent 技能，绑定版本化能力 |
| `skill-pack.json` | 供 `linxira-skills` 和其他 Agent 运行时导入的技能包边界 |
| `workflows/` | 第一方 Python/R 工作流 pack、清单、锁、通知、测试 |
| `runtimes/` | 托管运行时提供者注册表（Python/R/Java/Conda 等） |
| `tools/` | 环境审计用的外部工具注册表 |
| `profiles/` | 环境规划 profile（当前 `local-core.json`） |
| `packaging/` | 发布 bundle 清单 |
| `scripts/` | 仓库校验、覆盖校验、第三方通知、release staging、Windows GNU 测试 |
| `tests/` | Rust/Python 测试、job 夹具、能力结果夹具 |
| `.github/workflows/ci.yml` | Windows GNU / Debian / Arch 门禁 |
| `deny.toml` | Cargo 依赖许可证门禁 |
| `THIRD_PARTY.md` / `licenses/` | 第三方许可证与声明 |

忽略且不进入发布的目录：`.research/`（上游源码克隆）、`.tools/`、`.linxira/`、`temp/`、`target/`、`.venv-ci/`。

### Rust 工作区

| crate | 职责 |
| --- | --- |
| `engine/crates/linxira-bio-core` | 确定性算法内核：序列、FASTQ、SAM、区间、注释、变异、表达、富集、结构、原生工具包装 |
| `engine/crates/linxira-bio-worker` | 执行 v1/v2 job 契约：输入校验、哈希、工件产物、工作流 runner |
| `engine/crates/linxira-bio-cli` | `linxira-bio` 命令行入口 |
| `engine/crates/linxira-bio-protocol` | 协议与 manifest 类型 |
| `engine/crates/linxira-bio-export` | JSON/CSV/TSV/JSONL/XLSX 导出 |
| `apps/linxira-bio-ui` | 原生 egui 桌面应用（本地项目、分析、图表、结构 3D 查看器、文档） |

### 文档

`docs/` 是权威离线文档来源，GUI、CLI、AI 检索和未来网站共用同一份 `docs/capabilities/<capability>/en-US.md + zh-CN.md`。核心设计文档包括：

- `PROJECT_CHARTER.md`：使命、产品边界、非目标、发布门禁。
- `ARCHITECTURE.md`：产品定义、仓库边界、桌面布局、平台兼容性。
- `AI_AND_SDK.md`：能力契约、内置助手、Python SDK/MCP 路线、隐私与审批。
- `RUNTIME_MANAGEMENT.md`：运行时隔离、provider、事务模型、workflow 调度。
- `WORKFLOW_PACKS.md`：workflow pack 信任级别与 installable 门禁。
- `CAPABILITY_ROADMAP.md`：能力分类、发布计划（0.1–0.4）。
- `DATA_FORMATS.md`：读取、识别、分析、导出矩阵。
- `EXECUTION_POLICY.md`、`TOOLCHAIN.md`、`DEPENDENCY_NOTICES.md`、`DOCUMENTATION_POLICY.md`：执行边界、工具链、依赖通知、文档规范。

## 三、总体状态

- 能力目录：95 个能力，其中 90 个 `available`、1 个 `experimental`（`system.gui.v1`）、4 个 `planned`。
- v1 覆盖目标：80/80 达成，实际已实现 84 项；清单共 100 项，其中 80 项为目标项、16 项为 `planned-with-owner`、4 项为外部工具/工作流包装。
- 本地全部门禁通过：`cargo fmt`、`cargo clippy -D warnings`、`cargo test --workspace`、CLI 冒烟、仓库 Schema 校验、覆盖清单校验、工作流 manifest 测试、9 个改动技能目录的 skill-creator 校验。
- 实现改动已提交（`6d16b77`），未推送；等待验收后再推送。

## 四、已完成

### 本地 Rust 分析内核

- 序列：FASTA 统计、提取、过滤、反向互补、翻译、ORF、ID 归一化、合并/拆分、表格互转、k-mer 计数、ePCR。
- FASTQ：QC、质量裁剪、接头去除、精确去重（可带严格 UMI）。
- 比对：SAM 文本 QC；BAM/CRAM QC、覆盖度、BAM 转 BigWig、短读比对（包装外部原生工具）。
- 区间：BED 相交、合并、扣除、最近区间（确定性最近特征查找）。
- 注释：GFF3/GTF 统计、归一化、基因位置、基因密度、参考基因组序列提取。
- 变异：VCF 统计、过滤、左对齐归一化、变异集比较。
- 表达：矩阵 QC、CPM/log2-CPM/median-ratio 归一化、PCA、聚类、热图；火山图。
- 富集：GO/eggNOG 注释归一化、custom/GO/KEGG 超几何富集、预排序 GSEA（固定种子置换）。
- 蛋白/结构：蛋白理化性质、InterProScan/HMMER 域解析、PDB/mmCIF 摘要、结构序列、接触图、几何、叠合、二级结构、AlphaFold pLDDT 显式解读、本地 PDB/mmCIF 3D 查看器。
- 其他：Venn/UpSet、Newick 归一化/重定根、CSV/TSV 表格处理与导出、SVG 可视化。

### 外部原生工具包装

- BLAST+、DIAMOND、HMMER、MUSCLE、trimAl、MEME、IQ-TREE、MCScanX、KaKs Calculator、samtools 等通过受控参数构造调用，不经过 shell；不捆绑、不自动下载外部可执行文件。
- 已登记 MCScanX 与 KaKs Calculator 环境探测，支持 `PATH` 和 `LINXIRA_BIO_*` 覆盖变量。

### R 工作流

- `org.linxira.bulk-expression-deseq2` 已 cataloged，服务 `expression.differential.v1`，兼容别名 `medical.bulk-rnaseq.v1`、`expression.deseq2.v1`。
- R 默认跟踪当前稳定版 4.6.1；解释器与项目包库分开选择（`LINXIRA_BIO_WORKFLOW_R`、`LINXIRA_BIO_WORKFLOW_R_LIBRARY`），不修改全局 R、全局包库或系统 PATH。
- 医学入口仅限科研用途（`clinical_use: false`），不做诊断或治疗建议。
- Workflow 请求层、Rscript 桩集成测试已通过。

### 环境审计与规划

- 支持 Python、R、Java、Conda/Bioconda、BLAST、DIAMOND、原生工具、WSL Debian/Arch、Docker、Podman、GPU 审计。
- Windows 允许 WSL 或 Docker 之一；Linux 同时检查 Docker 与 Podman。
- 支持“仅使用现有 / 用户隔离 / 项目隔离 / 系统缺失项”等模式；安装类动作仍由 `environment.apply.v1` 门控，目前为 planned。

### UI / 文档 / 许可

- egui 桌面 UI 含本地项目、导入、分析、结果图表、结构 3D 查看器、双语文档与依赖许可页；UI 测试 54/54 通过。
- 新增能力均已接入 UI 路由、参数面板、中英文档和结果字段翻译。
- 仓库协议 AGPL-3.0-or-later；`THIRD_PARTY.md`、依赖通知和许可证检查已就绪。
- 追踪内容中已确认不出现 `tbtools`/`tbtool` 名称（`git grep` 无命中）；相关原始安装包只放在未追踪的 `temp/` 下。

## 五、本批新增/接通的能力

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| `fastq.deduplicate.v1` | available | 精确序列去重；可选读名后缀 UMI 或序列前缀 UMI，严格匹配、不纠错 |
| `interval.closest.v1` | available | 每个查询区间输出一个确定性最近目标 TSV；同链距离，等距取最小坐标 |
| `enrichment.gsea.v1` | available | 预排序加权 GSEA；固定种子基因标签置换 + BH FDR |
| `variant.compare.v1` | available | 拆分多等位、去重、稳定顺序比较，不冒充基因型一致性 |

以上均已接入 CLI、Worker V1/V2、UI、能力目录、覆盖清单、测试夹具和中英文档。

## 六、验证结果

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过（CLI 33、core 174、export 12、protocol 8、UI 54、worker lib 18、worker 集成 34）。
- CLI 冒烟：`linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json` 通过。
- `scripts/validate-repository.py`：44 个 Schema、159 个实例、95 能力、28 技能、29 工具、6 运行时、覆盖 84/80。
- `scripts/verify_coverage_v1.py`：100 项清单、80 目标、84 已实现、57 条离线参考事实。
- `tests/python/test_coverage_v1.py`：16/16 通过（已把过时的“恰好 80”断言改为“不少于 80”）。
- `workflows/tests/test_pack_manifests.py`：2/2 通过。
- 9 个改动技能目录通过 skill-creator `quick_validate.py`（需 UTF-8 模式）。

## 七、未解决 / 待办

### 产品化缺口（planned）

- `environment.apply.v1`：托管安装、校验和、原子激活、回滚尚未实现，因此工作流包还不能由产品安装/调度。
- `sequence.convert.biopython.v1`：pack 已 cataloged，但未 installable/dispatchable。
- `protein.af2.predict.v1`（本地 AlphaFold 2）与 `protein.af3.server.v1`（云端 AlphaFold 3）：仅接口规划，未实现；云端/浏览器自动化需另行审批。

### 数据与算法验证

- DESeq2 真实算法端到端未运行：项目隔离 R 包库尚未安装 `DESeq2`、`jsonlite`、`digest`；完整传递依赖锁仍待解析，`installable` 保持 false，未虚报。
- R 脚本层校验测试未在本次交接时重跑（Workflow 请求层与 Rscript 桩测试已通过）。

### 剩余 16 项 planned-with-owner

- `alignment.long-read`：长读参考比对工作流。
- `variant.annotate`：功能变异注释。
- `expression.wgcna`：加权共表达网络。
- `similarity.blast.remote`、`ncbi.sequence.fetch`：需批准的远程连接器。
- `motif.mast`：MAST 基序搜索。
- `phylogeny.tree.visualize`：系统发育树可视化。
- `comparative.dotplot`：基因组点图。
- `rna.secondary-structure`：RNA 二级结构折叠与绘图。
- `chemistry.descriptors`：小分子理化描述符。
- `protein.alphafold-connectors`：AlphaFold 2/3 批准连接器。
- `medical.microbiome`、`medical.metabolomics`、`medical.pharmacogenomics`、`medical.survival`、`medical.spatial-transcriptomics`：科研用途医学分析。

### 平台与发布

- Linux 实机验证未完成：Windows GNU 本地已验证；Debian/Arch 仅有 CI 配置，未在交接前实际跑通。
- 远程 CI 未确认：当前改动未推送，远端 CI 反映的是旧提交。
- 打包/发布未做：release staging、Windows 安装包、deb/Arch 包尚未产出。
- UI 视觉打磨仍待进行（功能已接通，界面样式属于后续迭代）。
- Python/Java 运行时只是 cataloged，尚未随产品捆绑安装。
- C++ 尚未启用：按仓库规则，只有 Rust 或现有原生依赖不满足并经基准证明后才引入 C++。

## 八、Git 与工作区状态

- 已验证的实现改动已提交为 `6d16b77`（136 个文件，约 1 万行新增），**未推送**；`main` 领先 `origin/main` 多个本地提交（含此前的 `cd83599`）。
- 本次交接文档的“仓库内容 / 项目目标”补充将作为下一个 docs 提交。
- `temp/` 中的外部安装包/源码克隆保持未追踪，不进入仓库。
- 没有清理 Git 历史，也没有强制推送；历史与提交都保留，等待验收。

## 九、接续建议

1. 验收当前工作区后，提交并推送 `main`，随后查看远端 CI（Windows GNU、Debian、Arch）。
2. 在 WSL Debian 实跑一次同一套门禁；Arch 交给 CI。
3. 在项目隔离 R 库安装 DESeq2 依赖，跑真实 DESeq2 E2E，再更新依赖锁并置为 installable。
4. 下一批优先做纯本地、无专有模型的 planned 项：MAST、RNA 二级结构、树可视化、比较点图、WGCNA、变异注释、医学科研统计。
5. 云侧（AlphaFold 2/3、远程 BLAST、NCBI、浏览器自动化）单独规划，并保持“需明确审批”的接线。

## 附录：关键文档索引

- 产品与边界：[README.md](README.md)、[docs/PROJECT_CHARTER.md](docs/PROJECT_CHARTER.md)
- 架构与执行：[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)、[docs/EXECUTION_POLICY.md](docs/EXECUTION_POLICY.md)
- Agent/SDK：[docs/AI_AND_SDK.md](docs/AI_AND_SDK.md)、[skill-pack.json](skill-pack.json)
- 运行时：[docs/RUNTIME_MANAGEMENT.md](docs/RUNTIME_MANAGEMENT.md)、[runtimes/catalog.json](runtimes/catalog.json)
- 工作流：[docs/WORKFLOW_PACKS.md](docs/WORKFLOW_PACKS.md)、[workflows/catalog.json](workflows/catalog.json)
- 路线图：[docs/CAPABILITY_ROADMAP.md](docs/CAPABILITY_ROADMAP.md)、[capabilities/coverage-v1.json](capabilities/coverage-v1.json)
- 格式与工具链：[docs/DATA_FORMATS.md](docs/DATA_FORMATS.md)、[docs/TOOLCHAIN.md](docs/TOOLCHAIN.md)
- 许可与依赖：[LICENSE](LICENSE)、[THIRD_PARTY.md](THIRD_PARTY.md)、[docs/DEPENDENCY_NOTICES.md](docs/DEPENDENCY_NOTICES.md)
