---
title: "把一个有标准答案的药物对接实验，搬到两台不同厂商的 GPU 服务器上"
date: 2026-10-02
tag: technical
lang: zh
desc: "Linxira Bio SDK 上线 GPU 厂商支持前的云端验证：全部采用社区既有基准（Vina 官方教程体系、STREAM 内存带宽惯例、pytrf 参照实现），在 NVIDIA 与摩尔线程两台服务器上验证正确性、复现性与速度——对接搜索预算给出明确够用线（e=8 三次丢一次，e≥32 从不丢），五类输出跨厂商逐字节一致，Rust 实现相对传统 Python 工具 3.6–4.0 倍。"
---

# 把一个有标准答案的药物对接实验，搬到两台不同厂商的 GPU 服务器上

> Linxira Bio SDK 云端验证报告 · 2026-10-02。环境、机器性能、命令全部
> 披露；复现脚本随仓库发布。
>
> **勘误（2026-10-12）**：补齐本轮的层次归属——GPU 内核由厂商原生编译器（`nvcc`/`mcc`）编译，
> 我们的 Rust 侧只做编排；对照表见 §五末，逐条见文首更新日志。**数据与结论未变。**

## 先说结论

1. **对接搜索预算有明确的"够用线"**：在 Vina 官方教程的经典体系（HIV-1
   蛋白酶 + 茚地那韦）上，默认的 exhaustiveness=8 三次里有一次找不到
   正确构象（偏差 12 Å）；给到 32 以上，三次全部落在正确构象 0.07 Å
   以内。快筛用 8，定结论用 32。
2. **分析链通过了信号注入检验**：模拟转录组里藏了 150 个差异基因和
   一条定向富集通路，五步分析链把它以 p = 6×10⁻³¹ 挖出来，其余通路
   全在背景水平——不漏报、不误报。
3. **两台机器的输出一个字节都不差**：对接、SSR 微卫星挖掘、表达谱
   五步链、富集分析——五类输出文件的 SHA256 在两台服务器上完全相同
   （**前提见 §3.3**：对接 JSON 的信封要求两次运行的 `--cpu` 相同；
   姿态文件与 `--cpu` 无关）。
4. **Rust 实现相对传统 Python 工具稳定快 3.6–4.0 倍**（100 万碱基 SSR
   挖掘，三个不同 CPU 上复测），输出与参照实现逐字节一致。
5. **国产卡能跑我们的东西**：同一份 **C/C++ 内核源码**（同一个文件，MUSA 与 CUDA
   两侧只差扩展名与宏），摩尔线程 `mcc` 一次编译通过、结果算对，内存带宽利用率与
   国际卡同档。**GPU 内核由厂商原生编译器编译，我们的 Rust 侧只做编排**（建缓冲区、
   发射、取回、校验）。

## 更新日志

- **2026-10-02** 初版发布。
- **2026-10-12 勘误**：补齐层次归属说明——本轮的 GPU 内核由厂商原生编译器（NVIDIA `nvcc` /
  摩尔线程 `mcc`）编译，我们的 Rust 侧只做编排（建缓冲区、发射、取回、校验）；§五末新增对照表，
  摘要第 5 条、§五首段、§六尾句同步限定。**数据与结论未变**（一份内核源码、两家原生编译器仍是
  本文的实测结论；当时两家都还不能用 Rust 写内核）。
- **维护规则**：新增内容落到其逻辑归属章节，并在本日志加一行日期摘要；勘误同样留痕于此。

## 一、这项测试是谁发起的、为什么

发起方是我们自己——Linxira Bio SDK 项目组。这个 SDK 即将加入 GPU
厂商后端（含摩尔线程 MUSA），上线前需要回答一个问题：**我们的工具链
放到不同厂商的硬件上，结果还对吗、还快吗、还能复现吗？** 于是租了
两台云服务器做一轮完整验证，本文是这轮验证的记录。

一个原则先说清楚：**考题全部用别人已经做好的现成基准，不自制**——

- 对接用 AutoDock Vina 官方教程体系（HIV-1 蛋白酶 + 茚地那韦，
  PDB 条目 1iep [3]），官方答案随教程发布（最优结合能 -13.234
  kcal/mol 及参考构象文件）；
- 内存带宽用 STREAM 惯例 [5]；
- SSR 挖掘用 pytrf（社区参照实现）做正确性对照；
- 转录组没有现成标准答案，用"信号注入"办法：在模拟数据里藏已知
  信号再让工具找回来——这是业界验证分析管线的通行做法。

用现成基准的好处是结果可与社区直接对比，不存在"考题为自己量身定做"
的问题。

## 二、环境与机器性能

两台云租赁服务器，同日窗口，同一仓库克隆后临场编译（构建 47.3 秒，
59 个 crate）：

| 项 | 服务器 A（NVIDIA） | 服务器 B（摩尔线程） |
| --- | --- | --- |
| GPU | RTX 4080 SUPER，32GB，驱动 595.71.05（sm_89） | MTT S4000，48GB，MUSA 驱动 2.7.0 |
| GPU 编译器 | nvcc 11.8 | mcc（MUSA toolkit 3.1.0） |
| CPU | Xeon Platinum 8375C @2.9GHz，12 vCPU（62GB 内存） | Xeon Gold 6430，15 vCPU（100GB 内存） |
| 系统 | Ubuntu 22.04，内核 5.15.0-25 | Ubuntu 22.04，内核 5.15.0-105 |
| 内存带宽（实测，STREAM triad） | 14.9 GB/s | 13.5 GB/s |
| 数据盘顺序写（2 GiB） | 505 MB/s | 677 MB/s |
| SDK | linxira-bio 1.1.1（Rust） | 同左 |
| 对接工具 | AutoDock Vina 1.2.5 官方二进制 | 同左 |

（内存与磁盘实测值受云实例调度影响，代表这两台实例而非厂商规格，
边界说明见第八节。）

## 三、证明了什么

### 3.1 对接：算得对，且知道预算要给多少

红停靠检验——把实验解出的复合物拆开，让对接程序从随机位置把配体
"摆"回去，看能否回到晶体结构的位置 [2,4]。12 组（搜索预算 × 随机
种子）在两台机器上各跑一遍，下表为最优构象与官方参考构象的重原子
偏差（埃）：

| 搜索预算 | 种子 42 | 种子 43 | 种子 44 |
| --- | --- | --- | --- |
| 8（默认） | 0.058 | 0.861 | **12.42（-10.9，正确构象丢了）** |
| 16 | 0.058 | 0.861 | 0.015 |
| 32 | 0.062 | 0.068 | 0.015 |
| 64 | 0.056 | 0.047 | 0.045 |

**两台机器的这张表逐值相同**（包括失败的那个种子）。1–2 埃以内算
"回到同一口袋"；12 埃是 Monte Carlo 搜索预算不足时的典型陷阱——
程序不报错，只安静给出次优解（-10.9 看起来也"不错"）。线程数超过
8 后不再提速（72 秒 → 13 秒后持平），因为 e=8 就是 8 条独立搜索链。

### 3.2 转录组分析链：信号注入检验通过

模拟表达谱（2000 基因 × 12 样本，6 对照 6 处理，泊松计数）里藏了
150 个差异基因和一条定向富交通路。五步链（质控 → median-ratio
标准化 [6] → PCA → 聚类 → GO 超几何富集 [7]）全部通过 Rust CLI
完成：

- 藏入通路以 **p = 6.3×10⁻³¹** 居首（44 基因命中），其余 25 条通路
  p > 0.03，无假阳性；
- PCA 与聚类把样本按处理/对照干净分开。

### 3.3 复现性：五类输出跨厂商逐字节一致

对接结果 JSON、SSR 挖掘 TSV（100 万碱基）、同种子对接复跑文件、
表达谱五步链 JSON、富集 JSON——**五类文件的 SHA256 在两台服务器上
完全相同**。换厂商、换 CPU、换核数，结果不变。

**但必须把前提写清楚（2026-10-12 更正）**：这里的"逐字节一致"指的是**这两轮的配置**——
JSON 信封里唯一的机器相关字段是 `result.thread_count`，它回显请求的 `--cpu`（引擎
`native_tools.rs` 始终透传该参数），因此**只有当两次运行的 `--cpu` 相同时，JSON 才是逐字节相同**；
后续第三轮扫描里 NVIDIA 用 `--cpu 32`、摩尔线程用 `--cpu 15`，每个 `A_*.json` 都因此不同，
**且仅差这一个字段**（把该字段归一化后即逐字节相同）。
**与 `--cpu` 无关的是姿态文件**：同种子复跑得到的 `*_out*.pdbqt`
（12,180 字节，SHA256 `6d5830…063e2`）在两台机器、以及在不同 `--cpu` 下都逐字节相同；
数值层（亲和力/RMSD 12 行）在两台机器上逐值相同。
另外诚实说明：我们没有证明 libm/ULP 级无关性，只证明了"这两台机器 + 这个官方二进制
（`vina_1.2.5_linux_x86_64`）+ 这个负载"逐字节相同；第一轮的 Vina 获取过程未留安装日志。

## 四、我们相对传统工具的速度

SSR（微卫星）挖掘是"同一算法、两种实现"的直接对照：我们的 Rust
实现 vs 社区通行 的 Python 实现 pytrf（同为正确性参照）。100 万
碱基输入，三台不同 CPU 上各测三次取中位：

| 机器（CPU） | Rust 实现 | pytrf（传统 Python） | 提速 |
| --- | --- | --- | --- |
| 第 1 轮云租赁实例（16 核 Xeon） | 0.082 s | 0.312 s | 3.8× |
| 15 核 Xeon Gold 6430（服务器 B） | 0.047 s | 0.169 s | 3.6× |
| 12 vCPU Xeon Platinum 8375C（服务器 A） | 0.073 s | 0.288 s | 4.0× |

输出文件与参照实现**逐字节一致**——提速不以改变结果为代价。值得
说明的是对接：我们的定位是编排与复现（调用 Vina 原生引擎 [2,4]），
不在对接算法本身上抢功；这轮验证的是我们的编排层不引入偏差。

## 五、两块 GPU 上的对照

同一份 **C/C++ 内核源码**（`bench_suite.mu`，CUDA 侧只是复制成 `.cu`），两家**原生**编译器
各编各的（`nvcc` / `mcc`）；Rust 侧只负责建缓冲区与发射。四个内核（拷贝、triad、直方图、
相关性）全部通过正确性门（位级或对 f64 参照误差 < 10⁻⁴）：

| 内核 | RTX 4080 SUPER | MTT S4000 | 备注 |
| --- | --- | --- | --- |
| 拷贝 1 GiB | 630 GB/s | 474 GB/s | 单向读写，NVIDIA 快三分之一 |
| triad（两读一写，STREAM 口径） | 666 GB/s | 711 GB/s | 两家基本打平 |
| 直方图（原子操作） | 26.6 GB/s | 6.5 GB/s | MUSA 原子吞吐目前落后约 4 倍 |
| 相关性矩阵 | 282 GB/s | 49 GB/s | 双方均未做合并访存，绝对值非峰值 |

结论：**两块卡的内存带宽同档**（各达标称值九成左右），跑数据搬运
为主的生信内核等价；原子操作差距写 MUSA 内核时应回避（换分桶归并）。

> **本文口径：哪部分是 Rust、哪部分是 C++（2026-10-12 补记）**
>
> 本文（以及同一批 GPU 实验）容易被读成"我们用 Rust 写了 GPU 内核"，不是。分工是：
>
> | 层 | 语言 | 谁完成 | 说明 |
> | --- | --- | --- | --- |
> | 编排层：读数据、建缓冲区、发射、取回、校验、写报告 | **Rust** | 本 SDK 的引擎与 CLI | 本文里所有"我们的实现"指的都是这一层 |
> | 内核层：拷贝、triad、直方图、相关性 | **C/C++**（CUDA C++ 与 MUSA C++ 是同一份源码） | 厂商原生工具链 `nvcc` / `mcc` | 四个内核的计时表来自这一层 |
> | CPU 基线层：SSR 挖掘、分子对接、表达谱五步链 | **Rust** | 本 SDK | 与 GPU 内核无关 |
>
> **为什么内核当时是 C++**：截至 2026-10-02，**两家都还不能用 Rust 写内核**。NVIDIA 的 CUDA
> Rust（`cuda-oxide` / `cutile-rs`）2026-09-08 才公布，且在我们租到的实例上未能进入执行；
> 摩尔线程的官方 Rust 工件（`cudarc-musa`，2026-09-23）是 **host 层**绑定，内核仍需 MUSA C++
> 编译。所以"一份 Rust 源码、两家厂商"当时并不成立，成立的是**"一份内核源码、两家原生编译器"**。
> 各家 Rust 支持的分层矩阵（谁能用 Rust 调 GPU、谁能用 Rust 写内核、什么仍必须 C++）记录在仓库
> 台账：<https://github.com/Linxira-OS/linxira-bio-sdk/blob/main/docs/engine-evals/vendor-rust-status-2026-10-12.md>

## 六、期间发现的两个驱动层事实

中性记录，日志证据在仓库台账里：NVIDIA 的 vGPU 云实例驱动不带
Vulkan 入口（三个独立实例同一报错）；摩尔线程的 Vulkan 驱动能加载、
能打开设备，但在初始化一步失败（疑用户态 toolkit 3.1.0 与宿主驱动
2.7.0 配对问题，已整理证据准备反馈厂商）。不影响本文结果——内核走的都是各家原生编译器（C/C++，不是 Rust）。

## 七、怎么复现

```bash
git clone https://github.com/Linxira-OS/linxira-bio-sdk.git
cd linxira-bio-sdk && cargo build --release -p linxira-bio-cli

bash scripts/benchmark/rental/dock-study.sh                          # 对接研究 + RMSD 分析
mcc -O3 scripts/benchmark/rental/bench_suite.mu -o bench_mu -lmusart # GPU 内核（MUSA）
cp scripts/benchmark/rental/bench_suite.mu b.cu && nvcc -O3 -arch=sm_89 b.cu -o bench_cu
bash scripts/benchmark/rental/rna_analysis.sh                        # 信号注入验证
bash scripts/benchmark/rental/system_bench.sh                        # 存储/内存带宽
```

原始输出（JSON/TSV/日志/哈希）在本地归档 `release-artifacts-rental/`
（不入库）；逐轮实验台账在 `docs/engine-evals/`。

## 八、边界

- 每卡一台云租实例、一个时间窗；云实例的 vCPU/内存带宽/磁盘受宿主
  调度影响，**实测时延只代表这两台实例，不代表厂商规格**。环境表的
  CPU/内存以租赁平台控制台规格为准（容器内 nproc 读到的是宿主 CPU 数，
  不反映实例配额）；软件版本均为实机读数。
- 磁盘读速为页缓存口径（容器无权清缓存）；对接结论只对官方 1iep
  体系成立，其他体系需自行验证。
- e=8 丢解是 Monte Carlo 搜索预算的固有性质，不是缺陷；它的价值是
  提醒你给自己定一条预算线。

## 参考文献

[1] Williams, S., Waterman, A., Patterson, D. Roofline: an insightful
visual performance model for multicore architectures. *Commun. ACM*
52(4), 65–76 (2009). doi:10.1145/1498685.1498715

[2] Trott, O., Olson, A. J. AutoDock Vina: improving the speed and
accuracy of docking with a new scoring function, efficient optimization,
and multithreading. *J. Comput. Chem.* 31(2), 455–461 (2010).
doi:10.1002/jcc.21334

[3] Berman, H. M. et al. The Protein Data Bank. *Nucleic Acids Res.*
28(1), 235–242 (2000). doi:10.1093/nar/28.1.235

[4] Eberhardt, J., Santos-Martins, D., Tillack, A. F., Forli, S.
AutoDock Vina 1.2.0: New docking methods, expanded force field, and
Python bindings. *Nucleic Acids Res.* 49(W1), W3355–W3362 (2021).
doi:10.1093/nar/gkab663

[5] McCalpin, J. D. STREAM: Sustainable Memory Bandwidth in High
Performance Computers. Technical Report, University of Virginia
(1991–2007). https://www.cs.virginia.edu/stream/

[6] Anders, S., Huber, W. Differential expression analysis for sequence
count data. *Genome Biol.* 11, R106 (2010). doi:10.1186/gb-2010-11-10-r106

[7] Ashburner, M. et al. Gene Ontology: tool for the unification of
biology. *Nat. Genet.* 25(1), 25–29 (2000). doi:10.1038/75556

## 版本与声明

- **版本**：2026-10-02 首发；**2026-10-12 勘误**：补充分层说明（Rust 编排 / C++ 内核），
  数据与结论未变，逐条见文首「更新日志」。
- **自评**：作者自报的验证报告，非第三方独立验证；单实例单窗口，
  边界见第八节。第三方复现为后续工作。
- **许可**：代码 AGPL-3.0-or-later；本文 CC-BY-4.0。
- **作者与溯源**：Linxira-OS 项目维护者 · 仓库：
  <https://github.com/Linxira-OS/linxira-bio-sdk>

---

*Linxira-OS · AGPL-3.0-or-later（代码）· 本文 CC-BY-4.0 ·
<https://github.com/Linxira-OS/linxira-bio-sdk>*
