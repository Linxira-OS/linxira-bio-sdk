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
- **回写**：`scripts/benchmark/analysis/update-runtime-preferences.py` 已把两行写入 `runtime-preferences.json`
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

### 6.1 三端结果（GNU time 高精度，median-of-3，全部 `consistent`）

| 能力 | 数据集（类别） | rust | python | r | 1 : py : r | rust RSS |
| --- | --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 97 MB FASTA（sequence） | **800 ms** | 840 ms | 1820 ms | 1 : 1.05 : 2.3 | 7.7 MB |
| `expression.pca.v1` | 2000×40 矩阵（matrix） | **20 ms** | 4000 ms | 2760 ms | 1 : 200 : 138 | 8.4 MB |
| `set.venn.v1` | 20k 项 × 4 集合（other） | **10 ms** | 180 ms | 10,880 ms | 1 : 18 : **1088** | 8.4 MB |
| `enrichment.overrepresentation.v1` | 5k 基因 / 45k 关联（annotation） | **30 ms** | 220 ms | 4470 ms | 1 : 7.3 : 149 | 16.1 MB |
| `structure.pdb.summary.v1` | 1.1 MB PDB / 24k 原子（structure） | **40 ms** | 820 ms | 20,700 ms | 1 : 20.5 : 518 | 29.8 MB |
| `variant.stats.v1` | 8.7 MB VCF / 200k 变异（other） | **80 ms** | 760 ms | 13,880 ms | 1 : 9.5 : 174 | 7.8 MB |
| `fastq.qc.v1` | 200 MB / 646k reads（other） | **1600 ms** | 49,170 ms | 704,130 ms | 1 : 30.7 : **440** | 7.6 MB |

- 七项全部三端 `consistent`（1e-6 相对容差内），全部 `default_backend=rust` 已回写
  `runtime-preferences.json`（**7 条**）。
- self_reported（进程内分析耗时）把墙钟拆开：stats python 471/840 ms（启动 44%）、r 319/1820 ms（启动 82%）；
  fastq.qc python 48.6/49.2 s、r 703.4/704.1 s（大规模下启动可忽略）。
- **分层结论（禁止整体声明）**：C 级原语可覆盖（stats）→ 1.05×；向量化库良好（pca，numpy/prcomp）→
  100–200×；**逐记录/逐项解释器循环**（fastq.qc、venn、pdb、variant）→ python 10–30×、**r 150–1100×**。
  Rust 的稳定收益是**内存**（同任务 1/8–1/40 峰值 RSS）与零解释器启动。
- 复现口径：输入由固定种子的生成器产出（纯 Python，`random.seed(42)`），只落在 `/tmp`，不脏仓库：
  `matrix.tsv` 2000×40（`273755e5…`）、`sets.tsv` 20k×4（`b1f67693…`）、`structure.pdb` 24k 原子
  （`ecb30a51…`）、`variants.vcf` 200k（`1dab9c47…`）、`genes.txt`+`associations.tsv`（`84ac1b6e…`/`b523c5b4…`）；
  FASTQ/FASTA 由 `simulate reads/sequence`（seed 42）产出。

## 6.2 N=10 复核（同机同输入的第二次跑分，替代 §6.1 的 N=3 数字）

按用户要求，**每项一律 N=10（含 SSR）**，判定规则改为两段式（CI 排除 1.0 = 可分辨；CI 下界 ≥1.10 = **material**）。
六个"快"能力 + `fastq.qc` 的 N=10 结果（median-of-10，GNU time 高精度；`sequence.ssr` 因维护窗口顺延）：

| 能力 | rust | python | r | 判定 |
| --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 820 ms（IQR 27.5） | 840 ms（IQR 10） | 1840 ms | python **within-noise**（1.02×，CI [0.99, 1.04]）；r material（2.24×） |
| `expression.pca.v1` | 20 ms | 4010 ms | 2760 ms | 双 material（200.5× / 138×） |
| `set.venn.v1` | 10 ms | 180 ms | 10,975 ms | 双 material（18× / 1097×） |
| `enrichment.overrepresentation.v1` | 30 ms | 230 ms | 4470 ms | 双 material（7.7× / 149×） |
| `structure.pdb.summary.v1` | 30 ms | 790 ms | 20,750 ms | 双 material（26.3× / 692×） |
| `variant.stats.v1` | 80 ms | 760 ms | 13,495 ms | 双 material（9.5× / 169×） |
| `fastq.qc.v1` | 1570 ms（IQR 7.5） | 49,320 ms（IQR 827.5） | 702,655 ms（IQR 1045） | 双 material（31.4×，CI [30.5, 32.0] / 447.6×，CI [432.4, 452.1]） |

稳健性：全 10 次 / 去第 1 次 / 只取中间 8 次三种口径下，中位数与判定**完全一致**
（`statistics-trimmed-*.tsv`）。注：`benchmark run` 本身已有 1 次**不计时预热**，报告里 10 条全是计时轮次。

### 6.2.1 假设被推翻：记录粒度不是 `sequence.stats` 无差异的原因

原假设：python 慢在"每条记录要穿一次语言边界"，因此记录越多差距应越大。实验（同一 ~100 Mb，三种记录形态，各 N=10）：

| 输入形态 | rust | python | r |
| --- | --- | --- | --- |
| 1 × 100 Mb | 820 ms | 850 ms | 1825 ms |
| 20 × 5 Mb | 820 ms | 840 ms | 1830 ms |
| 100,000 × 1 kb | 830 ms | 860 ms | 1940 ms |

**假设被推翻**：记录数从 1 → 10 万，rust 仅 +10 ms、python 仅 +10 ms（比值从 1.037 变到 1.036）。
更正后的解释：该能力在 ~100 MB 量级上**两侧都被 I/O 与大块内存分配主导**（每碱基的活在 C 原语里、
每行的读取与建串两侧都要付），语言边界的穿越次数在这个量级上不是主导项。
真正拉开差距的是"**每元素是否需要解释器参与**"：`fastq.qc`/`set.venn`/`pdb`/`variant` 的逐碱基或逐项逻辑
在解释器里跑（30×–1100×），而 `sequence.stats` 的逐碱基逻辑在 C 里跑（≈1×）。
小输入上的 ~20× 则是解释器启动（python 约 170 ms）在 10 ms 级工作负载上的占比。

### 6.2.2 归档链路与存储披露（2026-10-11 新增）

- **输入在 tmpfs 上**：`/tmp` 是 tmpfs（7.7 GB），所有基准输入与报告都在 RAM-backed 存储上
  → 测量中**没有磁盘延迟项**；仓库检出位于 btrfs/NVMe（512 GB Netac），另有两块机械盘
  （ST1000DM003、ST500DM002）存在且与本轮无关。该事实已写入 descriptor 的 `storage` 字段。
- **归档一键化**：`scripts/benchmark/windows/2026-10-11/{descriptor.json,archive.sh}`
  —— 暂存报告（粒度实验的三份重命名以避让同名能力）、记录输入 sha256、生成 summary（过 schema）、
  统计表、图，并跑 `figures/verify-figures.py` 复算图上的每个数字。
- **本地端到端演练**：用已取回的 6 项 + 粒度报告空跑整条链路，`198/198` 图数值与表一致、
  summary 通过 `schemas/benchmark-summary.schema.json` 校验。
- 演练中修掉两个真缺陷：① 粒度实验的 3 次 `sequence.stats.v1` 若进入 `summary.json` 会与主运行
  **同名撞键**（图会多画 3 行、校验按 dict 折叠）→ 改为置于 entries 之外、单独归档；
  ② `verify-figures.py` 的"0 mismatch"曾因正则未匹配 `height="8.00"` 而**空解析假通过**，
  已修并加"表内 capability/backend 重复键"守卫。

### 6.2.3 时间线与暂缓决定（2026-10-11 晚）

用户按本地时间给出的窗口约束与决策（**不得擅自更改**）：

- 19:20 时点：`fastq.qc.v1` N=10 在跑（r 腿 11 × 704 s ≈ 2.15 h），预计 **≈20:20** 完成。
- `sequence.ssr.v1` N=10 的 r 腿约 **5.0 h**（11 × 1633 s），若 20:40 起跑则 **≈01:40** 才结束。
- 用户计划 **23:30 停机维护** ⇒ **SSR 的 N=10 长跑暂缓**，维护完成、用户通知后再跑。
- 因此今晚只做：取回 `fastq.qc` 报告 → 填进文章草稿；**归档与站点发布一并顺延**，等 SSR 数据齐了
  一次性生成（descriptor 已钉定 `report_id = bench-20261011-001`，重跑覆盖同一编号，不会自增）。
- 若发现 20:40 之后 SSR 真在跑，按此前惯例**停掉它**（本轮不会主动发车）。

### 6.2.4 两个必须解释的现象（2026-10-11 补）

**① `sequence.stats` 为什么测不出差异（1.02×，CI 跨 1）。** 原直觉是"数据在语言边界来回搬运"
应当有可观代价；实测推翻：Python 腿的热点循环不在解释器里，而在 CPython 的 C 原语
（`str.count`/`str.upper`）中；Biopython 虽逐行穿过解释器，但每行工作量小，两侧都要付的
大块分配与文件读取把差异吃掉。⇒ 该行不是"Rust 无优势"，而是**两侧关键路径都在原生代码里**。
一旦每元素的工作必须由解释器执行（`fastq.qc` 逐碱基、`set.venn` 逐项），同一边界即变成 31×–1097×。

**② 这台机器只测到单核（范围声明，不写成优势）。** 三端关键路径本窗口都是**单线程**：
引擎当前**无任何并行设施**（`grep -rn "rayon\|thread::spawn\|par_iter" engine/crates/**` 无命中，
Cargo 亦无 rayon/crossbeam 依赖），CPython 有 GIL，R 解释器单线程（仅 BLAS 之类自带线程）。
2 核/4 线程 + powersave 的低端机因此**完全没有体现 Rust 的并行语言优势**；反过来，若 Python 腿的
向量化路径在本机用了多线程，则"用更少核还快 200×"更凸显其**单核效率**。众核扩展性需另一次测量。

### 6.3 抓出的缺陷（全部因为该包从未被真正执行过）

| # | 位置 | 症状 | 根因 | 状态 |
| --- | --- | --- | --- | --- |
| 1 | `sequence_ssr_v1.R` | `width()` 派发失败 | Biostrings 只对 **XStringSet** 定义 `width()`，裸 XString 没有方法 | 已修（宽度提到集合级） |
| 2 | 测试本身 | 断言 22 与字面量不符 | 测试字面量 `"AAAAAAAAAATATATATATATAT"` 实为 **23** 字符（多一个 T） | 待修（测试用例） |
| 3 | `sequence_ssr_v1.R` | SSR 恒返回 0 条 | `start(...)` **未加前缀** → 命中 `stats::start`（时间序列）返回 `c(1,1)` | 已修（`Biostrings::start`） |
| 4 | CI | 缺陷长期潜伏 | **CI 从未运行 benchmark 两包的测试**（`benchmark-r`/`benchmark-python` 无任何 step） | 已补（debian 作业新增两包测试；R 侧经 BiocManager 装 Biostrings） |
| 5 | `sequence_ssr_v1.R` | 无 SSR 的输入被 worker 拒绝：`invalid type: sequence, expected a map` | 空 `list()` 被 jsonlite 序列化成 `[]`，而契约的 `motif_length_counts` 是 map（须 `{}`） | 已修（保留 names 属性；已用「无 SSR 输入」端到端验证 `{}` 与 Rust 一致） |
| 6 | `set_venn_v1.R` | 20k 项输入 `rust != r`：`intersections` 长度 4 vs 20000 | `summarize_memberships` **按 item 逐条**生成 intersections（并按条累加 set_sizes）；小夹具掩码恰好互不相同，所以没抓到 | 已修（按 occupancy mask 分组，与 Rust 的 BTreeMap 语义一致）+ 新增重复掩码回归用例（期望值取自 Rust 真值） |

### 6.4 顺带抓到的**产品**缺陷：宽数值矩阵被误判为 SAM

用 40 个样本的矩阵跑 benchmark 时，worker 直接拒绝：
`input input-matrix-1 format mismatch: request declares tsv but content identifies sam`。

- **根因**：`linxira-bio-core/src/dataset.rs` 的 `looks_like_sam` 只判
  `字段数 ≥ 11 && fields[1]:u16 && fields[3]:u64 && fields[4]:u8`。宽计数矩阵的小整数列天然满足
  （实测边界：**10 列 → tsv，11 列 → sam**），于是任何 ≥11 列的计数矩阵都被判成 SAM——`dataset inspect`
  与 worker 的输入校验都会错，真实用户的高样本量表达矩阵首当其冲。
- **修复**（`bfe6fd0`）：要求 SAM 结构证据——FLAG ≤ 0xFFF、RNAME **非纯数字**、CIGAR 符合
  `(\d+[MIDNSHP=X])+`（或 `*`）、SEQ 为碱基字母（或 `*`）。新增回归测试
  `wide_numeric_matrix_is_not_mistaken_for_sam`（12 样本矩阵 → `Tsv`；SAM 行 → `Sam`）。
- 验证：`m10/m11/m12.tsv → tsv`、`alignments.sam → sam`，目标测试与全量 `cargo test` 均绿。

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
`HEAD == origin/main`**。⚠️ 反过来也要注意：`git checkout --` 会**丢掉 NAS 上刚生成的**
偏好表/清单内容，所以正确顺序是「先在 NAS 上生成 → 取回本机提交 → 再同步」，本窗口因此
丢过一次 job A 的回写（用全部报告重跑工具即无损恢复，报告本身都在 `/tmp`）。

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

### 8.1 脚本整理时的一次误入库与用户决定（2026-10-11）

`scripts/benchmark/run/tier1v2-biolab.sh` 含实验室机器的 `/mnt/F`、`/mnt/G`、`/mnt/disk2`
挂载路径，原本被 `.gitignore` 挡住；2026-10-11 的脚本归组提交（`09fe3a9`）用 `git add -A`
把它带了进去。已用 `f112c9d` 从工作树移除并加固 ignore 规则（`scripts/benchmark/run/*-biolab.sh`），
文件仍留在磁盘上。

**用户决定（2026-10-11）：不改写历史。** 这些路径不是凭据，重写历史的代价大于收益；
后续不要再为此重开讨论。教训记在这里：**`git add -A` 会绕过"曾经未跟踪"的忽略预期**——
对"故意不入库"的文件，提交前先看 `git status --short`，或在 `.gitignore` 里保留与新位置匹配的规则。

## 9. 本轮得出的行动项

- [ ] 对外口径改写：小输入比值标注为"启动开销"，吞吐结论按内核类型分层（C 级原语 ≈1.0×、纯 Python 逐记录 ≈30×）
- [x] 三端覆盖扩展：R 装到 NAS（用户批准）后补齐 r 腿，`sequence.stats` 的 r 行与 `fastq.qc` 三腿数据已回写（§6.1）
- [ ] 低档 GPU 扩测：GTX 750 Ti 的 CUDA 12.x 路径（需装 toolkit）与 Vulkan 路径对照
- [x] 其余三语言能力补齐 r 腿：`expression.pca`/`set.venn`/`structure.pdb.summary`/`variant.stats`/`enrichment.overrepresentation` 全部三端 `consistent` 并已回写（§6.1，共 7 条偏好）
- [ ] `sequence.ssr.v1` 三端长跑（97 MB；r 腿约 27 min/次，用户明确要求"浪费也要做"）——进行中
- [x] N=10 复核：六个"快"能力 + 记录粒度实验完成（§6.2）；`fastq.qc.v1` 与 `sequence.ssr.v1` 的 N=10 长跑在跑
- [ ] 对外材料引用三端数据时按 §6.1/§6.2 的分层口径，不要写"Rust 普遍更快"
- [x] R 包测试字面量修正（`"AAAAAAAAAATATATATATATAT"` 多一个 T，断言按 22 写）— `c567f94`
- [x] R SSR 规模化实测：97 MB 输入 R 1633 s（rust 1.6 s / pytrf 2.3 s，≈1000×），结论=SSR 不用 R（§6.3）
- [ ] NAS 侧固定 `performance` governor 或记录并接受 powersave（当前已如实记录）
- [x] 文档边界：已改为 bundle 白名单（`docs/capabilities` + 17 份规范文档），`engine-evals`/`proposals` 不再随包；**完整打包实测 606 文件、无内部目录**
- [ ] `docs/research/` 空目录（git 未跟踪、易与 `.research/` 混淆）待清理
- [ ] 若认可 CI 成本：确认 debian 作业新增的 benchmark 包测试在 GitHub 上首跑通过（Biostrings 需源码编译，预计 +5–10 min）
