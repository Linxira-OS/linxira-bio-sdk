# 本地 NAS 低档机窗口 —— 2026-10-04（G7400 + UHD 710 / GTX 750 Ti）

> 目的：用零成本的自有低档机承担"同机三端速度对比 + 跨机字节 parity + 最低档 GPU
> 回退档"三类数据，把租用预算留给第三方真值与匹配硬件。本机（Windows/WSL）按用户
> 指令**不跑负载**，全部运行在 NAS 完成；口径与 `docs/engine-evals/gpu-*` 系列一致
> （固定输入 + 固定种子 + 同机 + 每档 median-of-3）。

## 1. 机器与环境

| 项 | 值 |
| --- | --- |
| OS | **CachyOS**（Arch 系；`environment audit` 报 `family=arch`，ID_LIKE 映射生效） |
| CPU | Intel **Pentium Gold G7400**，2 核 / 4 线程（3.7 GHz max），governor = **powersave** |
| 内存 / 磁盘 | 15 GiB / NVMe（校验用输入 100 MB 级，余量足够） |
| Rust | rustc/cargo **1.99.0**（stable） |
| Python | 3.14.7 + venv（numpy 2.5.4、biopython 1.88 —— 包锁 pin 1.85，pack 发 warning 并如实披露） |
| R | **未安装**（R 后端待装后再补） |
| 门禁/测量附加依赖 | ✅ 已装：`rustup component add clippy rustfmt`、`requirements-ci.txt`（pinned `jsonschema` 4.25.1）、GNU `time`（高精度计时与 RSS） |
| GPU（Vulkan 枚举） | ① **NVIDIA GTX 750 Ti**（DiscreteGpu，GM107 = 旧卡档 G-L）② **Intel UHD 710**（IntegratedGpu，低端核显） |
| 构建耗时 | CLI release **2m29s**；gpu-lab release **2m01s** |

环境坑（供后续窗口复用）：远端默认 shell **不是 bash**（脚本必须以显式 `bash` 管道送入）；
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

## 5. 官方 harness（`benchmark run`，worker 协议，median-of-3，GNU `/usr/bin/time -v` 高精度）结果

前置：`cargo build --release -p linxira-bio-worker` + `LINXIRA_BIO_WORKER`（CLI 自身不带 worker）；
NAS 另需装 GNU `time`（否则报告精度降级、`peak_rss` 为 `null`——本窗口第一遍就踩到了）。

| 能力 | 数据集 | rust 中位 | python 中位 | 墙钟比 | rust RSS | python RSS | 省内存 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 97 MB FASTA（20×5 Mb） | **800.0 ms** | **830.0 ms** | **1.04×** | 5.84 MB | 72.55 MB | **91.9%** |
| `fastq.qc.v1` | 1.1 GB FASTQ（3,333,334 reads × 150 bp） | **7,760.0 ms** | **249,780.0 ms** | **32.2×** | 6.12 MB | 48.92 MB | **87.5%** |

- 两次独立窗口（无 GNU time / 有 GNU time）同输入复跑，中位差 <3% ⇒ 数字可复现；
  离散度 `iqr_wall_ms`：rust 0.59 ms、python 4.62 ms（stats）；rust 125.6 ms、python 227.9 ms（fastq.qc）。
- **python 侧自报进程内分析时间**（`benchmark.self_reported`）把墙钟拆开：
  `sequence.stats` 分析 472.5 ms / 墙钟 841 ms ⇒ **约 45% 是解释器启动 + pack 校验**；
  `fastq.qc` 分析 250.3 s / 墙钟 252.6 s ⇒ 大规模下启动占比可忽略，差值即真实吞吐差。
- **回写**：`scripts/update-runtime-preferences.py` 已把两行写入 `runtime-preferences.json`
  （`sequence.stats.v1` 类 `sequence`、`fastq.qc.v1` 类 `other`，均 `default_backend=rust`）。
  注意：该工具按报告内的后端整行替换，**旧行的 r 后端数字（1770 ms / 238.5 MB，2026-09-11）
  因 NAS 未装 R 未入新行**——那是另一台机、另一日期的口径，混排反而不诚实；原值留在
  git 历史（`3591bf6`）中，待 R 装好后重新测量回填。

> 结论（分层，禁止整体声明）：
> ① **C 级原语可覆盖**（`sequence.stats` 的线性扫描）→ Rust 优势仅 **1.04×**；
> ② **纯 Python 逐记录循环**（`fastq.qc`）→ Rust 优势 **32.2×**；
> ③ Rust 的**稳定收益是内存**（同任务 1/8–1/12 峰值 RSS）与零解释器启动，而非全局吞吐倍数。

## 6. 门禁宿主机迁移（本轮新增约束）

用户 2026-10-04 指令：**本机（含 WSL）不得再跑负载**（其他主线项目占用），NAS 可随意使用。
⇒ 正式 Linux 门禁的宿主机从 WSL **迁到 NAS**，并补齐 NAS 缺的两个开发依赖：

- `rustup component add clippy`（此前 NAS 只有 rustc/cargo，`cargo clippy` 直接报组件缺失）
- `python3 -m pip install -r requirements-ci.txt`（`validate-repository.py` 需要 pinned `jsonschema`）

这两项是**CI 文档化的固定依赖**，不是系统服务变更；`docs/EXECUTION_POLICY.md` 的
"Linux 原生文件系统才是合法门禁位置"因此仍由 NAS（CachyOS 真机）满足，且不再依赖 WSL。

**NAS 上的完整门禁结果（提交 `9d9a8e3`）**：

| 门 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | ✅（先补 `rustup component add rustfmt`） |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 无警告 |
| `cargo test --workspace` | ✅ **525 passed / 0 failed** |
| `python3 scripts/validate-repository.py` | ✅（60 schemas + 223 实例、132 能力、46 技能、36 工具、profile local-core） |
| `gpu-lab clippy --all-targets -D warnings`（工作区外） | ✅ 12 → **0**（多余 `unsafe`×5、容量提示、`as_chunks`、死代码 `as_str`、`vec!` 宏） |

`gpu-lab` 不在 workspace 图里，所以工作区门禁**不覆盖它**；本轮把它单独拉齐到
`-D warnings` 干净，避免"载体项目长期带警告"变成盲区。功能性冒烟同时通过：
`gpu-lab backend-smoke` → `histogram[1]=64 pass=true`。

## 7. 文档边界与隐私评估（2026-10-04 追加）

**发布树事实**：`packaging/bundle-manifest.json` 的 `include_trees` = `docs, schemas, skills,
workflows, licenses` —— 即 **`docs/` 整棵树随发布包发给用户**，`docs/engine-evals/**` 也不例外。

按此口径逐项评估本窗口新增内容：

| 内容 | 判定 | 处置 |
| --- | --- | --- |
| 硬件/OS/工具链版本、耗时、哈希、GPU 内核分相 | 与既有 `docs/engine-evals/gpu-*` 台账同级（可比性所必需） | 保留 |
| 租用渠道路径（如某个容器数据盘路径） | 用户此前明确同意保留 | 保留 |
| **家用机运维指纹**：SSH 别名、远端默认 shell、免密 sudo、磁盘余量 | 内部运维信息，不该随包外发 | **已脱敏**（改为"局域网内可接入"这类中性表述） |
| 内部语气内容（窗口计划、预算、待批准项） | 属"内部工程计划"，随包外发不理想但已是既成惯例 | 见下建议 |

**结构性建议（待定，删除成本低）**：把 `docs/engine-evals/`、`docs/proposals/` 这类
**内部工程计划**移出发布树（例如仓库根 `notes/`：版本受控但不在 `include_trees` 内），
`docs/` 只保留用户可见内容；或反过来在 bundle manifest 里显式列出要发布的 doc 子集。
用户倾向："哪些给用户看、哪些内部看"需要梳理，后续再动。

## 8. 本轮得出的行动项

- [ ] 对外口径改写：小输入比值标注为"启动开销"，吞吐结论按内核类型分层（C 级原语 ≈1.0×、纯 Python 逐记录 ≈30×）
- [ ] 三端覆盖扩展：R 装到 NAS 后补 r 后端（`sequence.stats`/`fastq.qc`/`variant.stats` 等），并把 `sequence.stats` 的 r 行重新测量回填
- [ ] 低档 GPU 扩测：GTX 750 Ti 的 CUDA 12.x 路径（需装 toolkit）与 Vulkan 路径对照
- [ ] NAS 侧固定 `performance` governor 或记录并接受 powersave（当前已如实记录）
- [ ] 文档边界：`docs/` 整树随包（见 §7），内部工程计划（`engine-evals/`、`proposals/`）是否移出发布树待用户定
- [ ] `docs/research/` 空目录（git 未跟踪、易与 `.research/` 混淆）待清理
