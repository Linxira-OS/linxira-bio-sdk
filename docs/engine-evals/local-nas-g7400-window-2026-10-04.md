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

> 本节是 **rust/python 两腿**的测量（1.1 GB FASTQ 档）；装了 R 之后用 200 MB FASTQ 重跑了三腿，
> 见 §6.1——两档数据互相印证（Rust ≈2.3–2.5 µs/read、python ≈70 µs/read 在两种规模上一致）。

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

## 6. R 腿（第三个语言后端）——首次真实执行，抓出 5 个缺陷

NAS 装好 R 4.6.1 + CRAN/Bioconductor（jsonlite 2.0.0、digest 0.6.39、Biostrings 2.80.2 /
Bioconductor 3.23；Arch 的系统库不可写 → 必须用用户库 `$HOME/R/x86_64-pc-linux-gnu-library/4.6`）。

### 6.1 三端结果（GNU time 高精度，median-of-3）

| 能力 | 数据集 | rust | python | r | rust:python:r | rust RSS / python RSS / r RSS |
| --- | --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 97 MB FASTA | **800 ms** | 840 ms | **1820 ms** | 1 : 1.05 : **2.28** | 7.7 / 69.6 / 321.2 MB |
| `fastq.qc.v1` | 200 MB（646k reads） | **1600 ms** | 49,170 ms | **704,130 ms** | 1 : 30.7 : **440** | 7.6 / 46.0 / 79.6 MB |

- `precision: high`、`consistency: consistent`（三端结果在 1e-6 内一致）。
- self_reported（进程内分析耗时）：stats python 471/840 ms（启动占 44%）、r 319/1820 ms
  （**启动占 82%**，R 解释器 + 包加载）；fastq.qc python 48.6/49.2 s、r 703.4/704.1 s（大规模下启动可忽略）。
- 结论升级为三层：**C 级原语可覆盖**（stats）→ r 仅 2.3× 慢；**纯解释器逐记录循环**（fastq.qc）
  → python 30.7×、**r 440×**。R 的用途是"生态唯一实现"（DESeq2/survival 等），不是性能路径。

### 6.2 抓出的缺陷（全部因为该包从未被真正执行过）

| # | 位置 | 症状 | 根因 | 状态 |
| --- | --- | --- | --- | --- |
| 1 | `sequence_ssr_v1.R` | `width()` 派发失败 | Biostrings 只对 **XStringSet** 定义 `width()`，裸 XString 没有方法 | 已修（宽度提到集合级） |
| 2 | 测试本身 | 断言 22 与字面量不符 | 测试字面量 `"AAAAAAAAAATATATATATATAT"` 实为 **23** 字符（多一个 T） | 待修（测试用例） |
| 3 | `sequence_ssr_v1.R` | SSR 恒返回 0 条 | `start(...)` **未加前缀** → 命中 `stats::start`（时间序列）返回 `c(1,1)` | 已修（`Biostrings::start`） |
| 4 | CI | 缺陷长期潜伏 | **CI 从未运行 benchmark 两包的测试**（`benchmark-r`/`benchmark-python` 无任何 step） | 已补（debian 作业新增两包测试；R 侧经 BiocManager 装 Biostrings） |
| 5 | `sequence_ssr_v1.R` | 无 SSR 的输入被 worker 拒绝：`invalid type: sequence, expected a map` | 空 `list()` 被 jsonlite 序列化成 `[]`，而契约的 `motif_length_counts` 是 map（须 `{}`） | 已修（保留 names 属性；已用「无 SSR 输入」端到端验证 `{}` 与 Rust 一致） |

**修复后验证**（全部在 NAS 上实跑）：

- R 包测试套件：**`benchmark-r harness tests passed (with Biostrings parity)`**（首次全绿）。
- SSR 三端字节一致：22 bp 夹具上 rust == r == python（A×10、TA×6、compound=1，与 Rust 引擎逐字段相同）；
  无 SSR 输入上 rust == r，`motif_length_counts` 均为 `{}`。
- **R 的 SSR 规模化能力（重要结论）**：同一 97 MB 输入，Rust 1.6 s、pytrf 2.3 s，**R 1633 s（≈27 min）**
  ——`matchPattern` 逐 motif 扫描近似 O(4^k × 序列)，比 Rust 慢约 **1000×**。该次运行结论只在
  结果序列化处失败（分析本身完成），故时间可作单次观测；三端跑分不应再把 SSR 这项纳入 r 腿。

> 教训：三个语言后端里，只有"真的在机器上跑起来"才暴露实现问题。**测试存在但不在 CI 执行 = 不存在**。

## 7. 门禁宿主机迁移（本轮新增约束）

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

### 7.1 发布包白名单的端到端验证（提交 `9b9e4fa`）

- CI 的 `ci.yml` 通过了 YAML 解析，且新增的两个 benchmark 包步骤确实在 `debian-bookworm` 作业里。
- 补齐缺失的 release 二进制后（NAS 上原来只建了 cli/worker）**完整打包成功**：
  `stage-release.py --platform arch` → **606 个文件**、`docs/` 下 **18 项 = 17 份规范文档 + `capabilities/`**、
  `engine-evals/` 与 `proposals/` **一个都没有**、`bundle-manifest.lock.json` 正常生成。
- 顺带数据点：Linux release 二进制 `linxira-bio-ui` **45.4 MB**、`linxira-bio-mcp` 0.93 MB。

### 7.2 NAS 上 pull 静默失败的真因（重要运维教训）

不是网络（网络只抖动过一次）：**NAS 侧被脚本改写的 `runtime-preferences.json` 与
`workflows/org.linxira.benchmark-r/manifest.json` 让工作区变脏**，`git pull --ff-only` 于是拒绝更新，
而脚本里 `git pull ... | tail -1` 把错误吞掉了——我因此白白在旧提交上跑了一轮"验证"。
现已固定为先 `git checkout --` 这两个由工具生成的文件再 pull，并在同步后**断言
`HEAD == origin/main`**。

## 8. 文档边界与隐私评估（2026-10-04 追加）

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

## 9. 本轮得出的行动项

- [ ] 对外口径改写：小输入比值标注为"启动开销"，吞吐结论按内核类型分层（C 级原语 ≈1.0×、纯 Python 逐记录 ≈30×）
- [x] 三端覆盖扩展：R 装到 NAS（用户批准）后补齐 r 腿，`sequence.stats` 的 r 行与 `fastq.qc` 三腿数据已回写（§6.1）
- [ ] 低档 GPU 扩测：GTX 750 Ti 的 CUDA 12.x 路径（需装 toolkit）与 Vulkan 路径对照
- [ ] 其余三语言能力（`variant.stats`/`expression.pca`/`set.venn`/`enrichment.overrepresentation`/`structure.pdb.summary`）也补 r 腿
- [ ] R 包测试字面量修正（`"AAAAAAAAAATATATATATATAT"` 多一个 T，断言按 22 写）
- [ ] R SSR 实现的规模化能力评估（`matchPattern` 逐 motif 扫描在 97 MB 上可能不可行——见 §6）
- [ ] NAS 侧固定 `performance` governor 或记录并接受 powersave（当前已如实记录）
- [x] 文档边界：已改为 bundle 白名单（`docs/capabilities` + 17 份规范文档），`engine-evals`/`proposals` 不再随包；**完整打包实测 606 文件、无内部目录**
- [ ] `docs/research/` 空目录（git 未跟踪、易与 `.research/` 混淆）待清理
- [ ] 若认可 CI 成本：确认 debian 作业新增的 benchmark 包测试在 GitHub 上首跑通过（Biostrings 需源码编译，预计 +5–10 min）
