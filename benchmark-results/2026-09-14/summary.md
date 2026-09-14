# Benchmark summary — 2026-09-14（bio-lab 部署 + tier1_v2 交叉校验）

> **状态口径**：本日两批（bench-20260914-001 commissioning、bench-20260914-002
> 14-run 交叉校验）是**部署验证与对照校验**，尚不构成"对传统管线的加速比"
> 结论——原因见 §3。真正的速度对照待深文库批量与有效参照后发布。

## 1. 环境与版本（披露）

- bio-lab（CachyOS，24 核 32G，F 盘），运行限核 `taskset -c 8-15 nice -n 10`
- SDK v1.0.3（c8ebee3 + quantify 修复 ff90f7d/d674472），salmon 2.7.0
  （bioconda **原版**二进制，经 micromamba 用户级安装于 `/mnt/F/biosdk_ws/tools-env`）
- 对照索引：Pinku1 CDS（33,955 转录本），与主线所有定量同一索引

## 2. 已测得的数字

### 2.1 全流程部署验证（bench-20260914-001）

- **SRR26171873**（双端 gzip，约 1.1G+1.1G）：`linxira-bio expression quantify`
  全流程 **59 秒**（引擎编排 salmon，含读入/映射/定量/quant.sf 落盘），
  33,955 转录本，14,320,007 条 reads 正常分配
- 与主线 quant.sf **结构完全一致**：行数、转录本 ID、表头逐项相同
- 截断守卫验证：拒算截断的 SRR22699515 fastq（结构化错误，零静默产出）

### 2.2 tier1_v2 交叉校验（bench-20260914-002，14 个单端 run）

| 通过线 | 结果 |
|---|---|
| 行数/ID 一致 | 14/14 全过 |
| NumReads 总量差 <1% | 14/14 全过（≤0.0013%） |
| TPM Pearson r ≥ 0.995 | 0.0–0.94，**不达标（根因见 §3）** |
| 单 run 耗时 | 5–17 秒（中位 10.5 秒，gzip 直读全流程） |

计时表：bio-lab `/mnt/F/biosdk_ws/bench/benchmark.csv`（由泛用工具
`scripts/quantify-bench.sh` 产出，该工具已入库、全参数化、无站点路径）。

## 3. 为什么"我们 vs 传统"的加速比现在还写不出来（重要）

对照方（主线管线）的 quant 输出**不是上游 salmon**：其日志自述
`salmon (rust port, reads mode) v2.7.0`，是一份重实现。在该重实现下这批
样本的 mapping rate 仅 **0.0017%**（1142 万 fragments 映射 192 条）。速度上
"它更快"恰恰因为它几乎不映射任何 reads——**任何对此参照的 wall-time 加速比
都是无意义的**；TPM 分配在如此极端低覆盖下也只是噪声。

结论：
1. 我们的实现与重实现对照**只取结构一致性与 NumReads 总量**（全过）；
2. 有效 speedup 必须等：主线侧修复 rust port（或换原版 salmon）后重算，
   或我们对同一 run 用**原版 salmon** 与 SDK 编排互测（同机器、同参数、
   同输入，双方都真实映射）；
3. 该发现已如实写入 bench-20260914-002 的
   `reference_implementation` 字段与 `status: partial` 口径。

## 4. 预期中的有效对照（下一步）

1. **深文库 25 个**（SRR19049440-45、SRR22699505-16、SRR24322339-53）：
   主线队列跑完后从 NAS `/mnt/disk1/tier23/` 拉 SRA，SDK 端 fasterq-dump +
   quantify 与主线产出逐数值对照（他们用真 salmon 时才有意义；当前队列
   跑的也需确认实现版本）。
2. **双端 15 run**（NAS `/mnt/disk2/tier1_v2/`）：等 NAS→lab 拷贝通路。
3. 三端（rust/python/r）fixture 基线见 `2026-09-13/summary.md`
   （7.5–8.25×，与今日 bio-lab 数字分属不同环境，禁止混排）。

## 5. 数据来源（与本日对话清单一致）

| 数据 | 检索号 | 来源 |
|---|---|---|
| SRR26171873 / SRR28573920-25 / SRR8205656-63 | NCBI SRA（荞麦项目，PRJ 见 final_census.tsv） | bio-lab 本地暂存（只读） |
| SRR22699515 截断 fastq | SRR22699515 | `/mnt/F/thesis_inbox/`（只读，主线正在重拉） |
| Pinku1 CDS 索引与对照 quant.sf | — | `/mnt/G/thesis_dwf4/analysis/`（只读） |

所有原始数据不入 git；本目录只存指标、口径与来源记录。
