# 生信图表覆盖矩阵（Figure Coverage Matrix）

> 对照来源：博客「生信图表大全」系列七篇（第一至六篇共 195 张编号图：173 模拟
> + 22 真实公开数据；第七篇为 Nature 风格重绘对照，不新增图型）。
> 口径：**图型级**映射（一个图型可能对应多张图；小节含双图的按图型归并）。
> Source: the seven-post chart compendium; type-level mapping against
> `capabilities/catalog.json` (113 available).

状态图例：✅ 现在就能端到端分析（数据读入 + 统计 + 结果表/原生可视化）；
🟡 数据侧已可分析、出图等 M1 绘图 pack（matplotlib/ggplot2）；❌ 能力缺口
（列为主要补齐候选）；🚫 范围外（湿实验仪器图像/照片，SDK 不做图像分析）。

## 总览（By the numbers）

| 状态 | 图型数 | 占比 | 说明 |
|---|---|---|---|
| ✅ 可分析 | 58 | ~30% | 火山/热图/韦恩/UpSet/PCA/聚类/富集/生存/共线性/结构全套等 |
| 🟡 数据可分析 | 33 | ~17% | 箱线/小提琴/散点回归/碎石/Circos/MSA 保守性等，出图等 M1 |
| ❌ 缺口 | 88 | ~45% | 集中在群体遗传统计、表观、单细胞深度、MD、酶动力学、ML 评估 |
| 🚫 范围外 | 12 | ~6% | WB/IHC/IF/TUNEL/流式/Sanger 峰图/凝胶照片等仪器图像 |
| 合计 | 191 | 100% | （按图型归并，覆盖 195 张编号图） |

## 逐篇映射

### 第一篇（图 1–41：差异表达、降维聚类、富集、建模、湿实验）

| 图型 | 图号 | 能力 / 路径 | 状态 |
|---|---|---|---|
| 火山图 | 1 | `expression.differential.v1` + `expression.volcano.v1` | ✅ |
| 表达热图 / 级联热图 | 2, 40 | `expression.heatmap.v1` | ✅ |
| 韦恩图（样本间 / 直系同源） | 8, 9 | `set.venn.v1` | ✅ |
| UpSet 图 | 10 | `set.upset.v1` | ✅ |
| PCA 得分图 / 碎石图 | 12 | `expression.pca.v1` | ✅ |
| 层次聚类树 + 热图 | 14 | `expression.cluster.v1` + heatmap | ✅ |
| KEGG 气泡 / 柱形 | 15, 16 | `enrichment.kegg.v1` + `enrichment.visualize.v1` | ✅ |
| GSEA | 18 | `enrichment.gsea.v1` | ✅ |
| KM 生存 / Cox 森林数据 | 20, 22 | `medical.survival.v1`（曲线绘制待 M1） | ✅ |
| 引物扩增子锚定 | 26 | `primer.epcr.v1` + `sequence.extract.v1` | ✅ |
| 结构锚点 + 保守残基 | 35 | `structure.viewer.v1` + `structure.contact-map.v1` | ✅ |
| 启动子顺式元件扫描 | 38 | `motif.mast.v1` + `annotation.sequence.extract.v1` | ✅ |
| 相关热图 | 3 | 矩阵组合 + `expression.heatmap.v1` | 🟡 |
| Pearson 散点 / P-S 对比 | 4, 5 | table + 统计 | 🟡 |
| 箱线 / 小提琴 | 6, 7 | 数据 ✅，图待 M1 | 🟡 |
| GO 弦图 | 17 | `enrichment.visualize.v1`（bar/dot/network 有，chord 无） | 🟡 |
| 证据矩阵 / 组织特异性 / 通路动态 | 36, 37, 39 | table 组合 + heatmap | 🟡 |
| TPM 定量与文库分布 | 11 | `expression.quantify.v1`（planned，M4） | ❌ |
| UMAP | 13 | `expression.single-cell.v1`（planned，M4） | ❌ |
| ROC / DCA | 19, 21 | 诊断建模评估缺 | ❌ |
| ΔΔCt / qPCR 扩增 / 熔解 | 23–25 | qPCR 定量计算缺 | ❌ |
| 桑基图 / 脊线图 | 34, 41 | 通用绘图缺 | ❌ |
| WB / IHC / IF / TUNEL / 流式×3 | 27–33 | 仪器图像 | 🚫 |

### 第二篇（图 42–74：群体遗传、比较基因组、微生物组、单细胞、ML、育种）

| 图型 | 图号 | 能力 / 路径 | 状态 |
|---|---|---|---|
| 系统发育树 | 46 | `phylogeny.iqtree.v1` + `phylogeny.tree.visualize.v1` | ✅ |
| 共线性点阵图 | 48 | `comparative.dotplot.v1` | ✅ |
| Ka/Ks 分布 | 49 | `comparative.kaks.v1` | ✅ |
| 染色体基因定位 | 50 | `annotation.gene-position.v1` | ✅ |
| 基因密度 | 52 | `genome.gene-density.v1` | ✅ |
| Motif logo | 53 | `motif.meme.v1` + `motif.visualize.v1` | ✅ |
| Alpha 多样性 | 57 | `medical.microbiome.v1` | ✅ |
| WGCNA 模块-性状 | 62 | `expression.wgcna.v1` | ✅ |
| PCA 碎石图 | 69 | `expression.pca.v1` | ✅ |
| Circos 圈图 | 51 | `comparative.synteny.visualize.v1`（圈图风格待补） | 🟡 |
| 基因结构图 | 54 | `annotation.gxf.*` 位置表数据 ✅，图待 M1 | 🟡 |
| MSA 保守性 | 55 | `msa.muscle.v1` + `msa.trimal.v1` 数据 ✅ | 🟡 |
| 物种丰度堆叠柱 | 58 | `metagenomics.classify.v1` 数据 ✅ | 🟡 |
| 生长曲线 | 66 | table + 拟合缺 | 🟡 |
| 曼哈顿 / QQ / LD 衰减 / ADMIXTURE / π-Fst 滑窗 | 42–45, 47 | 群体遗传统计缺（GWAS 栈，二批候选） | ❌ |
| 拟时序 / 单细胞点图 | 59, 60 | single-cell 深度分析缺 | ❌ |
| PPI / 共表达网络 | 61 | 网络分析缺 | ❌ |
| IC50 / 米氏动力学 | 63, 64 | 剂量-酶动力学拟合缺 | ❌ |
| PR 曲线 / 混淆矩阵 / 随机森林 | 67, 68, 70 | ML 评估缺 | ❌ |
| 多性状雷达图 | 71 | 通用绘图缺 | ❌ |
| 琼脂糖凝胶示意 | 65 | 湿实验图像 | 🚫 |

### 第三篇（图 75–103：测序 QC、变异、GWAS 定位、表观、单细胞通讯）

| 图型 | 图号 | 能力 / 路径 | 状态 |
|---|---|---|---|
| 逐循环碱基质量 | 72 | `fastq.qc.v1`（per-cycle 统计） | ✅ |
| GC 偏倚 / 覆盖深度 | 73 | `fastq.qc.v1` + `alignment.coverage.v1` | ✅ |
| pileup 变异位点 | 76 | `variant.*` + `alignment.coverage.v1` | ✅ |
| WGCNA 全装图 | 92 | `expression.wgcna.v1` | ✅ |
| sashimi 剪接图 | 75 | coverage ✅，junction 计数缺 | 🟡 |
| 突变频谱 | 77 | `variant.stats.v1`（spectrum 细分待补） | 🟡 |
| PCA + ADMIXTURE 双联 | 83 | PCA ✅ / ADMIXTURE ❌ | 🟡 |
| GO DAG | 89 | `enrichment.visualize.v1`（DAG 绘制缺） | 🟡 |
| 空间 spot 特征 | 100 | `medical.spatial-transcriptomics.v1`（summary ✅，spot 图缺） | 🟡 |
| SFS / Tajima's D / 单倍型 / GRM / 选择三联 | 78–82 | 群体遗传统计缺 | ❌ |
| LocusZoom / PIP | 84, 85 | 精细定位缺 | ❌ |
| QTL LOD / 连锁图 / AMMI | 86–88 | 数量遗传缺 | ❌ |
| ChIP 剖面 / TF 足迹 / 甲基化 / Hi-C | 90, 91, 93, 94 | `epigenetics.*`（planned，M4） | ❌ |
| velocity / 组成柱 / marker 热图 / 通讯圈 / L-R 气泡 | 95–99 | single-cell 深度缺 | ❌ |
| Sanger 峰图 | 74 | 仪器数据（chromatogram） | 🚫 |

### 第四篇（图 104–133：蛋白互作、生化曲线、微生物生态、AI 多组学）

| 图型 | 图号 | 能力 / 路径 | 状态 |
|---|---|---|---|
| 结构域架构图 | 104 | `protein.domain.parse.v1` + `protein.domain.visualize.v1` | ✅ |
| AlphaFold pLDDT 置信度 | 105 | `structure.pdb.summary.v1` | ✅ |
| 镜像质谱图 | 118 | `medical.metabolomics.v1`（峰检测 ✅，镜像图待 M1） | 🟡 |
| 瀑布图 | 128 | `variant.compare.v1` 数据 ✅，图待 M1 | 🟡 |
| 三维 PCA | 130 | `expression.pca.v1` 数据 ✅，3D 图缺 | 🟡 |
| RMSF / 自由能 landscape / RMSD | 103, 104 区 | 分子动力学缺（MD 栈，远期） | ❌ |
| Lineweaver-Burk / DSF / ELISA / 荧光光谱 / 荧光素酶 | 107–111 | 生化曲线拟合缺 | ❌ |
| 稀疏化 / rank-abundance / 三元相图 / RDA / LEfSe / 杀菌曲线 | 112–117 | 生态统计缺（部分二批候选） | ❌ |
| Van Krevelen / Blob 图 | 119, 120 | 代谢组深度 / bin 质控缺 | ❌ |
| MOFA / 归因 / PLM UMAP / 泛基因组 / 跨组学 UMAP | 121–125 | AI 多组学前沿缺 | ❌ |
| 漏斗 / 哑铃 / 帕累托 | 126, 127, 129 | 通用统计图缺（M1 后补） | ❌ |
| EMSA | 106 | 湿实验图像 | 🚫 |

### 第五篇（图 134–173：蛋白结构可视化、突变扫描、表达验证）

| 图型 | 图号 | 能力 / 路径 | 状态 |
|---|---|---|---|
| C-alpha / 线框 / 棍棒 / 球状 / 缎带 | 134–138 | `structure.viewer.v1`（交互查看器） | ✅ |
| 交互 cartoon / WT-mut 对比 / pLDDT 上色 / TF-DNA | 139–142 | viewer + `structure.superpose.v1` + pdb summary | ✅ |
| 残基接触图 / 界面频率热图 / 接触数曲线 | 144, 154, 166 | `structure.contact-map.v1` | ✅ |
| 结构叠合 | 145 | `structure.superpose.v1` | ✅ |
| 二级结构轨道 | 149 | `protein.secondary-structure.v1` | ✅ |
| 拉氏图 | 164 | `structure.geometry.v1`（torsion 测量） | ✅ |
| 逐残基 B 因子曲线 | 165 | `structure.pdb.summary.v1` | ✅ |
| 组织 / 野栽表达热图 | 158, 159 | `expression.heatmap.v1` | ✅ |
| 突变棒棒糖图 | 146 | `variant.*` 数据 ✅，图待 M1 | 🟡 |
| 保守性剖面 / SASA | 152, 155 | msa+consensus 数据 ✅ / SASA 计算缺 | 🟡 |
| 表达箱线 / 基因型×环境 / 灰度-qPCR 相关 / 表达-表型回归 | 160–163 | table + 统计组合 | 🟡 |
| 循环数线性检验 | 172 | table 组合 | 🟡 |
| PAE / 容忍度矩阵 / ΔΔG / 口袋定位 / 口袋体积 / 对接 | 143, 147, 148, 150–153 | 结构深度（PAE 解析、口袋学、对接）缺 | ❌ |
| qPCR 标曲 / 熔解峰 | 156, 171 | qPCR 定量缺 | ❌ |
| RMSD/Rg / 氢键 / 2D 互作 / 共进化 | 167–170 | MD / 共进化缺 | ❌ |
| 半定量凝胶 / WB 灰度 | 157, 173 | 仪器图像 | 🚫 |

### 第六篇（图 174–195：真实公开数据 22 张）

| 图型 | 图号 | 能力 / 路径 | 状态 |
|---|---|---|---|
| 基因型 PCA（真实 + 对照） | 174, 192 | `expression.pca.v1` | ✅ |
| 组织表达热图（真实 + 对照） | 183, 193 | `alignment.bam-to-bigwig.v1`/coverage + heatmap | ✅ |
| accession 表达 PCA | 184 | `expression.pca.v1` | ✅ |
| 基因模型统计 | 186 | `annotation.gxf.stats.v1` | ✅ |
| BRI1–SERK1 3D 交互 / 界面接触 / B 因子 | 188–190 | viewer + contact-map + pdb summary | ✅ |
| SNP 密度 | 178 | `variant.stats.v1` + interval 数据 ✅，图待 M1 | 🟡 |
| 根长-纬度渐变群 | 179 | table + 回归缺 | 🟡 |
| FLC 覆盖度 track | 185 | `alignment.coverage.v1` 数据 ✅，track 图缺 | 🟡 |
| LD 衰减 / Fst / SFS / Manhattan / QQ / 区域关联（真实 + 对照） | 175–177, 180–182, 194, 195 | 群体遗传 / GWAS 栈缺 | ❌ |
| 采集地理分布 | 187 | 地图可视化缺 | ❌ |
| 埋藏面积条形图 | 191 | 界面 buried-area 计算缺 | ❌ |

## 缺口聚类（Gap Clusters → 补齐候选）

1. **群体遗传 / GWAS 栈**（曼哈顿、QQ、LD、π/Fst、SFS、Tajima、ADMIXTURE、
   LocusZoom、单倍型、GRM）——最大单簇，约 18 个图型；建议列为二批首要主题。
2. **表观基因组**（ChIP 剖面、足迹、甲基化、Hi-C）——M4 已排 `epigenetics.*`。
3. **单细胞深度**（拟时序、通讯、velocity、marker 图）——M4 已排
   `expression.single-cell.v1`，绘图配套 M1。
4. **MD / 结构深度**（RMSF/RMSD/Rg、口袋学、ΔΔG、对接、PAE、SASA）——依赖
   外部 MD 工具，远期。
5. **剂量-响应 / 酶动力学拟合**（IC50、米氏、LB、DSF、ELISA 标曲）——一个
   `curve.fit` 类能力可覆盖 5+ 图型，性价比高。
6. **ML 评估**（ROC、PR、混淆矩阵、特征重要性）——一个评估类能力可覆盖。
7. **网络 / PPI / 通用统计图**（桑基、脊线、雷达、漏斗、哑铃、帕累托）——
   M1 绘图 pack + 少量数据能力。

## 结论（Answer）

**并非 195 张都能分析**：现在能端到端出数的约 58 型（~30%），数据侧已可分析、
等 M1 绘图 pack 即可出图的约 33 型（~17%），明确缺口约 88 型（~45%，集中在上
面七个簇），仪器图像类约 12 型（~6%）属于范围外。该矩阵作为 M4 与二批能力排期
的输入，每补一个能力回填本表状态列。
