# 实现迭代日志（Implementation Iteration Log）

> 用途：同一分析方法可能有 **v1/v2 多个实现版本共存**，算法会持续更新换代。
> 本文件是所有分析实现（engine 内核 + benchmark pack 的 Python/R 实现）的
> **唯一迭代台账**：每次算法/实现变更写一条，写清改了什么、为什么、证据。
> Purpose: the single iteration ledger for analysis implementations across
> Rust/Python/R; every change gets one dated entry with evidence.

## 规则（Rules）

1. **版本共存**：能力 id 带版本后缀（`foo.v1` / `foo.v2`）。发布 v2 时 v1
   保持可用（契约、测试独立），v1 的废弃是显式决定并在日志中标注
   `superseded-by`。
2. **什么必须记**：算法变更（数值行为可能改变）、性能重写、依赖切换、
   修复数值 bug、pack 实现更新。纯文案/重构不改行为的不用记。
3. **证据要求**：算法或性能变更必须附 benchmark 报告编号（`benchmark-results/`
   的 `report_id`）或测试引用；三端实现的对齐变更附 parity 测试结论。
4. **条目格式**：日期 | 能力/组件 | 变更 | 动机 | 证据 | 状态。

## 台账（Ledger）

| 日期 | 能力 / 组件 | 变更 | 动机 | 证据 | 状态 |
| 2026-10-12 | benchmark 图表工具链（非产品能力） | `figures/render-svgs.py` 由 8 张扩为 12 张/语言并统一版式：新增离散度 CV、判定阶梯（1–10× 与 ≥10× 两段线性，含 1.10× 实质线）、内存倍率、条件对照信噪比；面板/斑马行/圆角条+轨道；装饰元素标 `class="deco"`，校验器据此跳过：`figures/verify-figures.py` 新增四组几何反解（含 30 行条件对照）与实体转义后的行宽测量；`windows/2026-10-11/archive.sh` 增传 `--contended` | 读者反馈：图注拥挤、须线过短、范围提示占比过大；需要能直接读出判定（1.10× 线）与稳定性（CV）的视图；条件图原按原始 Δ% 对 ±带宽作图，被 54% 带宽的行压扁，改为 |Δ|÷带宽的信噪比视图（原始数值仍逐行标注） | `verify-figures.py` 全绿：1055/1055（zh）、1067/1067（en），含 12×2 张图；归档 `bench-20261011-001` | 完成 |
|---|---|---|---|---|---|
| 2026-09-10 | `sequence.stats.v1` 三端 | 新增 benchmark-python（Biopython）与 benchmark-r（Biostrings）pack 实现，对齐 Rust golden | M3 三端基线第一步 | pack parity 测试；bench-20260913-001 | current |
| 2026-09-11 | `expression.differential.v1` | R pack（DESeq2）对标实现入库 | M3 #1 | workflows/org.linxira.bulk-expression-deseq2 | current |
| 2026-09-12 | GSEA 参数 | `score_exponent` 参数键修正 + GUI 形状契约回归测试 | 参数键与 worker 契约不一致 | commit eb114c5 | current |
| 2026-09-12 | `expression.pca.v1` 三端 | 确定性幂迭代 PCA 的 Python/R 移植（末位并列语义对齐 Rust `max_by`；R `rev()` 行反转语义修正） | M3 #5 三端 | pack parity（max rel err 2.26e-11）；bench-20260913-001 | current |
| 2026-09-12 | `set.venn.v1` 三端 | 掩码集合运算的 Python/R 移植 | M3 #7 三端 | pack parity；bench-20260913-001 | current |
| 2026-09-13 | `structure.pdb.summary.v1` 三端 | 固定列 PDB 解析器（MODEL/ENDMDL 状态机、元素推断、pLDDT 分档）的 Python/R 移植 | M3 #3 三端 | pack parity（max rel err 0.0）；bench-20260913-001 | current |
| 2026-09-13 | benchmark 环境披露 | 报告新增 Windows 宿主机/WSL 探测（interop）与 distro/wsl 字段 | 跨环境数字可追溯 | commit 9052bcf；bench-20260913-001 | current |
| 2026-09-13 | `scripts/materialize-r-lock.py` | 下载加固：https-only + host allowlist + 重定向后校验 | Mimosa 深扫 SSRF high | scan-2026-09-13T10-06-04（seal sha256:9656b8d3…） | current |
| 2026-09-13 | `plot.render.v1` 双包 | 新增 matplotlib/ggplot2 绘图 pack（M1-T1/T2）：PlotSpec 全参数渲染、svg/png 字节稳定（固定 svg hash 盐 / svglite）、跨后端 data_summary 一致性断言 | 回应"绘图=可复现数据产品"设计定位；M1 落地 | 双 pack 测试（py 5/5、R 全过、跨后端一致） | current |
| 2026-09-13 | `enrichment.overrepresentation.v1` 三端 | 引擎 ORA 的 Python/R 移植（log 阶乘超几何上尾 + 同款 BH 平局规则；刻意不用 gseapy/clusterProfiler）。修复两处移植语义：查询表头行跳过、BH rank 反向错误、R jsonlite 单元素数组坍缩 | M3 #6 三端 | 双 pack parity 对 Rust golden（default+include_genes）；WSL E2E Consistent（rust 90ms / py 390ms / r 500ms，4.33x） | current |

| 2026-10-11 | `linxira-bio-core` 输入探测 | `looks_like_sam` 改为要求 SAM 结构证据（FLAG ≤ 0xFFF、RNAME 非纯数字、CIGAR `(\d+[MIDNSHP=X])+` 或 `*`、SEQ 碱基字母或 `*`） | 产品级缺陷：≥11 列的宽计数矩阵被误判为 SAM，worker 会拒绝合法表达矩阵（实测边界 10 列→tsv、11 列→sam） | commit `bfe6fd0` + 回归测试 `wide_numeric_matrix_is_not_mistaken_for_sam` | current |
| 2026-10-11 | benchmark-r pack（6 处） | R 后端**首次真正执行**后修复 6 个潜伏缺陷：① 裸 `XString` 的 `width()` 派发 ② 测试字面量 23 bp 却断言 22 ③ `start()` 未加命名空间命中 `stats::start` ④ 空 `motif_length_counts` 被序列化成 `[]` ⑤ `set_venn` 按 item 而非 occupancy mask 生成 intersections ⑥ CI 从未运行 benchmark 两包测试 | 测试存在但不在 CI 执行 = 不存在；三端声称缺一端的执行证据 | commits `90d982a`/`dd8e226`/`c567f94`/`9b9e4fa`/`005404c`；NAS 全量门禁 525 测试绿 | current |

| 2026-10-12 | benchmark 报告结构 | `BenchmarkRun.started_at`（RFC 3339 UTC）在 worker 启动前打点，经 `ExecutedRun` 落入报告；schema 里为可空字段（旧报告仍可解析） | 污染归因需要逐次时间戳：只有阶段级窗口比对时，无法判断某一次重复是否被挤占 | commit `12e9f32`；协议往返单测 + 旧报告兼容断言 | current |
| 2026-10-12 | `scripts/benchmark/` 窗口驱动 | `rerun-clean.sh` 修正 `fastq.qc` 入参名（`fasta=` → `fastq=`，此前被 worker 格式守卫拒绝、产出空报告）；补齐「先 build 后重建输入」顺序；文档化长窗口必须 `setsid nohup` 脱离 SSH 会话（会话断开曾杀掉驱动脚本而 worker 仍在跑） | 一次秒失败的 N=0 报告 + 一次断线事故，都是可预防的流程缺陷 | commits `d7049a7`、`89882a5`；NAS 实跑复现 | current |
| 2026-10-12 | 跨条件对照工具 | `scripts/benchmark/analysis/compare-conditions.py`：读两组报告，逐能力/后端输出中位数、Δ%、2×(CV_A+CV_B) 噪声带与判定 | 需要回答「CI 抢核是否改动了任何一行」，而不靠肉眼；产品侧对应命令登记为 ROADMAP M2-T11 | 2026-10-11 归档实测：30 行、0 行超噪 | current |

## v2 迁移模板（Upgrading to v2）

当某能力需要不兼容的算法改动时：

1. 新建 `foo.v2`（独立契约/测试/skill 文档），v1 不动；
2. 同批测 `foo.v1` 与 `foo.v2` 的 benchmark，报告编号写入本表；
3. v2 稳定后在 v1 行补 `superseded-by: foo.v2`，capability 目录同步标注；
4. 至少保留一个过渡版本周期后再讨论 v1 下线。
