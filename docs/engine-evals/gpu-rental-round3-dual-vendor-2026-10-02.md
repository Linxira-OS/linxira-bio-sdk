# 云租赁第 3 轮 — 双机并发：内核套件 + 对接定量研究（2026-10-02）

> 承接第 1/2 轮。用户开两个窗口同时跑：克隆 vGPU 实例（RTX 4080 SUPER，
> Xeon Platinum 8375C 128 vCPU）+ S4000（Xeon Gold 6430 15 核）。本轮补齐
> 论文两张主表：**双厂原生工具链内核实测**与**分子对接定量研究**。
> 复现源码在 `scripts/rental/`（仓库内，不进发布包）。

## 机器快照（论文环境表）

| 项 | vGPU 克隆 | S4000 |
| --- | --- | --- |
| GPU | RTX 4080 SUPER 32GB, 驱动 595.71.05, sm_89 | MTT S4000 48GB, 驱动 2.7.0 |
| 编译器 | nvcc 11.8 (V11.8.89) | mcc（clang-14 派生，toolkit 3.1.0） |
| CPU | Xeon Platinum 8375C @2.9GHz, 128 vCPU（超卖疑） | Xeon Gold 6430, 15 核 |
| 内核 | 5.15.0-25-generic | 5.15.0-105-generic |
| VK | **第二次逐字复现第 1 轮错误**（ICD 无 vkCreateInstance） | 第 2 轮结论不变（CreateInstance 驱动内失败） |

VK 双机合计 3 个独立实例同一形态结论：vGPU 无 Vulkan 是驱动切片结构性事实。

## 表 1 — 双厂单源内核套件（同文件双编译，median-of-3，全过正确性门）

`bench_suite.mu`（`.cu` 仅扩展名不同）：copy 1GiB rw、STREAM-triad、
256-bin 直方图（共享内存+原子）、Pearson v2 4000×1000（499500 对）。

| 内核 | 4080S | S4000 | 比 |
| --- | --- | --- | --- |
| copy (rw) | **629.8 GB/s** | 473.8 GB/s | 1.33× |
| triad (rwr) | 665.7 GB/s | **711.2 GB/s** | 0.94× |
| hist256 (read+原子) | 26.6 GB/s | 6.5 GB/s | 4.1× |
| pearson4k (逻辑) | 281.9 GB/s | 49.0 GB/s | 5.75× |

- **带宽孪生假说验证成立**：triad（最接近 STREAM 口径）666 vs 711 GB/s，
  规格值 736 vs 768 GB/s——两卡带宽效率同档（90%/93%），孪生配对正确；
  copy 单向读写下 4080S 快 33%（写路径/L2 策略差异，非 DRAM 差距）。
- 原子与 Pearson 的差距是 MUSA 共享原子吞吐和编译器成熟度短板，
  与 G3 计划"逐卡定策略、不预设赢家"一致；pearson 首版非合并访存，
  两边同源同罚，比值公平但绝对值非峰值。
- 正确性：copy/triad maxerr=0（位级）；hist 256 桶与 CPU 精确相等；
  pearson 相对误差 1.5e-7 / 8.3e-8（< 2e-4 门）。
- 测量坑（已修，脚本内注明）：计时 3 次会重复累加直方图 → 检查前重置
  单跑一次；首版 hist 检查 FAIL 是该坑不是驱动问题。

## 表 2 — 分子对接定量研究（chemistry.dock.v1，官方 1iep）

扫描：exhaustiveness {8,16,32,64} × 种子 {42,43,44}（cpu=机器核数）+ 线程
扫描（NV 1/8/32/128，S4000 1/4/15，e=8 seed42）。指标：best affinity、
重原子 RMSD vs 官方参考构象（37 重原子，in-place）、wall。

### 跨厂商确定性（主发现）

**12 个 (e,seed) 点的 RMSD 表在两台机器上逐值相同**（0.058/0.861/12.422/
0.015/…），affinity 表亦同值——Vina 给定 seed+exhaustiveness 的输出与
CPU 厂商/核数无关，**双厂复现字节级成立**（两轮 1iep -13.27 同源的强化）。

### 收敛性（论文核心结论）

| e | seed42 | seed43 | seed44 |
| --- | --- | --- | --- |
| 8 | 0.058 Å | 0.861 Å | **12.42 Å（-10.9，全局最优丢失）** |
| 16 | 0.058 | 0.861 | 0.015 |
| 32 | 0.062 | 0.068 | 0.015 |
| 64 | 0.056 | 0.047 | 0.045 |

- e=8 时 1/3 种子丢全局最优（RMSD 12 Å 量级）；**e≥32 全种子收敛到官方
  构象 0.07 Å 内**。给用户的操作含义：默认 e=8 快筛可以，定构象用 e≥32。
- 姿态间能量差（rank1-rank2 ≈ 2.0 kcal/mol）稳定，模式数恒 9。

### 线程扩展

- NV：cpu1=71.9s → cpu8=12.7s → cpu32/128≈13s（**8 线程吃满即平坦**，
  vina e=8 为 8 条独立 MC 链，物理上限就是 8）。
- S4000：cpu1=80.5s → cpu4=22.2s → cpu15=14.4s（同型曲线）。
- 时长纹理（如实注记）：S4000 e=64 wall 52-53s vs NV 135-142s——NV 实例
  "128 vCPU"疑云上超卖，单核效率低于 S4000 的 15 个真核；**wall 是实例
  属性不是厂商 CPU 基准**，论文只作现象记录。

## SSR 补充（128 vCPU 机器）

1M 碱基：rust 0.070/0.075/0.073 s（median 0.073）vs pytrf 0.332/0.288/0.287
（median 0.288）——4.0×，与 15 核机（3.6×）、16 核 Xeon（3.8×）跨机一致；
TSV 字节一致。

## 产物

- `release-artifacts-rental/round3-vgpu-clone/`（16 对接 JSON+PDBQT、
  bench_nv.txt、nvcc 版本、sha256）与 `round3-s4000/`（15 JSON、bench_mt.txt）。
- `scripts/rental/`：bench_suite.mu、pearson_bandwidth.mu、dock-study.sh、
  dock-study-s4000.sh、analyze_dock.py、README（复现指引；仓库内、
  不进发布包）。
- 分析器两处修正已固化：参考构象在 `expected/` 子目录；PDBQT 解析须
  过滤氢原子且 `ENDROOT` 不得当作模型边界（`startswith("END")` 陷阱，
  首版"4 原子"即此坑）。

## 遗留

- Pearson 合并访存优化版（提升两卡绝对值，比值预期不变）。
- NV 侧若要更高 CUDA 版本对照：镜像自带 11.8，CUDA 13 需 pip wheel 路线
  （第 1 轮已踩通）。
- 博客素材至此齐备：带宽孪生实测、对接收敛与跨厂确定性、双厂生态对照。
