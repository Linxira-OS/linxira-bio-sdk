# 云租赁第 2 轮 — MTT S4000（摩尔线程）对照验收（2026-10-02）

> 承接 `gpu-rental-2026-10-02.md`（第 1 轮 vGPU-32GB）。同套 T0–T4 流程在
> S4000 上执行，双供应商同任务对照。核心反直觉发现：**摩尔原生工具链一次
> 点火通过，NVIDIA 社区前沿工具链（cutile）反而走不通**；两家 Vulkan 各自
> 堵在不同的层。

## 机器与环境

- **MTT S4000**，49152 MiB（48GB），MUSA 驱动 **2.7.0**（`mthreads-gmi 1.14.0`）。
- MUSA toolkit **3.1.0**（`kuae1.3.0_musa3.1.0`，2024-09-05），CUB 1.12.1，
  **mcc = clang-14 派生**（mtcc baf70da0），`/usr/local/musa/bin` 133 个工具。
- 容器内设备：`/dev/mtgpu.5` + `/dev/dri/renderD133`（card6）。
- Ubuntu jammy，15 核，1TB 内存容器；镜像源与第 1 轮相同（rsproxy/清华/turbo）。
- 同一 rental bundle（SHA256 校验同源）、同一 RUNBOOK、同一验收标准。

## T0 — 环境就绪：PASS（首次仪器化计时）

- `time cargo build --release -p linxira-bio-cli`：**real 47.255s**（59 crates，
  15 核，rsproxy 全量下载+编译含在内）。二进制在仓库根 `target/release/`
  （workspace 统一 target-dir，不在 `engine/` 下——RUNBOOK 需修正的一处）。
- `doctor` / `environment audit` 均 exit 0；audit 检出 `vulkan-runtime=false`
  （与 T2 一致）、无容器后端。
- rustup 走 rsproxy：**45 秒**完成安装（对比第 1 轮的反复重试）。

## T1 — 分子对接：PASS，跨供应商同值

| 项 | 第 2 轮 S4000 | 第 1 轮 vGPU | 判定 |
| --- | --- | --- | --- |
| best affinity | **-13.27** | -13.27 | 同值（官方 -13.234 ± 1.0） |
| 同种子复跑 | out1 ≡ out2（字节） | 字节一致 | PASS |
| 对接是 CPU 任务 | 同 vina 1.2.5 / seed 42 / 输入 | 同 | **跨供应商可复现** |

## T2 — Vulkan 路线：记档（两家堵点不同，摩尔更深一层）

| 阶段 | vGPU-32G（第 1 轮） | S4000（第 2 轮） |
| --- | --- | --- |
| ICD manifest | 有 negotiate 符号、无入口 | `musaicdconf.json` 正常发现 |
| dlopen + negotiate | 失败（缺 vkCreateInstance） | **成功**，报出实例扩展 |
| 设备访问 | 不适用 | **renderD133 打开成功，ioctl 全成功** |
| vkCreateInstance | — | **驱动内部失败**（terminator 跳过 ICD） |

- 排查路径：装 loader 后 ICD 缺 `libSPIRV-Tools-shared.so` → 自建 SPIRV-Tools
  （HEADERS 进 `external/`，`make SPIRV-Tools-shared`）装入 musa 目录 →
  ldd 干净 → 仍失败。strace：除 renderD134-191 不存在扫描外无任何失败
  open/ioctl，无 sysfs 缺口。
- **嫌疑（未定案）**：容器用户态 toolkit 3.1.0 vs 宿主内核驱动 2.7.0 的
  版本错配；驱动在能力协商后主动放弃。loader/strace 证据已留档，可出上游 issue。
- 上游价值：Moore 镜像缺 `libSPIRV-Tools-shared.so` 依赖，属镜像打包问题。

## T3-Moore — 原生 MUSA 内核：PASS（一次点火，3 次微迭代）

```c
// muhello.mu — CUDA 风格源码，musa_runtime.h
__global__ void square(const float* in, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = in[i] * in[i];
}
// 编译：mcc muhello.mu -o muhello -lmusart   （无需架构旗标）
// 运行：square OK fails=0 hout[10]=100.0
```

- 微迭代 3 步：`--musarchs` 旗标不存在（mcc 走 nvcc 风格且有默认架构）→
  裸编链接缺 runtime → `-lmusart` 成功。
- **与第 1 轮 T3 的对照是本轮最大发现**：NVIDIA 侧的社区前沿栈（cutile，
  TILE/MLIR）五级阻塞后定案不可用；摩尔**厂商自家编译器开箱即用**，
  `musa_runtime` API 与 CUDA 同构（`musaMalloc/musaMemcpy/musaLaunchKernel`），
  **cudarc-musa 的前置可行性当场验证**——G3 厂商抽象层的 MUSA 后端有了
  实机锚点。

## T4 — SSR 三后端 parity：PASS

| 项 | S4000（15 核新机） | vGPU 轮（16 核 Xeon） |
| --- | --- | --- |
| tiny / 1M 摘要+TSV | **字节一致** | 字节一致 |
| rust 1M wall | **0.047 s** | 0.082 s |
| pytrf 1M wall | 0.169 s | 0.312 s |
| rust/py 比 | 3.6× | 3.8× |

- 踩坑与第 1 轮相同：pytrf 需先 pip（`--backend python` 对缺失包报 exit 3）；
  另新记一条：`simulate sequence` 输出是位置参数（非 `--prefix`）。

## 耗时对照（本轮核心交付）

| 任务 | 第 1 轮 vGPU | 第 2 轮 S4000 |
| --- | --- | --- |
| 工具链安装（rust 等） | 反复重试（rsproxy 之前） | **45 s** |
| T0 release 构建 | 会话级未单测 | **47.3 s**（仪器化） |
| T0–T4 全程 | ~2.5–3 h | **~29 min**（20:28–20:57 机器时钟） |

- **归因声明**：第 2 轮享受 playbook 红利（bundle/RUNBOOK/已知坑清单/镜像
  源配置），全程时差不全是供应商差异。**供应商真差异在 T2/T3 的形态**：
  T2 两家都堵 Vulkan，但摩尔堵在更深的运行层（可修概率更高）；T3 是
  质变——摩尔 vendor 工具链立即可用，NVIDIA 社区前沿栈走不通。
- 用户外推规则的实证：非 NVIDIA 路线的**下限**没有被打破——但"上限"比
  预期好得多：只要走厂商支持的编译器，摩擦可以低于 NVIDIA 社区路线。
- **带宽孪生内核对照（Pearson wgpu 768 vs 736 GB/s）两轮都未跑成**：
  两家 Vulkan 各有阻断。替代路径已打开——把 v2 内核移植为 MUSA C 用
  mcc 原生对照（第 3 轮候选，机上即可做）。

## 产物

- 本地：`release-artifacts-rental/round2-s4000/artifacts-round2/`（16 件，
  含两轮同值对接 JSON、parity TSV、muhello.mu 源码；git-ignored）。
- 远端 `/root/autodl-tmp` 保留：SDK 克隆、bundle、muhello、vkdebug.txt。

## 下一步

1. Pearson v2 内核 → MUSA C 移植，mcc 原生跑带宽孪生对照（第 3 轮主目标）。
2. MT ICD CreateInstance 失败 → 整理 loader/strace 证据出上游 issue。
3. 博客（论文规范+参考文献）：双供应商同任务对照 + "前沿社区栈 vs 厂商
   工具链"的反差叙事，素材已齐。
