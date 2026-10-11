# 云租赁 Round 5 计划 —— 国产优先多平台：S4000 深化 + 昇腾 910 系 + NVIDIA 随时插空（2026-10-04）

> 承接 round 1–4（`cloud-rental-plan-2026-10-02.md` 及各轮台账）。租赁可得性
> （2026-10-04 定案）：**NVIDIA 真卡最易得、随时可测**（不占预订槽）；**S4000 易租**
> （国产主线）；**昇腾最难**——价高 / 时长受限 / 低价档**不允许 SSH 与远程控制**；
> **AMD 无公开租赁平台** → 只能找人协助代测（工具包路线）。本轮新增 NPU 轨
> （昇腾 = M5-G4 首测）与"其他可加速计算"候选清单（§5），窗口前开发项见 §3，
> 逐窗测试项见 §4。

## 1. 平台分层与租赁决策表

| 序 | 平台 | 可得性（2026-10-04 定案） | 技术口径 | 本轮角色（解锁的数据） | 预算 |
| --- | --- | --- | --- | --- | --- |
| W0 | NVIDIA 真卡 V100-32GB / T4（4090 克隆搭车） | **随时可得，不占预订槽**——任意空闲时段插空执行 | sm_70 / sm_75（4090: sm_89），CUDA 12.x–13.0 | cudarc 真 sm 下限边界 + **wgpu 首个成功数据点**（现状 0/2）+ k-mer CUDA 版；4090 = S4000 显存孪生大矩阵对照 | 2h/次 |
| W1 | MTT S4000 / 48GB | 易租，**国产主线** | MUSA 驱动 2.7.0、toolkit 3.1.0（镜像可能更新，窗口内核对） | cudarc-musa Rust 栈首测（G3 唯一未验证环节）、Vulkan 错位复查、k-mer/pearson-v4 MUSA C 内核 | 3h |
| W2 | 昇腾 910B（优先）/ 910C / 初代 910 | **最难**：价高、时长受限、低价档**无 SSH/不允许远程控制**——窗口最脆，随时可能出现且随时可能被掐断 | CANN toolkit 版本窗口内核对；容器多为 **aarch64（鲲鹏）** | **M5-G4 首测**：torch_npu parity + NPU 带宽/算力 probe + cann-rs probe（内核不动工） | 1.5–3h（按时长限制收缩，优先级 T1>T2>T0>T3） |
| W5 | **vGPU-32GB（4080S 16G→32G 改装）** | 与 R1 同款，随时可得——**本轮插队窗口** | 驱动 595.71.05、CUDA 13.2、sm_89、32GB；**无功能性 Vulkan ICD（R1 定案）→ GPU 路线仅 CUDA/torch** | ① simulate.cell.v1 跨机字节级 parity；② 批次 G cellpose pack GPU 冒烟；③ AIVC 可行性探针（scvi-tools 小模型推断） | 2–3h |
| W6 | **本地低档机：NAS / Intel G7400（2C/4T）** | ✅ **2026-10-04 已执行**（接入见 `docs/proposals/local-low-spec-verification-plan.md` §四） | 实测**有两块 GPU**：UHD 710 核显 **+ GTX 750 Ti 独显**（均为 Vulkan ICD）；R 缺失 → r 腿待批 | ① rust/python 同机对照（大/小两档）✅；② `simulate.cell.v1`/`sequence.ssr.v1` 跨机字节 parity ✅；③ 双 GPU 低端回退档 `gpu-bench` ✅；④ 顺手抓到并修掉 wgpu dispatch 越界崩溃 | 实际 3 轮共 ~25 min |
| — | AMD RX 9060/9070 XT | **无公开租赁平台**——只能找人协助代测 | ROCm/HIP 官方栈 + wgpu/Vulkan 覆盖 | 非窗口：**协助测试工具包**（D7），第三方代跑回传产物 | — |
| — | 砺算等 | 观察档 | OpenCL+Vulkan | 不在本轮（M5-G3 后置项） | — |

预订只看技术配置（机型/显存/驱动/架构），渠道与时价不入库（沿 e0991cb 口径）。
**执行顺序按准备度而非可得性**：NVIDIA/S4000 随到随测；昇腾窗口一旦出现只剩
1.5h 且可能无 SSH——因此 §3 的 D3/D6/D8（parity 工具、aarch64、无 SSH 预案）
**全部就绪后才允许接 W2**；AMD 不进窗口序列，工具包先行。

## 2. 既定事实与本轮假设

- cudarc-musa：官方 fork，按 **MUSA 5.2 driver ABI** 构建（ROADMAP §9.5）；S4000 现货镜像是
  驱动 2.7.0 + toolkit 3.1.0 → **ABI 错位是 W1-T1 第一检查点**（R2 遗留）。错位成立时的替代
  路线已验证：mcc 原生 C 内核（R2/T3 一次点火）。
- S4000 Vulkan：CreateInstance 驱动内失败（R2，loader/strace 证据已留档），嫌疑 toolkit 3.1.0
  vs 驱动 2.7.0 错位 → W1-T2 仅在新镜像（SDK 5.x / 新驱动）出现时重试，否则维持结论。
- 昇腾：官方零 Rust；社区 cann-rs（2026-08 活跃）、ruda-driver-cann（§9.5）。架构事实：
  国产卡 CUDA 兼容均为源码级重编译，**内核二进制不跨厂商**，共享层 = host driver FFI + WGSL。
- 昇腾主战场现实路线：**torch_npu（PyTorch NPU 后端）** 跑矩阵类计算 → 与 CPU 对照出
  NPU 数据点；ACL/Ascend C 自定义内核链路重（bindgen + bisheng 工具链），本轮只做 probe
  级 spike，不动工内核。
- gpu-lab `GpuBackend` 抽象、`MemoryModel::{Discrete, Unified}`、aarch64 交叉编译检查已就绪
  （68afe08，2026-10-01）。

## 3. 开发内容（窗口前仓库内工作，全部 feature-gated / scripts 隔离）

| # | 项 | 位置 | 内容 | 验收 |
| --- | --- | --- | --- | --- |
| D1 | MUSA 内核移植包 | `scripts/rental/musa/` | pearson v2（R3 已有）→ 补 **v4 合并访存版** 与 **k-mer 计数** 的 `.mu` 源码 + 编译/运行脚本（mcc，`-lmusart`，无架构旗标——R2 口径） | 脚本本地可 dry-run；正确性门同 R3（hist 整数精确、pearson 相对误差 <2e-4） |
| D2 | cudarc-musa 接线预案 | `gpu-lab/src/backends_musa.rs` | stub → 预案注释补齐：probe 顺序（`musa-smi` → dlopen `libmusart`）、musaMalloc/Memcpy/LaunchKernel 映射表、错误分类（ABI 错位 vs 设备缺失）；**不改 CI**（feature `gpu-musa`） | `cargo check -p gpu-lab` 绿（stub 保持编译）；映射表评审通过 |
| D3 | torch_npu parity 工具 | `scripts/rental/ascend/` | `npu_parity.py`：① pearson 4000×1000 相关矩阵 NPU vs CPU；② SVD/PCA（符号规范化后对照，沿 M3 §8.1 口径）；③ k-means（固定种子）；④ copy 1GiB 带宽 + matmul 4096³（f16/f32）probe | 双输出 JSON（npu/cpu）+ 判据内建（pearson \|Δ\|≤1e-5、PCA 逐分量一致）；README 记录 torch/torch_npu/CANN 版本矩阵检查法 |
| D4 | cann-rs spike 预案 | `gpu-lab/src/backends_ascend.rs`（新 stub，feature `gpu-ascend`） | 只做设备枚举级 probe 设计（`libascendcl.so: aclrtGetDeviceCount`）；cann-rs / ruda-driver-cann 可用性核对清单；**内核不动工**（M5-G4 口径） | stub 编译绿；`describe()` 说清 spike 范围 |
| D5 | k-mer GPU 内核（首批新内核，G1 门控） | `gpu-lab/`（WGSL 规范源）+ MUSA C 版（D1） | k≤31、2-bit 编码、atomic 计数谱；CPU 参考实现在 gpu-lab 内（沿 bench 套件先例）；**生产化 promotion 不在本轮** | WGSL + `.mu` + CPU 三方对同输入：谱完全相等（整数精确门） |
| D6 | aarch64 就绪 | `scripts/rental/RUNBOOK*` | W2 容器若为鲲鹏：有 SSH → rustup aarch64 工具链 + 机上构建时限预估；**无 SSH 档走 D8 预编译件**，机上构建降级为对照数据点；gpu-lab aarch64 交叉检查结论引用 | RUNBOOK 更新入库 |
| D7 | AMD 协助测试工具包 | `scripts/rental/amd/` | **无公开租赁平台 → 第三方代测包**：wgpu probe + gpu-bench（Vulkan 覆盖路线）+（代测机有 ROCm 时）`.hip` 同源内核编译运行脚本；自足式 README（依赖、命令、产物命名、回传方式） | 干净 Linux 机器按 README 可独立复跑；产物字段与台账格式一致 |
| D8 | 无 SSH 预案：预编译 aarch64 件进 bundle | `scripts/prepare-rental-bundle.py` | bundle 追加 `aarch64/linxira-bio-cli`（`--target aarch64-unknown-linux-gnu` 交叉产物）+ `scripts/rental/ascend/`；昇腾 Web 终端/Jupyter 档：上传 bundle → 解包 → 直接跑二进制与 python 工具，**不依赖机上 rustup/cargo** | `--check` 含新件；交叉二进制过一次 qemu-user 或 CI 冒烟 |
| D9 | 批次 G 前置：cellpose pack 骨架（插队） | `workflows/org.linxira.cellpose-segmentation/` | manifest（`gpu: required` + `minimum_vram_mb: 8192`）+ 依赖锁（torch CUDA 轮子口径）+ 内置样例图 + tests；**CPU 路径单测先行**（CI 三平台绿），GPU 路径留 W5 窗口验收 | pack 冒烟测试绿（CPU 路径）；W5-T2 GPU 冒烟通过 |

红线沿用：内核受 benchmark 门控（无 CPU 基线不写内核）；数值一致性判据沿 §7.1；
`LINXIRA_BIO_GPU` 披露字段预留。

## 4. 测试内容（逐窗口 T 清单，1–3h，结果回填 `docs/engine-evals/` 新台账）

执行顺序：W1/W0 随到随测；**W2 是稀缺窗**，预案就绪即接、出现即按收缩清单执行；W4 搭车。

### W1 — S4000（摩尔主线，3h）
| # | 测试 | 判据 | 记录物 |
| --- | --- | --- | --- |
| T0 | 镜像/驱动/SDK 版本核对 + `cargo build --release -p linxira-bio-cli`（仪器化计时，对照 R2 的 47.3s） | audit exit 0；版本表入台账 | 版本快照 |
| T1 | **cudarc-musa 首测**：git 依赖接入 scratch crate → hello-compute → pearson v2 发射对照 | 任一结果都算验收：成功 → pearson \|Δ\|≤1e-5 且分相计时 vs R3 C 版（49.0 GB/s）；失败 → 错误原文 + ABI 错位定案（上游 issue 素材） | build log + bench JSON |
| T2 | Vulkan 复查（仅当镜像含 SDK 5.x/新驱动）：vkCreateInstance 重试 | 复现 → 维持 R2 结论；通过 → 记录新证据链 | strace/loader 摘录 |
| T3 | k-mer 内核（D1/D5）：`.mu` 版 vs CPU 参考同输入 | 谱逐桶相等；1M/100M reads 档计时入表 | bench JSON |
| T4 | pearson v4 合并访存版 vs v2：同卡对照 | 正确性门同 R3；吞吐提升幅度记录（比值预期不变口径核对） | bench JSON |

### W2 — 昇腾 910B / 910C / 910（NPU 首测；稀缺窗，1.5–3h 按时长限制收缩）

> 接入两档：完整 SSH → 跑全清单；**低价档仅 Web 终端/Jupyter（无 SSH、禁远程控制）** →
> 走 D8 预编译 aarch64 二进制 + bundle 上传，T0 构建项改为「解包 + `--version` 冒烟 +
> `npu-smi`/CANN 版本快照」，执行顺序收缩为 **T1 → T2 → T0 快照 → T3**。

| # | 测试 | 判据 | 记录物 |
| --- | --- | --- | --- |
| T0 | `npu-smi info` + CANN 版本 + 架构判定（aarch64/x86）+ cargo 构建（D6） | 版本表 + 构建时长入台账；aarch64 构建即验收项之一 | 版本快照 |
| T1 | **torch_npu parity**（D3）：pearson / PCA / k-means | pearson \|Δ\|≤1e-5；PCA 符号规范化后逐分量一致；k-means 同种子同划分 | npu/cpu 双 JSON |
| T2 | NPU 物理力 probe：copy 1GiB 带宽、matmul 4096³ TFLOPS（f16/f32 分列） | 数值入表（对标 G2 档），注明内存模型（Discrete） | bench JSON |
| T3 | cann-rs / ruda-driver-cann probe：设备枚举级 | 枚举成功与否都算验收；内核不动工；结论进 M5-G4 台账 | build log |
| T4 | （若同时租到 910 与 910C）同 T1 缩水版 + CANN 版本差异记录 | 跨代 parity 一致性 | 台账 |

### W0 — NVIDIA 真卡 V100 / T4（随时插空，2h/次）
| # | 测试 | 判据 | 记录物 |
| --- | --- | --- | --- |
| T0 | 构建 + audit | exit 0 | 版本快照 |
| T1 | cudarc（NVIDIA 主线）hello + pearson v2 | sm_70/75 上成功 → 与 W1/R3 同表对照；失败 → 错误原文 | bench JSON |
| T2 | **wgpu probe + gpu-bench 首个成功点**：pearson v1/v2/v3 + 直方图 | probe 列出真卡；全 kernel pass（直方图整数精确、pearson \|Δ\|≤1e-5）；分相中位数入表——补齐"真卡数据中心驱动 Vulkan 可用"对照点 | gpu-bench-<card>.json |
| T3 | k-mer CUDA 版（`.cu` 仅扩展名不同的同源内核） | 谱逐桶相等 + 计时 | bench JSON |

### W5 — vGPU-32GB（4080S 16G→32G 改装；插队窗口，2–3h）

> R1 同款 vGPU 家族（驱动 595.71.05、CUDA 13.2、sm_89、无功能性 Vulkan ICD）→
> GPU 路线仅 CUDA/torch；本窗验证两条面试主线：**机制细胞模拟跨机确定性** +
> **AIVC（批次 G 包装范式）可跑**。

| # | 测试 | 判据 | 记录物 |
| --- | --- | --- | --- |
| T0 | 构建 + audit + torch CUDA 探测（`torch.cuda.is_available()` + 设备名/SM） | exit 0；torch 识别 sm_89 设备与 32GB 显存 | 版本快照 |
| T1 | **simulate.cell.v1 跨机 parity**：同 seed/模型本地与本卡各跑 | 两机 JSON/TSV **字节级一致**（跨机确定性第 6 个数据点） | 双机产物 + sha256 |
| T2 | 批次 G cellpose pack GPU 冒烟（依赖 D9） | 内置样例图跑通，掩码结果产出；同输入复跑掩码一致 | pack 结果 JSON |
| T3 | AIVC 探针：scvi-tools 小模型推断 ×3（开 deterministic 标志） | 推理成功且 3 次输出一致；记录 VRAM 峰值与 wall | 推理日志 |

### W4 — vGPU-48GB（搭车，可选）
48GB=48GB 与 S4000 显存孪生：大矩阵 pearson（4000×4000 级）显存驻留对照 + wgpu 第二 sm_89 点
（预期仍无 Vulkan ICD，作 vGPU 家族结论第 4 实例）。

## 5. "其他可被加速的计算"候选清单（开发排期输入，按可行性分档）

| 档 | 计算 | 形态 | 依据 |
| --- | --- | --- | --- |
| 已完成 | copy/triad/hist256/pearson v2（双厂 C 同源） | 带宽型 | R3 表 1 |
| A 高（带宽型，直移植） | **k-mer 计数谱**（D5，本轮首发）；FASTQ 每周期质量直方图；序列 GC/长度统计；SSR/串联重复扫描；Hamming/p-distance 矩阵 | u32/atomic、分支少 | 与 hist256 同型；kmer/SSR 均有既有能力可对照（`sequence.kmer.count.v1`、`sequence.ssr.v1`） |
| B 中（矩阵型，**NPU 强项**） | PCA/SVD、k-means、相关矩阵（= pearson）、WGCNA 邻接加权 | GEMM/SVD 为主 | 910B 算力档收益最大；torch_npu 通道（D3）直接覆盖 |
| C 低（重 dev） | Smith-Waterman/WFA 比对内核；Vina 对接 GPU 化 | 重型/重构 | Vina 已证 CPU 多核物理上限（8 线程平坦）；比对内核 dev 量大，不进本轮 |
| D 次优先 | 序列 shuffle / bootstrap 置换检验（RNG 密集） | musa-rand / ACL RNG | 有既有能力（`sequence.shuffle.v1`）可对照；D5 之后排 |
| 不动工 | DL 推理（C6）、宏基因组分类器包装 | 另轨 | 与 M5-G 内核轨正交 |

## 6. 风险与预案

| 风险 | 预案 |
| --- | --- |
| cudarc-musa（5.2 ABI）对 S4000 老驱动 2.7.0 不兼容 | T1 按验收即数据记档；C 路线（mcc）已验证为替代主轴；上游 issue 素材归档 |
| 昇腾容器 CANN/torch_npu 版本不配 | 预订时**优先选预装 torch_npu 的镜像**；T0 版本矩阵核对先行；不配则只跑 T2/T3 |
| 鲲鹏 aarch64 构建超时/依赖缺 | 仅 `-p linxira-bio-cli`；rustup 走 rsproxy（R2 口径 45s）；gpu-lab 已过 aarch64 交叉检查 |
| 910 初代 CANN 版本过旧，torch_npu 不可用 | T4 缩水为 torch CPU + npu-smi 快照记录，跨代对照顺延 |
| W2 库存不稳 | W2 整窗可后置不阻塞 W1/W0——W1/W0 不依赖昇腾结果 |
| 昇腾低价档无 SSH / 禁远程控制 | D8 预编译 aarch64 件 + bundle 经 Web/Jupyter 上传执行；清单收缩 T1→T2→T0→T3；SSH 可用则跑全清单 |
| AMD 无公开租赁平台 | D7 工具包交协助人代测（wgpu/Vulkan 为主，ROCm 可选）；产物回传后代填台账；不占窗口预算 |
| W5 vGPU torch 版本匹配 | 优先 cu126/cu128 stable wheel（sm_89 全支持，驱动 595 > wheel 运行时即可）；失败则记录驱动/版本矩阵，退回 CPU parity + AIVC 探针顺延 |

## 7. 回填与发声

- 各窗结果回填 `docs/engine-evals/` 新台账（沿 round 1–4 格式：3 次取中位、分相、容差显式）。
- G5 红线不变：本轮量化测试全部完成后方可动笔下一篇技术文章（素材新增：NPU 轨首测、
  cudarc-musa 定案、wgpu 真卡首点、k-mer 内核跨厂 parity、simulate.cell.v1 跨机
  字节级 parity、cellpose/AIVC 首点）。
