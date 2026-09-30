# M5-G3 厂商栈推进方案（提案）

> 状态：提案，未立项。本文衔接 ROADMAP §9.5 已定义的 M5-G0~G5 里程碑与 §3.1 批次 D
> （Pearson v3 → 厂商栈 host 抽象层 → sycl-rs 本机尝试），不重新立项、不改红线。
> 目标里程碑：**M5-G3**（厂商切入 + 每家至少 1 个上游 issue/PR）。前置门槛：
> M5-G2 wgpu 对照表已齐（docs/engine-evals/gpu-wgpu-2026-09-30.md：pearson v2 树归约
> 约 48×、quality-histogram 约 5-6×；口径 = 3 次取中位、upload/compute/readback 分相）。

## 0. 不变约束（引用既有定案，不新增规则）

- **benchmark 门控**：无 CPU 基线不写内核（§9.5 红线）；本文所有厂商栈内核都在
  M5-G1 CPU 基线（gpu-cpu-baseline-2026-09-29.md）之上对照。
- **双实现硬规则**（§9 精度策略节）：精度敏感且 GPU 可加速的计算，CPU（scalar+SIMD，
  f64 audit 档）与 GPU 实现均为必须交付项；厂商栈内核只是 GPU 实现的另一种 host。
- **测量口径**：median of ≥3 runs；GPU 分相计时（upload / compute / readback）；
  数值一致性容差逐指标显式声明（延续 pearson v2 的 7.42e-7 实测口径，f32 档）。
- **能力层边界**：capability 只包装开放权重、不写内核；GPU 内核全部活在 gpu-lab →
  engine 内核推进管线里。
- **Linux 优先工况**：正式基准以 Linux 复现为准；Windows 本机（Arc 130T）数据标注
  "临时值 + 上游贡献素材"。
- **CI**：windows-gnu 工具链是 CI 阵地（§9.5 前提）；厂商栈 SDK 不可入 CI 依赖，
  只允许特性开关下的编译隔离（见 §3）。

## 1. 逐厂商执行卡

同一对内核贯穿四家：**Pearson v3**（subgroup 归约 + 每组多对批次化，wgpu 档的
v2 树归约直系后续）与 **quality-histogram**（u32 精确直方图，已经是瓦片/ subgroup
友好的整数内核）。每卡执行流程一致：

```
环境审计（probe）→ 厂商栈 host 编译 → 同输入两内核 → CPU f64 audit / CPU SIMD /
wgpu 回退 / 厂商栈 四路对照 → 台账（3 次中位、分相、容差）→ 上游 issue/PR
```

### 1.1 NVIDIA —— cutile-rs（云租 RTX 4090，sm_89）

| 项 | 内容 |
| --- | --- |
| 硬件 | 云租 RTX 4090（sm_89，CUDA 13.3 推荐；§9.5：cutile-rs 仅 Linux、sm_80+） |
| 栈 | cutile-rs（瓦片 DSL，stable Rust 1.89+），Ubuntu 24.04 官方测试组合 |
| 内核移植 | Pearson v3：瓦片内 subgroup 归约映射 cutile tile 语义；histogram：整数原子/分桶 |
| 对照组 | CPU f64 audit（AVX2 实测 2.55× 档）+ wgpu 回退（RX 580/750 Ti Vulkan 档数据已有） |
| 旧卡边界 | 750 Ti 走 cudarc/CUDA 12.x（sm_50）**而非 cutile-rs**（需 sm_80+），两轨分开记 |
| 台账 | `docs/engine-evals/gpu-cutile-<日期>.md`，表式沿用 gpu-wgpu-2026-09-30.md |
| 上游产出 | 见 §2.3 |

### 1.2 摩尔线程 —— cudarc-musa（民购 S80 优先；标记最优先）

| 项 | 内容 |
| --- | --- |
| 硬件 | 民购 S80 16GB（¥1499 档）优先，KUAE 云租备选；MUSA 5.2 driver ABI |
| 栈 | cudarc-musa（官方 fork，2026-09-23 发布；§9.5 定为"国产最优先"） |
| 内核移植 | host 侧走 cudarc 同形 API（fork 兼容），内核源码级重编译（cu-bridge/musify 语义） |
| 对照组 | 同 §1.1；另与官方 Vulkan 1.3 驱动的 wgpu 回退对照（S80 Vulkan compute 已被 ollama-musa 实用） |
| 首批窗口 | crates.io 发布准备中即窗口期：环境问题、文档缺口、ABI 假设都是 issue 素材 |
| 台账 | `docs/engine-evals/gpu-musa-<日期>.md` |
| 上游产出 | 见 §2.2 |

### 1.3 Intel —— sycl-rs（本机 Arc 130T，零租用成本）

| 项 | 内容 |
| --- | --- |
| 硬件 | 本机 Core Ultra 5 225H + Arc 130T（Xe-LPG+，DPAS）；oneAPI 2025.3 全套已装 |
| 栈 | oneapi-src/oneapi-rs 的 sycl-rs v0.1.0（官方声明 experimental，仅 Linux 测试配置） |
| 本机定位 | **Windows 非官方配置实测 = 上游贡献本身**（§9.5 定案）；数据以"临时值"标注 |
| 内核移植 | SYCL kernel：Pearson v3 用 group/sub-group API，histogram 用 device 累加器 |
| 对照组 | 本机 CPU（H1 AVX2+FMA 实测档；本机未暴露 AVX-512，H2 留云租）+ wgpu Vulkan 回退 |
| 台账 | `docs/engine-evals/gpu-sycl-<日期>.md`（Windows 行单独标注） |
| 上游产出 | 见 §2.1 |

### 1.4 AMD —— ROCm/HIP（云租 RX 9060/9070 XT；无官方 Rust 是既定事实）

| 项 | 内容 |
| --- | --- |
| 硬件 | 云租 RX 9060 / 9070 XT（RDNA4）；旧卡 RX 580（gfx803）官方 ROCm 不支持，走 Vulkan 档 |
| 栈 | 无官方 Rust 计算路径（§9.5：hip-rs 社区停更）→ 方案 = **Vulkan 持平层 + amdsmi 观测**：内核统一在 wgpu/Vulkan 基线层，ROCm 档用 HIP C++ 内核对照 + amdsmi 采集功耗/时钟 |
| 内核移植 | Pearson v3 / histogram 写一份 HIP C++ 对照内核（单独目录，不入 engine），验证 Vulkan 层数值与性能差距 |
| 对照组 | 同卡 wgpu（Vulkan）vs HIP C++（ROCm）vs CPU audit；差距即"持平层"结论 |
| 台账 | `docs/engine-evals/gpu-rocm-<日期>.md` |
| 上游产出 | 见 §2.4 |

## 2. 上游贡献路径表（每家至少 1 个 issue/PR 的具体提案）

### 2.1 Intel sycl-rs —— Windows 支持实测报告（issue）

- **仓库**：oneapi-src/oneapi-rs（sycl-rs v0.1.0）。
- **提案标题**：`Windows build/test report: sycl-rs v0.1.0 against oneAPI 2025.3 on
  Core Ultra 225H (Xe-LPG+ iGPU)`。
- **内容要点**：官方仅测试 Linux；本报告给出 Windows（MSVC + windows-gnu 两套目标）
  的编译结果矩阵、Level Zero loader 发现行为、build.rs 探测路径差异、最小可复现示例
  （与 gpu-lab probe 同构）、失败点最小重现代码。
- **时机**：本机 sycl-rs 首次跑通或明确失败后立即提（失败也是贡献：上游缺 Windows CI
  的证据）。

### 2.2 摩尔线程 cudarc-musa —— 首批窗口 issue

- **仓库**：摩尔线程官方 cudarc-musa fork（crates.io 发布准备中）。
- **提案标题**：`First-developer window report: cudarc-musa on S80 (MUSA 5.2) with a
  real workload (Pearson correlation kernel)`。
- **内容要点**：从 crates.io 占位/文档缺口入手——driver ABI 初始化在 MUSA 5.2 的实测
  差异、cu-bridge/musify 重编译流程对 Rust 侧 kernel 字符串的要求、S80 16GB 显存下的
  buffer 策略；附 gpu-lab 最小示例与分相计时。
- **时机**：拿到 S80 当周；首批窗口问题密度最高、响应最快。

### 2.3 NVIDIA cutile-rs —— sm_89 用例（issue 或 PR）

- **仓库**：NVIDIA 官方 cutile-rs。
- **提案标题**：`Consumer sm_89 workload example: tile-based Pearson correlation on
  RTX 4090`。
- **内容要点**：官方示例集中在数据中心卡；提交一个消费级 sm_89（4090）的完整瓦片
  内核用例——subgroup 归约在瓦片 DSL 的表达、occupancy 与瓦片形状取舍、与 CUDA C++
  基线的对照数字。若示例目录接受 PR 则直接 PR。
- **时机**：云租窗口内完成测量并提交。

### 2.4 AMD —— Vulkan 持平层数据反馈（issue，目标 wgpu/ash）

- **仓库**：gfx-rs/wgpu（AMD 官方无 Rust 栈，贡献对象转为社区栈）。
- **提案标题**：`RDNA4 vs Polaris compute parity data: same WGSL kernels, 3×
  performance span`。
- **内容要点**：同 WGSL（pearson v3 + histogram）跨 RX 580（gfx803）/ RX 9060 XT
  （RDNA4）的分相计时矩阵、subgroup 特性开关差异、已知 wgsl 特性在旧卡的行为——
  为 wgpu 的旧卡兼容叙事提供可复现数据。

## 3. Host 抽象层设计草案（§3.1 批次 D 第 2 项）

**目标**：一套 trait，同一内核逻辑在 wgpu / cutile-rs / cudarc-musa / sycl-rs /
HIP-C++ 对照间切换；厂商栈未就绪时自动回退 wgpu。

```rust
// gpu-lab 内原型，成熟后迁 engine（gpu:optional 能力层只消费 trait 对象）
pub trait GpuBackend {
    fn name(&self) -> &'static str;                       // "wgpu" | "cutile" | "musa" | "sycl"
    fn is_available(&self) -> bool;                        // 运行时探测（驱动/SDK）
    fn upload(&mut self, bytes: &[u8]) -> BackendBuffer;   // host → device
    fn dispatch(&mut self, kernel: KernelId, buffers: &[BackendBuffer], workgroup: [u32; 3]);
    fn readback(&mut self, buffer: &BackendBuffer, out: &mut [u8]);
    fn phases(&self) -> PhaseTimings;                      // upload/compute/readback 分相
}
```

设计约束：

1. **内核即数据**：WGSL 内核以 `KernelId` + 字节串描述；厂商栈 host 负责把等价内核
   （cutile tile DSL / SYCL C++ / HIP C++）按 `KernelId` 注册。跨厂商可复用的是 host
   侧 driver FFI 与 WGSL，不是内核二进制（§9.5 架构事实）——trait 只保证**布局、
   分发、计时、一致性协议**四件事统一。
2. **回退链**：`cutile → musa → sycl → wgpu(Vulkan)` 依可用性探测次序；wgpu 永远
   编译进二进制作为保底，厂商栈全部 `is_available()` 失败时静默回退并在结果 JSON
   的执行档位字段标注实际 backend。
3. **windows-gnu CI 约束**：CI 只编译 trait 层 + wgpu 后端；厂商后端一律
   `#[cfg(feature = "gpu-cutile")]` 等特性开关隔离，SDK 不进 CI 依赖图。sycl-rs 的
   Windows 编译尝试在 gpu-lab（workspace 外）进行，不触碰 CI。
4. **一致性协议**：同一输入四后端跑同一内核，逐指标容差显式写死在对照脚本
   （pearson：f32 档 |Δ| ≤ 1e-5 相对、histogram：整数精确），超差即记录并回退。
5. **双实现硬规则落点**：CPU 侧（scalar+SIMD）与 GPU 侧（trait 任意后端）交付
   对齐 §9 双实现硬规则；k-mer 的 GPU 化（动态内存需求）排在厂商栈就绪后
   （§9 既定），走 musa/cutile 的动态 buffer，不走 trait 首版。

## 4. 风险表

| 风险 | 等级 | 应对 |
| --- | --- | --- |
| AMD 无官方 Rust 计算路径 | 高（既定） | Vulkan 持平层为正式 AMD 路线（§1.4）；ROCm 档仅做 HIP C++ 对照 + amdsmi 观测，不承诺 Rust on ROCm |
| sycl-rs v0.1.0 实验性（Windows 未测） | 中 | 失败即产出：Windows 实测报告本身是 §2.1 的 issue 素材；主线不依赖 sycl-rs |
| 旧卡 CUDA 版本边界（750 Ti sm_50） | 中 | 750 Ti 双路径拆分：wgpu/Vulkan + cudarc/CUDA 12.x；cutile-rs 仅 sm_80+；对照表分列 |
| 国产兼容为源码级（PTX/cubin 不通用） | 中 | 不做二进制复用假设；每家内核独立编译，trait 层只统一 host 协议 |
| 云租窗口短（4090/ROCm/910B） | 中 | 执行卡预演：本地 gpu-lab 流程先跑通 wgpu 档，云租只补厂商栈档；脚本离线可重放 |
| cudarc-musa crates.io 未发布 | 低 | 持续跟踪官方仓库；发布前以 git 依赖在 gpu-lab 内验证，不进 engine |
| 上游 issue 无人响应 | 低 | issue 即交付（M5-G3 验收线）；每季度复查一次状态，不做无限等待 |

## 5. 交付序列（对齐 §3.1 批次 D 顺序）

1. **D1 Pearson v3（wgpu 档先行）**：subgroup 归约 + 多对批次化，与 v2 对照升级
   engine-evals 台账（不改台账既有结论，追加 v3 节）。
2. **D2 host 抽象层**：本文 §3 草案在 gpu-lab 落码（trait + wgpu 后端 + 探测回退），
   厂商后端特性开关空壳先行。
3. **D3 sycl-rs 本机尝试**：§1.3 执行卡 + §2.1 issue（失败也算交付）。
4. **硬件到位即插队**：S80（§1.2/§2.2）→ 4090 云租（§1.1/§2.3）→ ROCm 云租
   （§1.4/§2.4）；每次插队走完整执行卡并更新本提案的状态列。
