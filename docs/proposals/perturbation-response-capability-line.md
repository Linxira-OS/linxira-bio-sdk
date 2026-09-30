# 细胞尺度扰动响应能力线立项草案（提案）

> 状态：提案，未写入 ROADMAP.md。本文供 ROADMAP §11 增补评审用——按仓库规则
> （§11 是封闭清单：先注册后实现），三行任务全部以任务级粒度注册，**禁止注册笼统
> "aivc.v1" 类大条目**。目标挂载位置：§11 通用批次表新增"🧬 细胞扰动响应批次"
> 小节（对标既有"⚡ GPU 前沿批次"格式）。

## 0. 现状锚点（全部已核实）

- **全仓库零命中**：LINCS / CMap / DoRothEA / PROGENy / State（Arc Institute）在
  capabilities/、skills/、docs/capabilities/ 均无任何条目——本线是空白新建，非改造。
- **MVP 链所需能力全部已实现**（2026-09-30 状态）：
  `chemistry.descriptors.v1`（RDKit pack）→ `chemistry.dock.v1`（AutoDock Vina，
  可选步骤）→ `similarity.reciprocal.v1`（BLAST/DIAMOND RBH）→
  `analyze-functional-enrichment`（custom ASSOCIATIONS.tsv ORA + preranked GSEA，
  SKILL.md 已含两模式命令）→ `plot.render.v1`（PlotSpec 渲染）。
- **数据桥**：`matrix.from-npz.v1` 是现成的矩阵导入桥；H5AD 目前仅魔数嗅探
  （engine/crates/linxira-bio-core/src/dataset.rs），DATA_FORMATS.md 标
  "Domain import planned"；`expression.single-cell.v1`（Scanpy+Seurat 双实现）
  排期 M4——本线**不依赖** H5AD 导入，查表输入用 TSV/CSV，与单细胞线解耦。

## 1. 拟新增 §11 行（任务级，四同步清单齐备后逐行实现）

### 行 1：扰动签名查表导入

| 能力 | 生态对标 | 实现 | 数据类别 | 绘图 | 状态 |
| --- | --- | --- | --- | --- | --- |
| 扰动签名查表导入（LINCS L1000 / CMap 公开子集 / DoRothEA / PROGENy → 归一化 ASSOCIATIONS.tsv） | eggNOG-mapper 输出归一化（annotation.eggnog.normalize.v1 同型） | Rust 编排+确定性输出（纯 Rust 解析/校验/归一） | 表(TSV/CSV/GMT) | — | 提案 |

- 输出契约：与 `analyze-functional-enrichment` 的 custom ASSOCIATIONS.tsv /
  GENE_SETS.tsv 格式**完全对齐**（基因×term 符号矩阵、GMT 两种），使查表产物零
  适配进入既有富集链。
- 四同步清单：catalog（`perturbation.signatures.import.v1`，category
  `perturbation`）+ schema（import summary：来源、版本、基因数、term 数、
  归一化规则、许可证字段）+ fixture（最小公开子集切片）+ skill
  `analyze-perturbation-signatures` + 双语文档。
- 确定性要求：同输入同版本输出字节级一致；来源元数据（URL、版本、下载日期、
  SHA-256）写入结果 JSON（对标 docs/BENCHMARK_DATA_SOURCES.md §3 必填字段）。

### 行 2：扰动响应虚拟预测（research-use-only）

| 能力 | 生态对标 | 实现 | 数据类别 | 绘图 | 状态 |
| --- | --- | --- | --- | --- | --- |
| 扰动响应虚拟预测（化合物 → 靶点/通路签名 → 富集 → 图，MVP 链编排） | CMap touchstone / GENMA（概念对照，不复现其商业数据） | Rust 编排+既有能力链（C5 档，无新内核） | 表+化学 | ✅富集条图/火山图 | 提案（research-use-only 前置标记） |

- **MVP 链**（全部复用既有能力，编排器为唯一新代码）：
  1. `chemistry.descriptors.v1`：化合物理化描述符；
  2. `chemistry.dock.v1`（可选）：候选靶点对接打分，筛选靶点候选；
  3. `similarity.reciprocal.v1`：模式物种靶点 → 人源直系同源 RBH 映射；
  4. 行 1 查表：靶点/通路签名查取；
  5. `analyze-functional-enrichment`：custom ORA + preranked GSEA（按 RBH 覆盖度
     加权解读）；
  6. `plot.render.v1`：确定性出图。
- 编排暴露：`perturbation.response.predict.v1`（category `perturbation`，
  default_execution `local-cpu`）；结果 JSON 顶层携带
  `research_use_only: true` 与逐级置信度字段。
- 四同步清单同行 1 格式；fixture 用合成小表（simulate-bio-data 生成）走通全链。

### 行 3：GPU 升级——State 开放权重包装

| 能力 | 生态对标 | 实现 | 数据类别 | 绘图 | 状态 |
| --- | --- | --- | --- | --- | --- |
| State（Arc Institute）细胞状态预测权重包装 | State（开放权重，许可证见 §3） | Py pack，`gpu:optional + minimum_vram_mb` 契约（沿 ⚡ 批次既定执行模式） | 表+化学 | ✅预测-查表对照图 | 提案（与行 1 交叉验证） |

- 定位：**与查表法交叉验证**，不替代 MVP 链；查表法（行 1/2）是确定性基线，
  State 推理结果与基线的差异本身是交付物。
- 遵守 ⚡ GPU 前沿批次既定约束：只包装开放权重、不写内核；权重不可再分发则走
  下载脚本 + 校验和（同 Borzoi/Evo 2 模式先例）。

## 2. Scope 声明（行 1/2/3 共用，写入能力文档 Inputs/Caveats 节）

1. **查表覆盖域**：仅限人类细胞系（LINCS L1000 所覆盖的细胞系/扰动组合）；
   输出结论只在查表覆盖域内有效。
2. **跨物种降级**：跨物种分析经 RBH 直系同源映射，置信度随直系同源覆盖度与
   一对一/一对多状态显式降级（结果 JSON 携带 per-gene 映射等级）。
3. **禁临床用途**：全部行标记 research-use-only；能力文档 Purpose/Caveats、
   skill 的 Validate And Interpret、结果 JSON 三处同时前置声明；不进入任何
   `medical.*` 命名空间。

## 3. 许可证核验清单（入库前置门槛；本表为核验计划，非结论）

| 数据/权重 | 预期许可（待核验） | 核验入口 | 可否再分发（初步判断） |
| --- | --- | --- | --- |
| LINCS L1000 公开矩阵（GSE92742 等） | 多为 CC0/公共领域，逐 dataset 核验 | GEO series 页 +.lin.sf.gov 使用条款 | 初步可再分发；逐集合登记许可字段后入库 |
| CMap / touchstone 衍生表 | **商业使用受限风险高** | Clue.io 条款页 | 仅用明确标注公开/CC0 的子集；touchstone 内部表不使用 |
| DoRothEA | 混合许可（按置信级/来源），主体 CC-BY 类 | DoRothEA 官方 GitHub + 许可文件 | 正则则文件内逐集标注；不可再分发集合改"下载脚本+校验和" |
| PROGENy | CC-BY 类（待核验版本） | saezlab 官方仓库 | 预期可再分发（附署名）；核验后定 |
| State 开放权重 | Apache/MIT 类（待核验，含微调数据声明） | Arc Institute 官方仓库 | 权重再分发与否按仓库 LICENSE 定；不可再分发走下载脚本 |

规则：**任何一项在许可字段未填前不进 catalog**；许可结论写入
docs/DEPENDENCY_NOTICES.md 与 pack NOTICE.md（既有先例：licenses/cargo-overrides
与第三方依赖清单流程）。

## 4. benchmark-datasets.json 登记项（对标 BENCHMARK_DATA_SOURCES.md §3）

该登记文件尚不存在（基准数据现状记录于 docs/BENCHMARK_DATA_SOURCES.md）；
本行随行 1 一并新建 `benchmark-datasets.json`（schema：id、dataset、source_url、
version、sha256、license_field、download_date、allowed_use），首批登记：

1. LINCS L1000 公开子集（GSE92742 挑选的细胞系切片，小样本起步）；
2. Virtual Cell Challenge 公开数据（若许可允许登记；作为 State 行的对照集）。

登记格式沿用 docs/BENCHMARK_DATA_SOURCES.md 既有必填字段（§3 "数据来源引用格式"），
不新造口径。

## 5. 明确不做（scope 外）

- 不注册 "aivc.v1" 或任何细胞尺度大而全条目；一切拆任务级行。
- 不做训练/微调（State 只推理包装）；不承诺单细胞输入（H5AD 导入属 M4 单细胞线，
  本线 TSV/CSV 起步）。
- 不复现 CMap touchstone 算法与数据；不使用不可再分发权重（沿 ⚡ 批次永久排除表
  的判定方式）。

## 6. 顺序与依赖

行 1（纯 Rust 查表导入，无外部依赖）→ 行 2（编排链，依赖行 1）→ 行 3（State pack，
依赖行 1 做交叉验证基线 + GPU 契约首发先例 cellpose 行就绪）。行 1 可随下一常规批
次首发；行 2/3 评审后各自排期。
