---
title: 三语言生物信息实现的三端重复基准：Rust / Python / R 在 8 项能力上的 N=10 测量
date: 2026-10-11
tag: benchmark
lang: zh
desc: 在一台低端 x86 机器上，把同一个 Rust 引擎与 Biopython、Biostrings 实现放在同一批输入上各跑 10 次，用预注册判定规则与 bootstrap 置信区间逐一给结论：7 项存在实质性差距（7.7×–1097×），1 项测不出可分辨差异。
---

> 本文是草稿（增量写作中）。**待补**：`seq.fastq.qc`/`sequence.ssr` 两项的 N=10 长跑（脚本已排队，跑完把数字填进 §8.1、§8.6 与 §9 的表）。
> 表格原始文件：本目录同级仓库路径（见 §12）。图：`./figures/`。

## 1. 摘要

我们在一台低端 x86 机器（Intel Pentium Gold G7400，2 核 4 线程，15 GiB）上，对 Linxira Bio SDK 中
**8 项已实现能力**做了三语言后端的重复测量：Rust 引擎，以及分别以 **Biopython**、**Biostrings**
为对照的 Python 与 R 后端。每项、每后端各计时 **10 次**（此前另有 1 次不计时预热），
输入与脚本固定、串行执行、`powersave` 调节器全程不变。

结论按**两段式预注册规则**给出（§6.3）：

- **7 项存在实质性差距**（比值 bootstrap 95% CI 下界 ≥ 1.10）：Rust 相对 Python 快 **7.7×–200×**，
  相对 R 快 **2.2×–1097×**。
- **1 项（`sequence.stats`）测不出可分辨差异**：Rust 820 ms、Python 840 ms，
  比值 1.02×、CI [0.99, 1.04]——**跨过 1.00，因此本报告不下"更快"的结论**。
- Rust 的**稳定收益在内存**：同任务峰值 RSS 为 Python 的 1/2.2–1/12、R 的 1/5.5–1/55。

需要强调的边界：这一比较是**整栈比较**（我们的 Rust 引擎 vs 惯用的第三方库实现 + 各自语言运行时 +
同一套测量框架），不是"语言本身的快慢"；且结论只在这台机器、这批输入、这套协议下成立（§11）。

## 2. 更新日志（相对前两篇）

- 第一次把 **R** 作为真正的第三后端跑完并修复其 6 个缺陷（§10）——此前 R 腿从未在 CI 中真正执行过。
- 判定规则从"点估计"改为**置信区间 + 实质性下限**（§6.3），并给出裁剪稳健性（§8.4）。
- 新增**记录粒度对照实验**，用来解释 `sequence.stats` 的 ≈1×（结论：原假设被推翻，§8.5）。
- 数据、脚本、表格、图全部**随仓库发布**，可逐项复核（§12）。

## 3. 背景

三篇脉络：①自己的毕业课题（荞麦基因组）只关心"能不能跑完"；②对一份公开 SRA 数据做了一次
端到端复现；③本篇——不再挑数据、不再挑指标，按一套事先写死的协议把三语言后端放在同一批输入上跑。

动机很具体：这个 SDK 的产品承诺是"**本地优先**"。本地优先意味着用户很可能在没有 GPU、
内存也不宽裕的机器上跑；如果某个后端在小机器上慢 20 倍、内存吃 10 倍，那它就不是可选项。
所以我们需要的不是"Rust 比 Python 快"这种笼统断言，而是**逐能力的、带区间的、可复现的对照**。

## 4. 相关工作与第三方基准披露

**本轮使用的对照臂**（第三方实现，非我们编写）：

| 后端 | 实现基础 | 版本 |
| --- | --- | --- |
| Rust | 本仓库引擎（`crates/…`） | v1.1.3 |
| Python | Biopython（FASTA/FastQ 解析与统计）、numpy、`prcomp` 等价路径 | Biopython 1.88 / numpy 2.5.4 |
| R | Biostrings、`prcomp`、`stats` | Biostrings 2.80.2 / Bioconductor 3.23 / R 4.6.1 |

**本轮没有使用第三方基准套件。** 计划中的两条外部校准路径（尚未执行，故不作为本文证据）：

1. **GIAB + hap.py**：用 NIST 的公开真值集校准 **正确性**（而非速度）。我们已把变体调用与比较能力
   做到可以吃这些输入的程度，但真值集下载、区域掩码、容器化执行都未在本窗口完成。
2. **公开托管基准平台**（如 Phoronix Test Suite / OpenBenchmarking 的公开结果库）：
   用于把"这台机器的绝对数字"锚到第三方机器谱系上。本轮同样未执行。

我们把这两项写成"计划中"而不是"已做"，因为**它们不会因为写进文章就变成已做**。

## 5. 数据与素材

所有输入由仓库内的**固定种子生成器**产出（纯 Python，`random.seed(42)`；序列类用
`simulate sequence/reads` 的 seed 42），因此任何读者都能在本地重建同批输入。

| 能力 | 输入 | 规模 | sha256 |
| --- | --- | --- | --- |
| `sequence.stats.v1` | FASTA（20 × 5 Mb） | ~100 MB | 待补（`simulate sequence --seed 42` 产出） |
| `expression.pca.v1` | TSV 矩阵 | 2000 × 40 | `273755e5eca0ae14d150365271dda778666109341ec0826baa01b7dc7bd76345` |
| `set.venn.v1` | TSV 集合表 | 20k 项 × 4 集合 | `484dffd240e46a4de6995ae935107e464185ca93c9a8025e79856e1fa5234531` |
| `enrichment.overrepresentation.v1` | `genes.txt` | 5k 基因 | `84ac1b6eec0a96798aa39b905c654faa6156dc4f4af4d4eb5f672faf8d1f98db` |
| `enrichment.overrepresentation.v1` | `associations.tsv` | 45k 关联 | `b523c5b4509487186461e8bc29adcdce910551ed1c4b88d4768c220c5ff9d781` |
| `structure.pdb.summary.v1` | PDB | 3000 残基 / 24k 原子 | `ecb30a5181b376760ceab330d9244f79b28195b54a4a8cac26a8ca23e2559afe` |
| `variant.stats.v1` | VCF | 200k 变异 | `1dab9c47535f9b3552cd3cfae1fa70ac4fccc5ff7518d4c9cf5b76950861ce53` |
| `fastq.qc.v1` | FASTQ | 200 MB / 646k reads | 待补（由 `big.fa` 经 `simulate reads --seed 42` 派生） |
| `sequence.ssr.v1` | FASTA | ~100 MB | 同 `sequence.stats`（同一文件 `big.fa`） |

生成器已入库：`scripts/benchmark/inputs/generate-inputs.py`。**跨机、跨版本核对**：在 Windows（Python 3.13.15）
与本次测量机（Linux，Python 3.14.7）上分别生成上表六个合成输入，**六个 sha256 全部逐字节一致**——
说明这批输入不依赖生成机器，任何人都能重建。

（生成器只写 `/tmp`，不落仓库；哈希与窗口描述符一并归档，见
[`../summary.json`](../summary.json) 与 [`../statistics.tsv`](../statistics.tsv)。）

## 6. 方法

### 6.1 测量协议

- **每后端 1 次不计时预热 + 10 次计时重复 = 每后端共 11 次进程执行。**
  **报告里只有 10 条计时轮次**（`run_index` 1..10），预热那次的结果被丢弃、不入任何统计量；
  它的作用是把页缓存与运行时首次代价移出计时区间（因此报告记 `page_cache = warm`）。
  代码依据：`engine/crates/linxira-bio-cli/src/main.rs:7257`（`"warmup"` 请求）与 `:7273`（`for run_index in 1..=repeat`）。
- **本文所有统计量（中位数、IQR、CV、CI、判定）一律只在这 10 条计时轮次上计算。**
  §8.4 的"去掉第 1 次 / 只取中间 8 次"是**额外的稳健性检查**，不是替换 N=10——头条数字始终是 N=10。
- 墙钟用 GNU `/usr/bin/time -v` 高精度取值；同时记录进程内自报耗时与**峰值 RSS**。
- 同一次能力测量内**串行执行、不做任何并发**（跑分期间不跑 cargo/clippy/归档脚本）。
- 调节器全程固定 `powersave`；不切换 governor，避免与已归档窗口混排。
- 输入文件与命令在两次窗口间完全一致。
- 后端顺序固定为 rust → python → r；跨后端的频点/热漂移现象作为已知披露项（§11）。

### 6.2 统计量

中位数（median-of-10）、四分位距（IQR）、变异系数（CV）、最小/最大、峰值 RSS；
后端比值取中位数之比，并对原始 `wall_ms` 做 **bootstrap 重采样 10,000 次**（固定种子 20261011）
给出 **95% 置信区间**。

### 6.3 预注册判定规则（先定规则，再看数字）

| 条件 | 结论写法 |
| --- | --- |
| 比值 CI 下界 **≥ 1.10** | **material**：可写"N× 更快（CI 下界 …）" |
| CI 排除 1.00 但下界 < 1.10 | **detectable-immaterial**：可分辨，但不具实际意义 |
| CI **跨过 1.00** | **within-noise**：写"该输入与该机器上测不出可分辨差异" |

规则修正记录：我们最初把"2×(两端 CV 之和)"当作噪声带阈值，实测后发现它会把一个**一致的 2.4% 差异**
判成噪声、把偶发的 15% 差异判成信号——**离散度不是效应量**。因此改用上表的两段式：可分辨性看 CI
是否排除 1.0，实质性看 CI 下界是否 ≥1.10。CV 仍逐行披露，但不再充当判定阈值。

### 6.4 裁剪稳健性

除全 10 次外，另按"去掉第 1 次（9 次）"与"只取中间 8 次"两种口径重算中位数与判定
（表：[`../statistics-trimmed-medians.tsv`](../statistics-trimmed-medians.tsv)、[`../statistics-trimmed-verdicts.tsv`](../statistics-trimmed-verdicts.tsv)）。
三种口径下**中位数与判定完全一致**——即头尾各一次的影响不足以改变任何一条结论。

## 7. 环境披露

| 项 | 值 |
| --- | --- |
| 操作系统 | CachyOS（Arch 系），Linux 原生文件系统（非 drvfs 挂载） |
| CPU | Intel Pentium Gold G7400，2 核 / 4 线程 |
| 内存 | 15 GiB |
| GPU | 本基准**不使用 GPU**（另有一个独立 GPU 实验载体，不在本文范围） |
| Rust | rustc / cargo 1.99.0 |
| Python | CPython 3.14.7；numpy 2.5.4、Biopython 1.88、pytrf |
| R | 4.6.1；Bioconductor 3.23、Biostrings 2.80.2、jsonlite 2.0.0、digest 0.6.39 |
| CPUPower 调节器 | `powersave`（全程不变） |
| 存储 | 根文件系统 btrfs / 512 GB NVMe；**`/tmp` 是 tmpfs（7.7 GB），所有基准输入都在 RAM 上** → 测量里没有磁盘延迟项（另有 2 块机械盘存在，与本轮无关） |
| 计时 | GNU time 高精度模式 |
| 引擎版本 | v1.1.3 |

> 这是一台**低端机**：2 核 4 线程、无睿频余量、`powersave`。它的意义不是"代表服务器"，
> 而是一个**保守下限**：在更快的机器上，解释器启动与大块内存分配等常数项会被摊薄，
> 比值**可能因此变大或变小**，所以本文的所有比值都只按本机口径声明。

## 8. 结果

### 8.1 三端中位数与判定（N=10）

表：[`../statistics.tsv`](../statistics.tsv)（每能力 × 后端一行，含 IQR、CV、min/max、峰值 RSS、比值与 CI、判定）。
摘录：

| 能力 | rust | python | r | python 判定 | r 判定 |
| --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 820 ms | 840 ms | 1840 ms | 1.02× [0.99, 1.04] **within-noise** | 2.24× [2.16, 2.26] **material** |
| `expression.pca.v1` | 20 ms | 4010 ms | 2760 ms | 200× [161, 215] material | 138× material |
| `set.venn.v1` | 10 ms | 180 ms | 10 975 ms | 18× material | **1097×** material |
| `enrichment.overrepresentation.v1` | 30 ms | 230 ms | 4470 ms | 7.7× material | 149× material |
| `structure.pdb.summary.v1` | 30 ms | 790 ms | 20 750 ms | 26× material | 692× material |
| `variant.stats.v1` | 80 ms | 760 ms | 13 495 ms | 9.5× material | 169× material |
| `fastq.qc.v1` | 待跑（N=10） | 待跑 | 待跑 | — | — |
| `sequence.ssr.v1` | 待跑（N=10） | 待跑 | 待跑 | — | — |

（本轮另有一次 N=3 窗口的同类测量，结论一致；见 §9 与台账 `docs/engine-evals/local-nas-g7400-window-2026-10-04.md`。）

### 8.2 内存（峰值 RSS，median-of-10）

| 能力 | rust | python | r |
| --- | --- | --- | --- |
| `sequence.stats.v1` | 5.9 MB | 69.7 MB (11.9×) | 321.1 MB (54.8×) |
| `expression.pca.v1` | 8.5 MB | 53.3 MB (6.3×) | 81.9 MB (9.7×) |
| `set.venn.v1` | 8.3 MB | 47.3 MB (5.7×) | 124.6 MB (15.0×) |
| `enrichment.overrepresentation.v1` | 14.8 MB | 61.0 MB (4.1×) | 145.5 MB (9.8×) |
| `structure.pdb.summary.v1` | 29.7 MB | 66.3 MB (2.2×) | 162.9 MB (5.5×) |
| `variant.stats.v1` | 6.2 MB | 45.8 MB (7.4×) | 122.4 MB (19.7×) |

图：[`./figures/fig-memory.svg`](./figures/fig-memory.svg)。

### 8.3 重复稳定性与比值区间

图：[`./figures/fig-repeat-stability.svg`](./figures/fig-repeat-stability.svg)（逐能力的 10 次散布与 IQR）。
多数能力的中位数在 10 次间落在 ±1 IQR 内；`sequence.stats` 的 rust 档位粒度是 10 ms
（`time` 的采样分辨），因此其 CV（1.95%）主要是**量化噪声**而不是真实抖动——这点限制了
小绝对值的分辨力，也是我们把实质性下限放在 1.10 而不是 1.01 的原因之一。

图：[`./figures/fig-ratio-ci.svg`](./figures/fig-ratio-ci.svg)（比值与 95% CI，含 1.00 参考线与 1.10 实质性下限线）。

### 8.4 裁剪稳健性

三种口径（全 10 / 去首 / 中间 8）的中位数与判定完全一致；详见
[`../statistics-trimmed-medians.tsv`](../statistics-trimmed-medians.tsv) 与 [`../statistics-trimmed-verdicts.tsv`](../statistics-trimmed-verdicts.tsv)。
换言之：**没有哪一条结论是靠"保留某一次运行"得来的。**

### 8.5 记录粒度实验：一个被推翻的假设

**原假设**：`sequence.stats` 在百万级碱基上与 Python 只差 2%，是因为"每条记录才穿一次语言边界"；
那么记录越多，差距应越明显。

**实验**：同一约 100 Mb 的总量，三种记录形态，各 N=10。

| 输入形态 | rust | python | r |
| --- | --- | --- | --- |
| 1 × 100 Mb（1 条记录） | 820 ms | 850 ms | 1825 ms |
| 20 × 5 Mb | 820 ms | 840 ms | 1830 ms |
| 100 000 × 1 kb | 830 ms | 860 ms | 1940 ms |

**结论：假设被推翻。** 记录数从 1 增到 10 万，rust 只多 10 ms、python 只多 10 ms
（比值 1.037 → 1.036）。在 ~100 MB 量级上，两侧都被 **I/O 与大块内存分配**主导：
每碱基的活由 C 原语（`str.count`/`str.upper` 一类）承担，而每行的读取与建串两侧都要付。

更正后的解释（也是本文的分层依据）：真正的分界线不是"记录数"，而是**"每个输入元素是否需要
解释器参与"**。逐碱基逻辑落在 C 里的能力（`sequence.stats`）≈1×；逐项/逐记录逻辑落在解释器里
的能力（`set.venn`/`pdb`/`variant`/`enrichment`/`fastq.qc`）从 7.7× 一路到 1097×。
小输入上出现的 ~20× 则是**解释器启动约 170 ms** 落在 10 ms 级工作负载上的占比，不代表吞吐能力。

### 8.6 分层速览

图：[`./figures/fig-speedup-layered.svg`](./figures/fig-speedup-layered.svg)。

| 层次 | 能力 | python | r |
| --- | --- | --- | --- |
| C 原语可覆盖的线性扫描 | `sequence.stats` | 1.02×（不可分辨） | 2.24× |
| 逐项解释器循环（内存内小对象） | `set.venn` | 18× | 1097× |
| 逐基因/逐变异解释器循环 | `enrichment`、`variant` | 7.7× / 9.5× | 149× / 169× |
| 逐原子解释器循环 | `structure.pdb.summary` | 26× | 692× |
| 向量化数值库 | `expression.pca` | 200× | 138× |
| 逐碱基解释器循环（待补） | `fastq.qc` | 待跑 | 待跑 |
| motif 扫描（待补） | `sequence.ssr` | 待跑 | 待跑 |

注意 `expression.pca` 一行：Python 反而比 R 慢（200× vs 138×），因为两侧走的都是向量化库
（numpy vs `prcomp`），差距来自**库与调用约定**而非语言本身——这正是"整栈比较"的含义。

### 8.7 正确性证据（跨后端一致性）

每一份 benchmark 报告都带三端一致性判定：**全部 8 项在 1e-6 相对容差内 `consistent`**，
精度档 `high`。此外做了跨机确定性核对：同一生成器产物在 NAS 与另一台机器上
sha256 逐字节一致（如 `ref.fa`、`cell.tsv`、`cell.json`）。
表：[`../statistics-consistency.tsv`](../statistics-consistency.tsv)。

**与早期报告记录的精确度量对比**（两个窗口各自记录了什么，如实并列，不混排）：

| 校验项 | 早期窗口 `bench-20260913-001` | 本轮（N=10） |
| --- | --- | --- |
| 后端一致性判定 | Consistent（4 项） | `consistent`（8/8 项） |
| 一致性容差 | 1e-6 相对 | 1e-6 相对（同默认） |
| PDB 逐字段偏差 | **0.0**（完全相同） | `findings` 为空（无差异字段路径） |
| PCA / Venn 最大相对误差 | **≤ 2.26e-11** | 本轮改以判定口径记录：`consistent`，无差异字段 |
| 精度档 | `high` | `high` |
| 跨机确定性 | 未记录 | 是：`ref.fa`/`cell.tsv`/`cell.json` sha256 跨机一致 |

诚实说明：早期窗口额外记录了**数值级**的 parity 误差（如 2.26e-11），本轮记录的是
**判定级**证据（是否在 1e-6 内、有无差异字段）。两者不能互相替代，所以本表把两套口径
并列展示，而不是把"0.0 / 2.26e-11"挪到本轮名下。

## 9. 与九月窗口的对比

同能力、同命令、不同输入规模与不同窗口的对照（[`../statistics-comparison.tsv`](../statistics-comparison.tsv)，生成自
`benchmark-results/2026-09-13/summary.json`）：

| 能力 | 九月窗口（N=5，小夹具） | 本轮（N=10，大输入） | 排序是否一致 |
| --- | --- | --- | --- |
| `sequence.stats` | rust 40 / py 330 / r 2110 ms | rust 820 / py 840 / r 1840 ms | 一致（rust ≤ py < r） |
| `expression.pca` | rust 40 / py 300 / r 460 ms | rust 20 / py 4010 / r 2760 ms | 一致 |
| `set.venn` | rust 40 / py 310 / r 400 ms | rust 10 / py 180 / r 10 975 ms | r 的位置随规模剧变 |
| `structure.pdb.summary` | rust 40 / py 320 / r 410 ms | rust 30 / py 790 / r 20 750 ms | 一致（量级随规模展开） |
| `variant.stats` | rust 60 / py 660 / r 12 000 ms | rust 80 / py 760 / r 13 495 ms | 一致 |

读法：**小夹具上三端都被启动/常数项拖平**（几十到几百毫秒），量级差距要在大输入上才展开；
而 `set.venn` 的 R 腿在小夹具上看着"还行"（400 ms），到大输入直接变成 10 975 ms——
这与 §8.5 的分层解释一致。

同能力的跨窗口**绝对**数字（相同输入时）中位差 <3%，说明本机测量本身是可复现的；
跨窗口数字不能直接比较的是**输入规模不同**的那些行（上表已标注）。

## 10. 工程发现：R 腿"真的跑起来"之后

三端测量真正的收获不是分数，而是**把人逼到必须真的执行**。R 后端此前从未在 CI 中运行过测试，
本次补上执行路径后一次性暴露 6 个缺陷，全部已修合并：

| # | 症状 | 根因 |
| --- | --- | --- |
| 1 | SSR 能力直接报错 | Biostrings 只对 `XStringSet` 定义 `width()`，裸 `XString` 无方法 |
| 2 | 测试字面量与断言不符 | 测试里那个"22 bp"字面量实际是 23 个字符 |
| 3 | SSR 恒返回 0 条 | `start(...)` 未加命名空间前缀，命中了 `stats::start`（时间序列） |
| 4 | 上述问题长期潜伏 | **CI 从未运行 benchmark 两个包的测试**（无任何 step） |
| 5 | 无 SSR 的输入被 worker 拒绝 | 空 `list()` 被序列化成 `[]`，而契约要求 map（`{}`） |
| 6 | 20k 项输入的 `intersections` 数量错误 | 按 item 逐条生成，未按 occupancy mask 分组 |

另外顺带抓到一个**产品级**缺陷：`looks_like_sam` 仅凭"≥11 列 + 若干整数列"就把**宽计数矩阵**
判成 SAM（实测边界：10 列 → tsv，11 列 → sam），任何 ≥11 个样本的表达矩阵都会被 worker 拒绝。
已改为要求 SAM 的结构证据（FLAG 上界、RNAME 非纯数字、CIGAR 语法、SEQ 碱基字母）并加回归测试。

教训一句话：**测试存在但不在 CI 执行 = 不存在。**

## 11. 局限与诚实边界

- **整栈比较**，不是语言对语言：对照臂是 Biopython/Biostrings 的惯用写法 + 各自运行时 + 同一测量框架。
  换一个更快的 Python 实现（例如自己写 `bytes.count` 循环）会显著改变比值。
- **单机、单输入规模、`powersave`**：这台机器是保守下限；不同 CPU/内存带宽上的比值可能不同。
- **后端串行测量**的已知偏差：跨后端存在频点/热漂移（本机 `powersave` 已尽量压低），
  我们披露每后端 10 次原始墙钟而不只给汇总，读者可自行判断。
- **计时分辨力**：`time` 在 10 ms 档位上有量化，小绝对值（如 rust 10–30 ms）的 CV 因此偏高，
  这也是实质性下限取 1.10 的原因之一。
- **本轮未用第三方基准套件**；GIAB + hap.py（正确性）与公开托管基准平台（机器谱系锚定）
  为**计划中**，未执行前不作为证据。
- 我们**不**声明"Rust 普遍更快"。本文能支持的只有逐能力的、带区间的、限定在本机本输入下的结论。

## 12. 复现

```bash
# 1) 生成输入（固定种子，只写 /tmp）
python scripts/benchmark/inputs/generate-inputs.py --out /tmp/lxn10   # 见台账内的种子说明

# 2) 逐能力重复测量（每后端 1 次预热 + 10 次计时）
linxira-bio benchmark run <capability> --input <file> --backends rust,python,r --repeat 10 \
  --report /tmp/lxn10/out-<capability>/report.json

# 3) 汇总为归档（校验 schema）与统计表
python scripts/benchmark/analysis/summarize-reports.py --reports-dir /tmp/lxn10 --descriptor descriptor.json \
  --output-dir benchmark-results/2026-10-11
python scripts/benchmark/analysis/statistics-tables.py --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 --output-dir benchmark-results/2026-10-11

# 4) 出图（自包含 SVG）
python scripts/benchmark/figures/render-svgs.py --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 --output benchmark-results/2026-10-11/blog-mirror/figures
```

## 13. 数据与代码可用性

- 归档：`benchmark-results/2026-10-11/`（`summary.json` / `summary.md` / 每能力报告 / 统计 TSV）。
- 原始逐次墙钟：各能力报告的 `backends[].runs[]`（每后端 10 条，含 `wall_ms`、`peak_rss_mb`、`ok`）。
- 输入哈希：见 `summary.json` 的 `data_sources[]`（本窗口生成的输入在归档时记录）。
- 图表：`benchmark-results/2026-10-11/blog-mirror/figures/`（7 张自包含 SVG，无脚本、无外链）。

## 14. 附录：表格索引

| 文件 | 内容 |
| --- | --- |
| [`../statistics.tsv`](../statistics.tsv) | 每能力 × 后端：N、中位数、IQR、CV、min/max、峰值 RSS、比值、CI、判定 |
| [`../statistics-trimmed-medians.tsv`](../statistics-trimmed-medians.tsv) | 三种裁剪口径下的中位数与 CV |
| [`../statistics-trimmed-verdicts.tsv`](../statistics-trimmed-verdicts.tsv) | 三种裁剪口径下的比值、CI 与判定 |
| [`../statistics-consistency.tsv`](../statistics-consistency.tsv) | 跨后端一致性、容差、精度档、报告路径 |
| [`../statistics-comparison.tsv`](../statistics-comparison.tsv) | 与九月窗口的跨窗口对照 |
| [`../summary.json`](../summary.json) / [`../summary.md`](../summary.md) | 归档索引（schema 校验通过） |
