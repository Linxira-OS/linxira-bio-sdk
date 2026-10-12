# 厂商 Rust/加速器支持状态复核 —— Intel / AMD / NVIDIA / 摩尔线程（2026-10-12）

> 起因：本仓库 `ROADMAP.md` §9.5（2026-09-29 记录）断言「**AMD：官方零 Rust 计算路径**」，
> 用户质疑该断言。本次按"外部声明必须有可点开出处"的口径重新核实，结论**修正 §9.5 三处**，
> 并撤回一条无法找到公开出处的声明。核实手段：GitHub 组织/仓库/代码检索（`org:`、`language:`
> 限定）、各仓库 README 一手文本、crates.io 与 docs.rs 页面。

## 1. 结论速览

| 厂商 | 官方 Rust 是否存在 | 能否**编写内核** | 一手出处 |
| --- | --- | --- | --- |
| NVIDIA | 是（官方博客 2026-09-08 双轨） | 是（官方自述早期，未生产可用） | developer.nvidia.com 官方博客 |
| Intel | 是（三条：推理绑定、SYCL 绑定、插桩绑定） | 是（`sycl-rs` 可提交内核，experimental） | `intel/openvino-rs`、`oneapi-src/oneapi-rs`、`intel/ittapi` |
| AMD | 是（主机侧：ROCm 组织 5 个 Rust 仓库 + `amdsmi` 官方 Rust crate） | **否**（内核/HIP 绑定仅社区） | `ROCm/spur`、`ROCm/rocm-cli`、`ROCm/amdsmi` |
| 摩尔线程 | **未能核实** | 未能核实 | 见 §5（撤回声明） |

## 2. 检索方法（可复现）

- 组织级：`org:ROCm language:Rust`、`org:amd language:Rust`、`org:intel language:Rust`、
  `org:oneapi-src language:Rust`、`org:MooreThreads language:Rust`。
- 代码级：在具体仓库内检索 `musa`、`rust`（如 `repo:chelsea0x3b/cudarc`、`repo:ROCm/amdsmi`）。
- 命名级：`cudarc musa`、`musa language:Rust`、`amdsmi rust` 等关键词组合。
- 一手文本：相关仓库 `README.md`（本次逐字引用其自述状态）。

## 3. Intel（原记录只提 `sycl-rs`，实际少记了两条）

- **`intel/openvino-rs`** —— 官方组织 `intel/` 下的 OpenVINO™ Rust 绑定，Apache-2.0，最近推送
  2026-10-09。crate 分层：`openvino`（高层推理）、`openvino-sys`（C API FFI，bindgen）、
  `openvino-genai` / `openvino-genai-sys`（LLM/VLM/Whisper 管线）、`openvino-finder`（库发现）。
  OpenVINO 本身面向 CPU/GPU/**NPU**，故这是"官方 Rust 触达 NPU"的实际路径。
- **`oneapi-src/oneapi-rs`** —— Intel 的 oneAPI 开源组织（`oneapi-src`，Hillsboro OR，
  webadmin@linux.intel.com）下，crate 名 `sycl-rs`；README 自述"Rust bindings for SYCL and the
  Intel oneAPI programming environment"，示例包含 **kernel launch**（分配、运行时内核编译、
  提交、回拷）；同时明示 "experimental, has not released version 0.1.0, and is not ready for
  production use"，测试配置仅 Linux + oneAPI Toolkit 2026.1。
- **`intel/ittapi`** —— Intel® Instrumentation and Tracing Technology (ITT) 与 JIT API 仓库，
  主语言 Rust，最近推送 2026-10-07；即官方性能插桩的 Rust 绑定。

**修正**：原记录"Intel 有官方 `sycl-rs`（实验性）"**不完整**——推理侧已有持续维护的官方绑定
（`openvino-rs`），且插桩侧也有官方 Rust 工件。

## 4. AMD（原记录"官方零 Rust 计算路径"表述错误）

`org:ROCm language:Rust` 命中 **5 个官方仓库**（均非内核/计算绑定，属主机侧工具与系统软件）：

| 仓库 | 定位 | 星标 | 最近推送 |
| --- | --- | --- | --- |
| `ROCm/spur` | "An AI-native job scheduler written in Rust"，Slurm CLI/REST/C FFI 兼容 | 132 | 2026-10-10 |
| `ROCm/rocm-cli` | 面向用户的 AMD GPU 本地 AI CLI/TUI，README 自述 "Tech Preview"，锁定 Rust 1.96.0 | 45 | 2026-10-10 |
| `ROCm/spur-cloud` | 基于 Spur 的 GPU 即服务平台 | 7 | 2026-07-01 |
| `ROCm/axis` | Agent 执行隔离沙箱运行时 | 6 | 2026-10-09 |
| `ROCm/cm` | CMake 工程配置/构建/测试前端 | 4 | 2026-10-08 |

此外 **`ROCm/amdsmi` 内含 `rust-interface/`**：官方 Cargo 包 `amdsmi`（bindgen 生成绑定 + 示例
`amdsmi_exporter`），即 GPU 管理/遥测的官方 Rust 绑定。

**内核/计算侧仍无官方路径**（社区项目，非 AMD 托管）：`RustNSparks/rocm-rs`（ROCm 库安全封装）、
`Reis-McMillan/rocmrc`（"based off cudarc"的 ROCm 封装）、`tracel-ai/cubecl-hip-sys`（CubeCL 的
HIP 运行时绑定）。

**XDNA NPU**：`amd/xdna-driver`（官方，C++/Python）未见 Rust；`org:amd language:Rust` 命中 **0**。
第三方 `yijunyu/tile-rs` 自称把 Rust 内核经 MLIR 编到多后端（含 Ryzen AI、CUDA、MUSA），但为
第三方项目，非厂商支持范围。

**修正**：应表述为"AMD **主机侧**官方大量使用 Rust（调度器、CLI、管理绑定），**但无官方 GPU/NPU
内核编写路径**；内核侧仍属社区"。

## 5. 摩尔线程（**撤回**原记录的 `cudarc-musa` 声明）

原记录（ROADMAP §9.5，2026-09-29）："2026-09-23 官方发布 `cudarc-musa`（cudarc 官方 fork，MUSA 5.2
driver ABI，crates.io 发布准备中）——国产厂商第一个官方 Rust GPU 工件"。本次核实**找不到任何公开
出处**：

- `repo:chelsea0x3b/cudarc` 内检索 `musa`：**0 命中**；该仓库 issue 检索 `musa`：**0 命中**；
  crates.io `cudarc`（0.19.10）自述为 "Safe and minimal CUDA bindings"，无 MUSA 说明。
- `org:MooreThreads language:Rust`：**0 命中**（该组织公开仓库以 Python/C++/Go 为主，如
  `torch_musa`、`vllm-musa`、`mate`、`tutorial_on_musa`）。
- 关键词 `cudarc musa`、`musa rust`、中文检索均未命中对应官方发布物；命中的 `musa language:Rust`
  结果均为无关同名项目或第三方实验（如 `ZhaoShanBiune/musapy`、`InfiniTensor/mudrv`（2024 年停更））。

**易混淆点（必读）**：`gpu-rental-round2-s4000-2026-10-02.md` 写道"cudarc-musa 的**前置可行性**当场验证"——
它验证的是**前提**（S4000 上 `musaMalloc/musaMemcpy/musaLaunchKernel` 与 CUDA 同构，因此"一份 cudarc 式绑定
在 MUSA 上可行"），**并非**验证了该绑定本身存在或跑通。前提可行 ≠ 工件存在，两者不可互相替证。

**处置**：该声明从 ROADMAP 与文章撤下，改为"厂商渠道声明，无可核实公开链接"。MUSA 生态侧**可核实**
的现状是：官方提供 PyTorch (`torch_musa`)、vLLM (`vllm-musa`)、算子库 (`mate`)、MUSA SDK/工具链
（`mcc`，clang-14 派生）等 **C++/Python** 路径；Rust 侧暂无官方工件。

## 6. 与本仓库实测证据的关系（避免混谈）

- **已执行**：`docs/engine-evals/gpu-rental-round3-dual-vendor-2026-10-02.md` —— 同一份 **C/C++**
  内核套件（`bench_suite.mu`，仅扩展名不同）分别由 `nvcc`（RTX 4080S，sm_89）与 `mcc`（MTT S4000）
  编译，median-of-3，逐项正确性门全过。这是"单源内核、两家原生工具链"的证据。
- **未执行**：Rust 工具链路径。`docs/engine-evals/gpu-rental-2026-10-02.md` T3 记录 `cutile-rs`
  当时不在 crates.io（仅 `cutile 0.0.0-alpha`），依赖链与驱动门槛使其**未进入执行**，`gpu-lab` 侧
  保持 feature-gated stub；MUSA 侧 Rust 绑定从未进入实测（`cloud-rental-plan-round5-2026-10-04.md`
  T1 仍为计划态）。

因此对外只能声明"**原生 C/C++ 单源内核跨两家可复现**"，不能声明"一份 Rust 源码已跨两家跑通"。

## 7. 后续动作

- [x] 修正 `ROADMAP.md` §9.5 的 Intel / AMD / 摩尔线程三处表述（本台账为依据）。
- [x] 修正博客中英稿 §14 段落与 §16 文献表。
- [ ] 若厂商获得公开稳定链接（尤其摩尔线程 Rust 工件），按本台账 §2 方法复核后回填。
