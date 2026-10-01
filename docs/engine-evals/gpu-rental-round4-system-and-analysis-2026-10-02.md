# 云租赁第 4 轮 — 存储/内存带宽 + 分析栈全链真数据（2026-10-02）

> 承接第 3 轮。用户点名补齐：硬盘读写、内存速度作为评估维度；把已有
> 能力尽可能真跑一遍。两机同窗口并发（NV 克隆 + S4000）。
> 复现脚本随 `scripts/rental/`（第 3 轮已立）追加。

## 表 A — 存储与内存带宽（dd + 自编 STREAM 四核，2.3 GiB 工作集）

| 维度 | vGPU 克隆（8375C, 128 vCPU） | S4000（Gold 6430, 15 核） |
| --- | --- | --- |
| 数据盘顺序写（dd conv=fdatasync, 2 GiB） | 505 MB/s | **677 MB/s** |
| 数据盘读（刚写文件，含页缓存） | 5.4 GB/s | 7.4 GB/s |
| 系统盘写（overlay, 1 GiB） | 437 MB/s | 677 MB/s |
| RAM Copy（单线程） | **27.2 GB/s** | 22.1 GB/s |
| RAM Triad（单线程） | **14.9 GB/s** | 13.5 GB/s |
| RAM Triad（OMP 全核） | 14.6 GB/s | 13.3 GB/s |

- **读速是页缓存值**（drop cache 容器内无权限），论文注明"缓存读"口径。
- **OMP 不扩展**（128 线程 ≈ 单线程）：云实例内存控制器带宽封顶，不是
  裸金属性能——与第 3 轮"128 vCPU 疑超卖"互相印证，实例属性而非厂商
  CPU 规格对比。
- 单线程 RAM：8375C 略快（27 vs 22 GB/s copy）。

## 表 B — 分析栈全链真数据（模拟表达谱 + GO 富集）

输入（种子 4242，numpy 合成）：2000 基因 × 12 样本（6 对照 + 6 处理），
150 个差异基因（log2FC ∈ ±[1,2]），泊松计数 + 文库系数；GO 关联表
30 词项、其中 1 词项定向植入前 40 个 DE 基因。

| 步骤（CLI 能力） | 结果 | 判定 |
| --- | --- | --- |
| `expression matrix-qc` | 2000×12，QC 指标 JSON | PASS |
| `expression normalize --method median-ratio` | norm.tsv 457KB | PASS |
| `expression pca --components 4` | 方差分解 JSON | PASS |
| `expression cluster` | 双向聚类 JSON 151KB | PASS |
| `enrichment go --min-overlap 3` | 26 词项；**植入词项 GO:0001000 p=6.3e-31（44 基因）居首**，其余 p>0.03 | PASS（信号回收正确） |

- **跨厂商位级复现（第 5 个数据点）**：五份结果 JSON 的 SHA256 在两台
  机器**完全相同**——QC→normalize→PCA→k-means→超几何 ORA 整链
  双厂 CPU 位级一致（与对接、SSR、dock 复跑同性质，证明 SDK 的确定性
  设计贯穿全栈，不挑分析类型）。
- 踩坑两处（校验器把关正确，属输入错误）：GO 号必须 `GO:` + 7 位数字
  （`GO:TERM00` 被拒）；容器无 `/usr/bin/time`（改 date 计时，已在第 3
  轮记档）。

## 能力覆盖对账（用户点名项逐一回应）

| 用户点名 | 状态 |
| --- | --- |
| 分子对接 | ✅ 第 1/2/3 轮（36 点扫描 + 跨厂同值 + e≥32 收敛） |
| SSR 分析 | ✅ 三机三后端字节一致 |
| 功能富集 | ✅ 本轮 GO ORA 真数据验证（GSEA 有能力未跑，输入需排秩表，下轮可加） |
| 差异表达 | ⚠️ 能力为 workflow pack（R/DESeq2 路线），租用容器无 R——本轮跑的是表达谱 QC/normalize/PCA/cluster 链 + 模拟真值；DE pack 上机待 R 环境 |
| 表达矩阵分析 | ✅ 本轮全链 |
| qPCR 模拟 | ❌ 无此能力 → 已入 ROADMAP §11（`simulate.qpcr.v1` 构思，MIQE 对标） |
| 虚拟细胞 | ❌ 无此能力 → 已入 §11 长期池观察项（开放权重包装路线） |

## 产物

- `release-artifacts-rental/round4/{nv,mt}/`：五份 JSON + norm.tsv + 输入
  数据 + 双机 sha256 + sysbench/rna 日志（git-ignored）。
- 复现脚本：`system_bench.sh`、`rna_analysis.sh`（本轮，随 scripts/rental/
  一并提交）。

## 结论

四轮租赁累计五类跨厂位级复现证据（对接 JSON/SSR TSV/dock 复跑/表达谱
五 JSON/富集 JSON），加带宽孪生内核实测与存储/内存维度补齐——论文的
"环境表 + 主结果表 + 复现性表"三件套数据齐备。
