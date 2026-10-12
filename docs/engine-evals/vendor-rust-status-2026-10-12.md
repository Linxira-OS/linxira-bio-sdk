# 厂商 Rust 支持状态复核 —— NVIDIA / Intel / AMD / 摩尔线程（2026-10-12）

> 目的：查清"GPU/NPU 编程里，哪些**真能用 Rust 写**、哪些**必须走 C++**"，并作为本仓库 GPU 轨
> 选型与对外声明的唯一依据。
>
> 起因与自我更正：`ROADMAP.md` §9.5 原记"摩尔线程 2026-09-23 官方发布 `cudarc-musa`"。本台账
> **第一版把该声明误判为"查不到出处"并撤回**——错的。复核方法本身有盲点（见 §2），实际两个官方
> fork 都在，本文 §6 恢复原结论并补齐证据；§7 同时记录本仓库自己的真实状态。

## 1. 结论矩阵（要的就是这张表）

三个层次必须分开：**① Rust 能否驱动/调用 GPU**（host 绑定）、**② Rust 能否编写内核**（device 代码）、
**③ 什么仍必须 C++**。

| 厂商 | ① Rust 调 GPU（host） | ② Rust 写内核（device） | ③ 仍需 C++ 的部分 | 一手出处 |
| --- | --- | --- | --- | --- |
| **NVIDIA** | ✅ 官方运行时 crate：`cuda-bindings`、`cuda-core`（含 `cuda-core-derive`）、`cuda-async` | ✅ **官方支持**：`cuda-oxide`（SIMT；自研 rustc codegen 后端 → PTX；固定 nightly）与 `cutile-rs`（Tile DSL；proc-macro；stable Rust 1.89+；JIT → Tile IR → cubin） | 成熟产量路径仍是 CUDA C++；官方自述 "early-stage research project… expect API breakage" | `NVIDIA/cuda-rust`（3694★，2026-10-11 推送）；2026-09-08 官方博客 |
| **Intel** | ✅ `intel/openvino-rs`（推理，覆盖 CPU/GPU/**NPU**；含 `openvino-genai`）；`oneapi-src/oneapi-rs`（SYCL 运行时）；`intel/ittapi`（插桩） | ❌ **不能**：`sycl-rs` 的内核是**内嵌 C++ SYCL 源码字符串**（官方示例 `IOTA_SRC`，经 `create_kernel_bundle_from_source` 交给 DPC++ 编译） | **内核体**（C++ SYCL，DPC++ 编译）；`sycl-rs` 自述 experimental、**未发布 0.1.0**、仅 Linux + oneAPI 2026.1 测试配置 | `oneapi-src/oneapi-rs`（`sycl/sycl-rs/examples/kernel_launch.rs` + README）；`intel/openvino-rs` |
| **AMD** | ✅ 但**仅主机/管理**：`ROCm/amdsmi` 自带官方 `rust-interface/` crate；`ROCm` 组织下 5 个 Rust 仓库（`spur` 132★ 调度器、`rocm-cli` Tech Preview、`spur-cloud`、`axis`、`cm`）。**无任何计算/内核绑定** | ❌ 官方无。**第三方可**：`CubeCL`（Rust `#[cube]` 内核 → LLVM → PTX/AMDGPU；HIP 后端）/ `wgpu`+SPIR-V | HIP C++ 内核；官方无 HIP Rust 绑定（社区 `rocm-rs`、`rocmrc`、`cubecl-hip-sys`）；**XDNA NPU 各层均无 Rust**（官方 `amd/xdna-driver` C++/Python） | `org:ROCm language:Rust`（5 库）+ `fork:true` 复核；`tracel-ai/cubecl` |
| **摩尔线程** | ✅ **官方两个 fork**：`cudarc-musa`（Driver 层，feature **`musa-50200`** = MUSA 5.2 Driver ABI；示例 `musa-driver-vector-add`）；`sppark-musa`（crate `sppark` 的 **`musa`** feature；`MCC`/`MUSA_ARCH=mp_31`；另带 Go 桥） | ❌ 内核仍是 C++/MUSA（`.musa`/`.hpp`，由 `mcc` = clang-14 派生编译） | **内核体**（MUSA C++） | `MooreThreads/cudarc-musa`、`MooreThreads/sppark-musa`（README + `rust/Cargo.toml`） |

**一句话判据**：**"用 Rust 写 GPU 内核"目前只有 NVIDIA 有官方支持**（Rust → PTX / Tile IR → cubin）；
Intel、AMD、摩尔线程的官方 Rust 都停在 **host 层**，内核体一律是 C++ 家族（SYCL C++ / HIP C++ /
MUSA C++）。想在 AMD 上写 Rust 内核，当前唯一现实路径是第三方 `CubeCL`（alpha，Burn 在用）或
`wgpu`/SPIR-V；Intel 侧同理走 Vulkan/SPIR-V。

## 2. 检索方法与**三个盲点**（本台账第一版就栽在这里）

正确做法（缺一不可）：

1. **必须显式包含 fork**：`org:<X> fork:true`。GitHub 仓库搜索**默认排除 fork**，而厂商的适配层
   常常正是 fork（`cudarc-musa`、`sppark-musa` 都是）。
2. **不要信 `language: Rust`**：该限定只反映**主语言**。`sppark-musa`（主体 Cuda/C++，Rust 绑定层）
   不会出现在 `language:Rust` 结果里；`cudarc-musa` 同理。
3. **org 级代码搜索不可靠**：`org:MooreThreads Cargo.toml` 返回 0 命中，但仓库内确有 `rust/Cargo.toml`。
   代码搜索不是仓库清单工具——**枚举清单要用 `search_repos`（含 fork），再逐仓库读 README/清单**。
4. 辅以命名级检索：`org:<X> <keyword>`（如 `musa`、`rust`）、crates.io/docs.rs 页面、各仓库 README
   逐字引用。

**教训**：一次"查不到"只能说明**检索式不完整**，不能直接推"不存在"；对外声明前若必须下"不存在"的
结论，至少要做 fork 包含 + 主语言突破 + 清单枚举三重检查，并在台账写明检索式。

## 3. NVIDIA（官方，且唯一能"Rust 写内核"）

`NVIDIA/cuda-rust` 是一个仓库承载三层：

- **运行时（host）**：`cuda-bindings`（driver API 绑定）、`cuda-core`（context/stream/内存/事件/发射）、
  `cuda-async`（惰性算子、future、图）。
- **`cuda-oxide`（SIMT）**：自研 rustc codegen 后端 + cargo 辅助工具，AOT 编译 Rust → PTX；需要
  **固定 nightly**（`cargo oxide` 管理）；能直接控制线程/共享内存/TMA/cluster。
- **`cutile-rs`（Tile）**：proc-macro 式瓦片 DSL，Rust **stable 1.89+**；首次发射时 JIT（Rust → Tile IR
  → cubin），由编译器决定瓦片到架构的映射。
- 两种模型的内核**可互操作**（同一 stream、共享 device buffer）；两者都把 Rust 的所有权规则延伸过
  launch 边界。
- 定位：官方自述早期研究项目，允许 breakage；与"成熟 CUDA C++"相比是新轨。

## 4. Intel（host 很完整，内核仍是 C++ SYCL）

| 仓库 | 层 | 状态 |
| --- | --- | --- |
| `intel/openvino-rs` | 推理（host）；OpenVINO 本身覆盖 CPU/GPU/**NPU** | Apache-2.0；crate：`openvino`、`openvino-sys`、`openvino-genai`/`-sys`、`openvino-finder`；持续更新 |
| `oneapi-src/oneapi-rs` | SYCL 运行时（host）+ **内核提交** | `sycl-rs`；示例含 `kernel_launch`（分配→运行时内核编译→提交→回拷）；**内核源为 C++ SYCL 字符串**；自述 experimental、未发布 0.1.0、仅 Linux + oneAPI 2026.1 |
| `intel/ittapi` | 性能插桩（host） | 官方 ITT/JIT API，主语言 Rust |

**对内核的含义**：Intel 目前**没有**"用 Rust 写内核"的官方路径；Rust 能做的是准备好缓冲区、提交
一个由 DPC++ 编译的 C++ SYCL 内核。想纯 Rust 内核在 Intel GPU 上跑，只能靠第三方 SPIR-V 路线
（`wgpu`/`CubeCL`，见 §1）。

## 5. AMD（主机侧真用 Rust；内核侧零官方）

`org:ROCm language:Rust` = 5 个仓库，全部**非**计算绑定：`spur`（Slurm CLI/REST/C FFI 兼容的 AI
作业调度器，132★）、`rocm-cli`（AMD GPU 本地 AI CLI/TUI，官方自述 Tech Preview）、`spur-cloud`、
`axis`、`cm`；另 `ROCm/amdsmi` 内含官方 `rust-interface/`（Cargo 包 `amdsmi`，bindgen + 示例）。
`org:amd fork:true language:Rust` 仅 `amd/tock`（嵌入式 RTOS fork，与 GPU 无关）。

- **无官方内核路径**；HIP 的 Rust 绑定均为社区（`rocm-rs`、`rocmrc`、`cubecl-hip-sys`）。
- **XDNA NPU**：官方用户态 `amd/xdna-driver` 是 C++（另有 Python 侧工具），无 Rust；第三方
  `yijunyu/tile-rs`（Rust 内核 → MLIR → 多后端，声称含 Ryzen AI）尚未被我们验证。
- 所以 AMD 若要用 Rust：host 走 wgpu/Vulkan（可移植层）或社区 HIP 绑定；**想写 Rust 内核走 `CubeCL`**。

## 6. 摩尔线程（**恢复并补充**：确实有官方 Rust 工件）

| 仓库 | 上游 / 版本 | Rust 集成 | 时间 |
| --- | --- | --- | --- |
| `MooreThreads/cudarc-musa` | fork 自 `cudarc` **v0.19.8**（commit `e1ab1e5`） | feature **`musa-50200`** 选择 MUSA 5.2 Driver ABI，动态链接 MUSA driver 库；`MUSA_HOME`/`MUSA_DRIVER_LIB` 环境变量；示例 `musa-driver-vector-add`（"a small driver example and a **MUSA device kernel**"）；MUSA 路径**仅 driver 层**，CUDA 库包装不静默映射 | 2026-09-23 |
| `MooreThreads/sppark-musa` | fork 自 `supranational/sppark` **v0.1.15**（baseline `17278d7`） | crate `sppark` 的 features `cuda`/`rocm`/**`musa`**；`MCC`/`MUSA_ARCH`（默认 `mp_31`）；`ff/gl64_t.musa`（Goldilocks 域）；Go 桥 `SPPARK_BACKEND=musa` | 2026-09-24 |

**性质**：两者都是**厂商维护的 fork**（不是原创独立项目），提供**host 层 Rust 调用**；内核体是
C++/MUSA 模板，由 `mcc`（clang-14 派生）编译。国产厂商第一个官方 Rust GPU 工件——**原 §9.5 记录
成立**，措辞应为"官方维护的 fork，Rust 调用层 + MUSA C++ 内核"。

## 7. 我们自己的状态（关键，避免自欺）

| 项 | 事实 |
| --- | --- |
| 我们的 Rust MUSA 后端 | `gpu-lab/src/backends_musa.rs` = **stub**：`is_available() == false`，`upload`/`dispatch`/`readback` 全部返回 `Err("musa backend not yet wired")`，由 feature `gpu-musa` 门控 |
| 我们在 MUSA 上真正跑过的 GPU 代码 | **C/MUSA**：`scripts/benchmark/rental/bench_suite.mu`、`pearson_bandwidth.mu`（`#include <musa_runtime.h>`，`mcc -O3 … -lmusart`）——这是 F1/F2 两张表的来源 |
| 我们在 MUSA 机器上跑的 Rust | 我们的 **SDK CLI**（`linxira-bio`），做的是 CPU 任务（SSR parity、矩阵流水线、分子对接）——"在 MUSA 机器上跑 Rust" ≠ "用 Rust 跑 MUSA" |
| wgpu/Vulkan 路线 | MUSA 侧**失败**（驱动内部 `vkCreateInstance` 失败）；NVIDIA vGPU 侧同样失败（ICD 缺入口）——两侧记录一致 |
| `cudarc-musa` 实测 | **从未执行**（`cloud-rental-plan-round5-2026-10-04.md` 仍为计划态） |

所以：**"我们用 Rust 写的 MUSA GPU 推理"这个说法在本仓库没有对应实现**。若记忆来自真实经历，最可能
是上述"Rust SDK 跑在 MUSA 机器上"，或把 MUSA 侧的 **C/MUSA 内核**与 Rust 记混了。任何对外表述都必须
按本表分辨。

## 8. 后期开发方向（本次调研的落地结论）

**原则**：官方 Rust 优先；官方只有 host 层时，采用"**Rust host + C++ 内核工件**"的组合，并把工具链
门槛写进清单；不自造 DSL，不假设内核二进制跨厂商可移植（PTX ≠ MUSA ≠ SPIR-V）。

1. **NVIDIA 轨（唯一可全 Rust）**：`cuda-rust` 的 `cutile-rs`（stable）优先做瓦片式内核（Pearson/直方图），
   `cuda-oxide`（nightly）留给需要显式线程控制的场景；运行时统一走 `cuda-core`/`cuda-async`。仍按
   `sm_80+`、官方"early-stage"定性记录风险。
2. **摩尔线程轨**：`gpu-lab` 的 `gpu-musa` stub 按 §6 接 `cudarc-musa`（feature `musa-50200`），
   内核沿用既有 `bench_suite.mu`/`pearson_bandwidth.mu`（MUSA C++，`mcc`）；窗口期（S80 民用机或
   S4000 云租）一次跑通即向上游提 issue/PR（driver ABI 初始化、错误分类、文档缺口）。
3. **Intel 轨**：host 用 `openvino-rs`（推理）或 `sycl-rs`（自研内核提交）；内核体写 **C++ SYCL**，
   明确"这不是 Rust 内核"；`sycl-rs` 仅 Linux + oneAPI 2026.1，Windows 实测反馈即上游贡献。
4. **AMD 轨**：官方无内核路径 → 分两级：**(a) 可移植层** wgpu/Vulkan（WGSL，走旧卡 gfx803 与 RDNA4）；
   **(b) 若必须 Rust 内核**，评估 `CubeCL`（alpha，HIP 后端）作为实验轨，不进引擎依赖。XDNA NPU 不纳入
   本期（无 Rust 路径）。
5. **抽象层契约**（`gpu-lab/src/backend.rs` 的 `GpuBackend`）：必须显式建模"**内核来源**"——
   `RustKernel`（NVIDIA）vs `ForeignKernel{toolchain, arch, source}`（Intel/AMD/摩尔线程），
   并要求**清单里记录工具链版本 + 目标架构 + 源文件哈希**，否则跨机复现无法验证。
6. **对外声明纪律**：任何"某厂商支持/不支持 Rust"的句子，必须附本台账 §1 的三层区分与出处；检索式
   按 §2 的三重检查执行，不重犯第一版的错误。

## 9. 变更记录

- 2026-10-12 第一版：建成并修正 ROADMAP/文章，**误将摩尔线程 `cudarc-musa` 撤回**（检索式未含 fork）。
- 2026-10-12 第二版（本文）：加入 §1 矩阵与 §2 方法学，**恢复**摩尔线程两条官方 Rust 工件，补齐
  §7 我方状态与 §8 开发方向。
- 2026-10-12 第三版：§10 固化对外表述口径（Rust 编排 + 厂商原生内核，三层硬规则）；旧文
  `cloud-gpu-dual-vendor-validation`（站点稿 + 仓库镜像）补分层说明并**加勘误**（顶部勘误行、文首
  更新日志、版本与声明条目），数据与结论未变。

## 10. 对外表述口径（Rust 编排 + 厂商原生内核）

**硬规则：任何同时出现"Rust"与"GPU/内核"的句子，必须写明是哪一层。** 三层固定说法：

| 层 | 语言 | 归属 | 谁说这一层 |
| --- | --- | --- | --- |
| 编排层（读数据、建缓冲区、发射、取回、校验、写报告） | **Rust** | 我们的引擎/CLI | "我们的实现"默认指这一层 |
| 内核层（拷贝/triad/直方图/相关性/自定义算子） | **C/C++ 家族**，由**厂商原生工具链**编译：NVIDIA `nvcc`、摩尔线程 `mcc`、Intel DPC++（C++ SYCL）、AMD HIP C++ | 厂商工具链 | 除非 NVIDIA 轨（`cuda-oxide`/`cutile-rs`），否则**不写"Rust 内核"** |
| 可移植层 | **WGSL**（wgpu/Vulkan），host 侧 Rust | 跨厂商回退 | 标明"WGSL 内核，非 Rust" |

一句话模板（中）：
> GPU 内核由厂商原生编译器编译（NVIDIA `nvcc` / 摩尔线程 `mcc` / Intel DPC++ / AMD HIP C++），
> 我们的 **Rust 侧只做编排**：读数据、建缓冲区、发射、取回、校验。**"用 Rust 写内核"目前只有
> NVIDIA 有官方路径**（`cuda-oxide`/`cutile-rs`，早期研究项目），其余三家厂商的官方 Rust 都停在
> host 层。

一句话模板（英）：
> GPU kernels are compiled by the vendors' native compilers (`nvcc`, `mcc`, DPC++ or HIP C++);
> our **Rust side only orchestrates** — read data, allocate, launch, read back, verify. Writing
> kernels *in Rust* has an official path only on NVIDIA (`cuda-oxide`/`cutile-rs`, early-stage).

**已发表/在写文章的适用情况（2026-10-12 审计）**：

| 文章 | 现状 | 处理 |
| --- | --- | --- |
| `bio-sdk-reproducibility-benchmark`（2026-09-18） | 不涉及 GPU 内核，口径本就是"Rust 确定性引擎 + 受控外调原生工具" | ✅ 无需改 |
| `cloud-gpu-dual-vendor-validation`（2026-10-02） | 原稿从未说明内核语言，易被读成"Rust 写 GPU 内核" | ✅ 已补四处：摘要第 5 条、§五首段、§五末"本文口径"表、§六尾句限定"（C/C++，不是 Rust）"；**站点稿与仓库镜像均已同步**，并各加：顶部勘误行、文首「更新日志/Changelog」段（2026-10-12）、「版本与声明」勘误条目 |
| `three-language-benchmark-2026-10-11`（草稿） | §14 已带四家三层矩阵与"我们从未实测 `cudarc-musa`"声明 | ✅ 无需改 |
| 站点其它文章（arch-adoption、fork-pain-changelog-2026-09、omarchy-*、kde-*、ai-contribution-policy） | 未涉及 GPU 内核（出现的"内核"均指 OS kernel） | ✅ 无需改 |

**历史更正留痕**：`cloud-gpu-dual-vendor-validation` 的"同一份内核源码/两家编译器"结论**不变**
（仍是已执行、可复算的证据），变的只是**把语言归属写清楚**；当时（2026-10-02）两家都还不能用 Rust
写内核——NVIDIA 的 CUDA Rust 2026-09-08 才公布且未进入执行，摩尔线程的官方 Rust 工件（`cudarc-musa`，
2026-09-23）属 host 层。
