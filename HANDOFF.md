# Linxira Bio SDK 交接说明

生成时间：2026-08-04
分支：`main`，本地领先 `origin/main` 一个提交（`cd83599 fix: make workflow runtime portable`），当前工作区还有大量未提交改动，**未推送**。

## 一、总体状态

- 能力目录：95 个能力，其中 90 个 `available`、1 个 `experimental`（`system.gui.v1`）、4 个 `planned`。
- v1 覆盖目标：80/80 达成，实际已实现 84 项；清单共 100 项，其中 80 项为目标项、16 项为 `planned-with-owner`、4 项为外部工具/工作流包装。
- 本地全部门禁通过：`cargo fmt`、`cargo clippy -D warnings`、`cargo test --workspace`、CLI 冒烟、仓库 Schema 校验、覆盖清单校验、工作流 manifest 测试、9 个改动技能目录的 skill-creator 校验。
- 未提交、未推送，等待验收后再提交。

## 二、已完成

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

## 三、本批新增/接通的能力

| 能力 | 状态 | 说明 |
| --- | --- | --- |
| `fastq.deduplicate.v1` | available | 精确序列去重；可选读名后缀 UMI 或序列前缀 UMI，严格匹配、不纠错 |
| `interval.closest.v1` | available | 每个查询区间输出一个确定性最近目标 TSV；同链距离，等距取最小坐标 |
| `enrichment.gsea.v1` | available | 预排序加权 GSEA；固定种子基因标签置换 + BH FDR |
| `variant.compare.v1` | available | 拆分多等位、去重、稳定顺序比较，不冒充基因型一致性 |

以上均已接入 CLI、Worker V1/V2、UI、能力目录、覆盖清单、测试夹具和中英文档。

## 四、验证结果

- `cargo fmt --all -- --check`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo test --workspace`：通过（CLI 33、core 174、export 12、protocol 8、UI 54、worker lib 18、worker 集成 34）。
- CLI 冒烟：`linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json` 通过。
- `scripts/validate-repository.py`：44 个 Schema、159 个实例、95 能力、28 技能、29 工具、6 运行时、覆盖 84/80。
- `scripts/verify_coverage_v1.py`：100 项清单、80 目标、84 已实现、57 条离线参考事实。
- `tests/python/test_coverage_v1.py`：16/16 通过（已把过时的“恰好 80”断言改为“不少于 80”）。
- `workflows/tests/test_pack_manifests.py`：2/2 通过。
- 9 个改动技能目录通过 skill-creator `quick_validate.py`（需 UTF-8 模式）。

## 五、未解决 / 待办

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

## 六、Git 与工作区状态

- `main` 领先远端 1 个提交；工作区 56 个文件有改动，另有大量新增文件（新能力文档、夹具、技能目录、Schema）。
- `temp/` 中的外部安装包/源码克隆保持未追踪，不进入仓库。
- 没有清理 Git 历史，也没有强制推送；历史与提交都保留，等待验收。

## 七、接续建议

1. 验收当前工作区后，提交并推送 `main`，随后查看远端 CI（Windows GNU、Debian、Arch）。
2. 在 WSL Debian 实跑一次同一套门禁；Arch 交给 CI。
3. 在项目隔离 R 库安装 DESeq2 依赖，跑真实 DESeq2 E2E，再更新依赖锁并置为 installable。
4. 下一批优先做纯本地、无专有模型的 planned 项：MAST、RNA 二级结构、树可视化、比较点图、WGCNA、变异注释、医学科研统计。
5. 云侧（AlphaFold 2/3、远程 BLAST、NCBI、浏览器自动化）单独规划，并保持“需明确审批”的接线。
