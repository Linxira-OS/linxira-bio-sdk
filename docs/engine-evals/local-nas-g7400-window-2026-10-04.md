# 本地 NAS 低档机窗口台账（Windows 之外的第 2/3 台机器；2026-10-04）

> 触发：用户要求用本地 NAS（Intel G7400 + 双 GPU）承担"三端速度对比 + 低档 GPU 回退档 +
> 跨机确定性"，把租用预算留给第三方真值与匹配硬件。**本机（Windows/WSL）不跑负载**，
> 所有运行在 NAS 上完成（用户 2026-10-04 指令）。

## 1. 机器与环境

| 项 | 值 |
| --- | --- |
| OS | **CachyOS**（Arch 系；`environment audit` 报 `family=arch`，ID_LIKE 映射生效） |
| CPU | Intel **Pentium Gold G7400**，2 核 / 4 线程（3.7 GHz max），governor = **powersave** |
| 内存 / 磁盘 | 15 GiB / NVMe 473 GB（437 GB 可用） |
| Rust | rustc/cargo **1.99.0**（stable） |
| Python | 3.14.7 + venv（numpy 2.5.4、biopython 1.88 —— 包锁 pin 1.85，pack 发 warning 并如实披露） |
| R | **未安装**（R 后端待装后再补） |
| GPU（Vulkan 枚举） | ① **NVIDIA GTX 750 Ti**（DiscreteGpu，GM107 = 旧卡档 G-L）② **Intel UHD 710**（IntegratedGpu，低端核显） |
| 构建耗时 | CLI release **2m29s**；gpu-lab release **2m01s** |

环境坑（供后续窗口复用）：远端 shell 是 **fish**（脚本须 `ssh nas bash -s` 管道送入）；
无 `bc`；`--coverage` 是**百分比 x100**（`--coverage 500` = 5x）；`simulate *` 第二个位置参数才是输出路径。

## 2. 跨机确定性（本轮新增 2 台机器）

同 seed/model/输入的产物**逐字节一致**：

| 产物 | sha256 | NAS（CachyOS/G7400） | WSL（Core Ultra 5 225H） |
| --- | --- | --- | --- |
| `simulate sequence` ref.fa（20×50 kb） | `389db7a7…` | ✅ | ✅ 一致 |
| `simulate cell` cell.tsv（120 min, seed 42） | `08a2fabc…` | ✅ | ✅ 一致 |
| `simulate cell` cell.json | `cc051281…` | ✅ | ✅ 一致 |

另外：`sequence.ssr.v1` 的 rust 与 pytrf 两后端 TSV 在 **1 Mb 与 100 Mb 两档都字节一致**（各 3 次复跑）。

## 3. 三端速度对比（同机、同输入、同种子、3 次复跑）

### 3.1 小输入（1 Mb，20×50 kb）——**启动开销主导**

| 能力 | rust | python | 比值 |
| --- | --- | --- | --- |
| `sequence.stats.v1` | 0.008 / 0.008 / 0.010 s | 0.172 / 0.173 / 0.199 s | ~20× |
| `sequence.ssr.v1` | 0.016 s ×3 | 0.187 / 0.188 / 0.190 s | ~11.7× |

### 3.2 大输入（97 MB 参考，同一台机）——**吞吐主导，比值塌缩**

| 能力 | rust | python | 比值 |
| --- | --- | --- | --- |
| `sequence.stats.v1` | 0.687 / 0.687 / 0.688 s | 0.832 / 0.832 / 0.843 s | **1.22×** |
| `sequence.ssr.v1` | 1.480 / 1.488 / 1.498 s | 2.288 / 2.288 / 2.290 s | **1.54×** |

> **核心结论（须写进对外口径）**：小输入的"20×"是**解释器启动 + 依赖栈**差异，不是吞吐差距。
> 比值取决于工作负载落在哪一类：
> - **C 级原语可覆盖**（`sequence.stats` 的线性扫描、pytrf 的 C 内核）→ Rust 优势仅 **1.05–1.5×**；
> - **纯 Python 逐元素循环**（`fastq.qc` 的逐记录/逐循环计数器）→ Rust 优势 **31.8×**（§5）。
>
> 因此对外叙事必须按**内核类型**分层，禁止再出现"普遍 N× 加速"的整体声明；
> Rust 的稳定收益是**零解释器启动、无依赖栈、确定性可复现与内存**，而非全局吞吐倍数。

## 4. 低档 GPU 回退档（wgpu/Vulkan）

- **可移植性 bug（已修，提交 `c72cc64`）**：`gpu-bench` 的 dispatch 分组用 `min(65_536)`，
  而 WebGPU 单维上限是 **65_535**；在 750 Ti/UHD 710 上直接 validation error 崩溃。
  云端独显（无 Vulkan ICD，从未跑过 wgpu）掩盖了这个 bug——**"最低档先行"当场抓到真实缺陷**。
  同时新增 `gpu-bench --adapter <index>` 以便分别测核显与独显。
- **两适配器全内核（median-of-3，全部 `pass=true`）**：

| 内核 | GTX 750 Ti（独显） | UHD 710（核显） |
| --- | --- | --- |
| `quality-histogram` | upload 88.7 / compute **12.29** / readback 0.011 ms | upload 74.0 / compute **26.64** / readback 0.026 ms |
| `pearson-column-pairs`（v1，逐对顺序） | compute **47.22** ms | compute **199.49** ms |
| `pearson-column-pairs-wgreduce`（v2，树归约） | compute **17.34** ms | compute **48.43** ms |
| `pearson-column-pairs-multi`（v3，4 子组） | compute **17.06** ms | compute **51.37** ms |

  → 树归约变体在两块卡上都比 v1 快 **2.7–4.1×**，设计意图在最低档硬件上成立；
  核显 2000×500 的 v2 计算 48.4 ms，对照同矩阵 CPU f64 审计 145.9 ms ≈ **3.0×**（精度差 7.42e-7，容差 1e-5）。
  核显 upload 便宜（统一内存）、算力弱（v1 慢 4.2×）；独显 upload 贵（88.7 vs 74.0 ms 量级受首次分配影响）。

## 5. 官方 harness（`benchmark run`，worker 协议，median-of-3）结果

需先 `cargo build --release -p linxira-bio-worker` 并设 `LINXIRA_BIO_WORKER`（CLI 自身不带 worker）。

| 能力 | 数据集 | rust 中位 | python 中位 | 比值 | rust IQR / python IQR |
| --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 97 MB FASTA（20×5 Mb） | **800.85 ms** | **836.69 ms** | **1.045×** | 0.59 / 4.62 ms |
| `fastq.qc.v1` | 1.1 GB FASTQ（3,333,334 reads × 150 bp, 5x） | **7,741.78 ms** | **246,035.18 ms** | **31.8×** | 125.6 / 227.9 ms |

> 两条结论必须分开讲：
> ① `sequence.stats` 两端都在做**线性扫描 + C 级原语**（Biopython 侧是 `str.count` 级别的 C 路径），
>    6 亿碱基下 Rust 只快 **4.5%**——"Rust 更快"在这类能力上**不成立**，不要在对外材料里声称。
> ② `fastq.qc` 是**纯 Python 逐记录循环**，1.1 GB 上 Rust 快 **31.8×**（2.32 vs 73.8 µs/read），
>    这才是 Rust 真正的吞吐优势区间：**逐元素/逐记录的自研内核**。
> ③ harness 口径与直跑 CLI 有小差：同一 97 MB 输入，rust 直跑 CLI 0.687 s、harness 0.801 s；
>    python 侧仅 +5 ms、rust 侧 +114 ms ⇒ worker 协议/进程启动被计入，属协议开销而非算法差异。

## 6. 本轮得出的行动项

- [ ] 对外口径改写：小输入比值标注为"启动开销"，吞吐结论用大输入口径（阈值：≥100 MB 或 ≥1 M reads）
- [ ] 三端覆盖扩展：R 装到 NAS 后补 r 后端（`sequence.stats`/`fastq.qc`/`variant.stats` 等）
- [ ] 低档 GPU 扩测：`gpu-bench` 全内核（含直方图）在 750 Ti 与 UHD 710 的完整分相表
- [ ] GTX 750 Ti 的 CUDA 12.x 路径（需装 toolkit）与 Vulkan 路径对照
- [ ] NAS 侧固定 `performance` governor 或记录并接受 powersave（当前已如实记录）
