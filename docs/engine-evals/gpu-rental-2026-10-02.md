# 云租赁第 1 轮 — vGPU-32GB（RTX 4080 SUPER 改装）验收（2026-10-02）

> 承接 `cloud-rental-plan-2026-10-02.md` 的 T0–T4 清单与验收标准。第 1 轮在
> vGPU-32GB 上执行。产出两类结果：**能力验收**（T0/T1/T4 PASS，T2 记档，
> T3 阻塞链记档）与**生态耗时基线**——按既定口径，NVIDIA 是生态最好的厂商，
> 在其上消耗的时间是非 NVIDIA 路线（摩尔线程 MUSA 等）的耗时下限，只多不少。

## 机器与环境

- AutoDL 容器：Ubuntu 22.04 (jammy)，x86_64，16 核 Xeon，`/root/autodl-tmp` 数据盘。
- GPU：NVIDIA GeForce RTX 4080 SUPER（vGPU 改装单卡），32760 MiB，驱动
  **595.71.05**，CUDA 13.2，sm_89（`nvidia-smi` 与 `environment audit` 双源确认）。
- 引擎 1.1.1（`git clone` 后临场编译），Python 3.10.12。
- 下载走 rsproxy.cn（cargo）/ 清华 pip 镜像；GitHub 走 `network_turbo` 代理。
- 说明：vGPU-32G（4080S 改装）与 vGPU-48G（4090 改装）共享同一 vGPU 驱动栈，
  本轮生态结论预期 1:1 迁移到 48G 档，仅显存容量不同（待实测确认）。

## T0 — 环境就绪：PASS

- `cargo build --release -p linxira-bio-cli` 成功；`doctor` exit 0。
- `environment audit` exit 0：3 available / 30 missing；独立检出
  `vulkan-runtime=false`（与 T2 手工追查相互印证）与无容器后端警告。
- 踩坑（均已修复）：ssh 密码认证失败 → 改密钥；cargo 锁 crates.io index
  超时 → `~/.cargo/config.toml` 切 rsproxy-sparse。

## T1 — 分子对接（首要验收项）：PASS

| 项 | 值 | 验收标准 | 判定 |
| --- | --- | --- | --- |
| best affinity | **-13.27 kcal/mol** | 官方 -13.234 ± 1.0 | PASS（Δ=0.036） |
| modes | 9，rank1 rmsd_lb/ub = 0 | ≥1 | PASS |
| 同种子复跑 | out1.pdbqt ≡ out2.pdbqt（字节一致） | 字节级复现 | PASS |

- 输入为官方 basic_docking 示例文件（bundle 内，SHA256SUMS 校验通过）。
- 产物：`t1-dock1.json`、`t1-dock2.json`、`out1.pdbqt`、`out2.pdbqt`。

## T2 — wgpu/Vulkan 路径：记档（阻断，非失败）

- `libvulkan1` + `vulkan-tools` 安装后 `nvidia_icd.json` 出现，但
  `vulkaninfo` 仍只枚举到 llvmpipe。
- `VK_LOADER_DEBUG=all` 关键行：`Could not get 'vkCreateInstance' via
  'vk_icdGetInstanceProcAddr' for ICD libGLX_nvidia.so.0`。
- **结论：vGPU guest 驱动 595.71.05 production 不带功能性 Vulkan ICD
  （有 negotiate 符号、无 vkCreateInstance 入口）→ 本卡上 wgpu/Vulkan 缺席，
  CUDA 是唯一 GPU 路线。** 这是 vGPU 改装卡（4080S/4090 系）的栈级事实，
  不是 SDK 缺陷；对真卡（V100/T4/S4000）不外推。

## T3 — cutile-rs 真测：阻塞链记档（第一开发者窗口素材）

目标：在 sm_89 + CUDA 13.2 上用社区 CUDA 栈跑通 hello-compute，为
"不绑 NVIDIA 官方 toolkit 的 Rust CUDA 路线"取第一手证据。阻塞链按序：

| # | 阻塞 | 现象 | 处置 | 上游价值 |
| --- | --- | --- | --- | --- |
| 1 | 目标 crate 缺位 | `cutile-rs` 不在 crates.io；仅 `cutile 0.0.0-alpha` | 改用 alpha | 文档/可发现性 |
| 2 | 依赖被 yank | `cuda-bindings 0.0.0-alpha` yanked，元数据无 repo 字段 | 本地 vendor + `[patch.crates-io]` | 上游应留 repo/重新发布 |
| 3 | 构建入口 | build.rs 要 `CUDA_TOOLKIT_PATH`（非 `CUDA_PATH`），无 toolkit 可用 | pip `nvidia-cuda-nvcc==13.3.73` wheel（`nvidia/cu13/`） | wheel 化 toolkit 可作构建端 |
| 4 | bindgen 依赖 | libclang 缺失；`curand.h` 等 header 缺失 | apt `libclang-dev`；pip runtime/curand/cccl 等 wheel，`cp -rs` 合并头文件进 `cu13/include` | header wheel 合并套路可复用 |
| 5 | **mlir/tblgen 版本墙（定案）** | `failed to find correct version (21.x.x) of llvm-config (found 14.0.0)`——报错源为 **mlir-sys 210.0.4 与 tblgen 0.8.1** 的 build.rs（注册表缓存实搜定位），要求 **LLVM 21 且带 MLIR** | Ubuntu 22.04 无；apt.llvm.org 在本网络 ~7 KB/s（144 MB ≈ 4 h）；GitHub release tarball 走加速代理亦停滞（67 MB/2 GB 处 stall） | 前沿工具链栈的真实系统要求 |

- **架构定性（T3 的核心产出）**：`cuda-bindings 0.0.0-alpha` 的 repository 字段
  指向 **`nvidia/tile-rust`**；cutile 依赖树含 `cuda-tile-rs`、`cutile-compiler`、
  `cuda-async`、`cuda-core`、`candle-core`，编译器路径走 **MLIR**。即 cutile
  0.0.0-alpha 不是薄 CUDA FFI，而是 NVIDIA **TILE 实验性编译器栈**的社区快照。
- **战略裁决**：G3 厂商抽象层需要的"薄宿主端内核发射"归 **cudarc**（成熟、
  纯 FFI、无 MLIR 工具链负担）；cutile 转入**实验轨**——它是 TILE ISA
  （Rubin 世代）的早期入口，待 TILE 硬件入列再启动；TILE 内核在 sm_89 上
  的可运行性未验证（即便完成 LLVM 21+MLIR 工具链，运行价值存疑）。
  `gpu-lab/src/backends_cutile.rs` 保持 feature-gated stub，本页为定案依据。
- 运维注：远端长任务必须 setsid 脱会话（父 ssh 取消会连带进程组 SIGHUP，
  第一轮 llvm 安装即死于此时）；容器内国际网络差，apt.llvm.org 实测
  ~50 KB/s（无代理）/ ~7 KB/s（turbo 代理反慢），GitHub release tarball
  加速通道亦会 stall——大件工具链获取是本轮最大的非 GPU 时间黑洞。
- 性质区分：#2–#5 属**前沿工具链摩擦**（alpha crate / LLVM 21），换任何厂商
  都要再付一遍；#2 之外唯 Vulkan ICD（T2）是 **NVIDIA vGPU 专属摩擦**。

## T4 — SSR 三后端 parity：PASS

| 项 | rust 后端 | pytrf 后端 | 判定 |
| --- | --- | --- | --- |
| tiny（3 seq, 12 bases）摘要 | `t4-rust.json` | `t4-py.json` | **字段级一致**（文件字节相等） |
| 1M bases 摘要 | `big_rust.json` | `big_py.json` | 字节相等 |
| tiny TSV | `t_ssr_rust.tsv` | `t_ssr_py.tsv` | **字节一致** |
| 1M TSV | `big_rust.tsv` | `big_py.tsv` | 字节一致 |
| wall（1M，16 核 Xeon） | **0.082 s** | 0.312 s | rust 3.8× |

- py 后端环境坑两处：numpy 缺失导致 exit 3（pip 后好）；结果 JSON 是摘要
  直出、无 `records` 包装（消费端已按此写）。

## 生态耗时基线（第一阶段核心交付）

会话级估计（未逐项秒表，S4000 轮起改用 `time` 逐项留痕）；量级可信，
用于估算后续非 NVIDIA 路线的**下限**：

| 任务 | 阻塞/返工 | 会话墙钟（约） | 性质 |
| --- | --- | --- | --- |
| T0 克隆+编译+doctor/audit | 2（ssh 认证、cargo index） | 20–30 min | 通用+中国网络 |
| T1 vina+官方数据+对接×2 | 0 | ~10 min | 通用 |
| T2 Vulkan 追查 | 1（ICD 无入口） | ~15 min | **NVIDIA vGPU 专属** |
| T4 pip+四跑 parity | 1（numpy） | ~10 min | 通用 |
| T3 cutile 阻塞链 | 5（见上表） | ~90 min，定案闭环 | **前沿工具链，跨厂商** |

- **NVIDIA 最优生态、零成本预算约束下，五项验收 ≈ 2.5–3 h**，其中 T3 占
  一半以上且尚未走完。
- 外推规则（供 S4000 轮预算）：摩尔线程耗时 ≈ 本表 T0/T3/T4 的前沿工具链
  部分（cudarc-musa 同为 alpha 级，LLVM/头文件墙同样存在）**加上** MUSA
  专属摩擦（SDK 2.7.0 ABI、无 CUDA 13 wheel 对应物、厂商 ICD 成熟度）。
  T2 类驱动栈事实需在 S4000 上独立重查，不沿用。
- 结论：若 NVIDIA 路线尚且如此，非 NVIDIA 路线只会只多不少——本表即下限
  证据，直接进博客的"工程成本"节。

## 验收对照（对 cloud-rental-plan 验收标准）

| 标准 | 结果 |
| --- | --- |
| docking -13.234 ± 1.0 | PASS（-13.27） |
| 字节级复现 | PASS |
| gpu-bench 通过门 | 不适用（无 Vulkan→T2 记档；CUDA 路线归 T3） |
| SSR 字段级 parity | PASS（tiny + 1M 双档） |

## 产物与回填

- 本地：`release-artifacts-rental/round1-vgpu32g/artifacts-round1/`（15 件，
  含全部 JSON/TSV/pdbqt；该目录 git-ignored）。
- 远端保留：`/root/autodl-tmp/`（供 S4000 轮同流程对照）。
- **T3 回填**：见下节。

## T3 回填（2026-10-02 定案）

- **结果：工具链未走通，按计划记档闭环（任一结果均算验收）。** 五级阻塞链
  全部实修/定位到源头，最终定案为 mlir-sys/tblgen 的 LLVM 21+MLIR 要求 +
  本容器国际网络对大件工具链获取不友好；非 GPU 因素，与 sm_89 无关。
- **以架构定性代替编译通过**：cutile = nvidia/tile-rust TILE 实验栈（证据：
  cuda-bindings repository 字段 + 依赖树），G3 薄 FFI 路线定 cudarc；
  cutile 留 TILE 硬件（Rubin 世代）启动后再战。
- 上游 issue 素材已备齐：yanked crate 无 repo 字段、`CUDA_TOOLKIT_PATH`
  命名、wheel 头文件合并套路、mlir-sys/tblgen 对 LLVM tip 的硬依赖。
