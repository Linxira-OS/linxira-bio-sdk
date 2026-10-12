# M5 并发/IO 方向评审（批次 J 前置调查）

> 状态：评审结论，未动工。日期：2026-10-12。用户指令：「这批次先调查清楚要往哪些方向来走比较合适，
> 有没有些公开 benchmark 给出方案来，然后我觉得在开发」——本文是那份调查。
> 证据来源：四路公开调查（压缩/解压基准、Rust crate、基准方法学、公开套件与数据集）+ 本地代码勘察；
> 所有外部结论附可点开出处，无法一手核实的标注置信度。
> 本文只决定「往哪走、按什么口径验收」；具体实现与验收在日常任务里跟踪（ROADMAP §3.1 批次 J）。

## 0. 结论摘要（TL;DR）

1. **并行解压只有一条"构造上正确"的路：BGZF**。BGZF 规范即「≤64 KiB 的独立 gzip 成员串联」，
   按块并行无格式歧义、输出天然逐字节一致（SAMv1 规范；htslib `bgzf_mt` 连**解码**也并行）。
2. **纯 `.gz` 不能"分块并行"**：pigz 手册作者明说解压无法并行；要并行必须换机制——
   索引辅助（`gzi`/mim）或两段试探解码（rapidgzip/pugz）。**原 M5-T1 里「普通 gzip 分块并行」的写法不成立，必须改写。**
3. **不并行也有 2× 可拿**：换 DEFLATE 后端即可——libdeflate 解压较 vanilla zlib ~2–2.3×（htslib 自测 1m17s→0m43s），
   zlib-rs 较 zlib-ng 解压快 ~10%。**优先级应高于引入线程。**
4. **公开的最接近数据点**：htslib 4 线程 BGZF 解码 15M reads BAM：**9.028 s → 2.592 s（≈3.5×）**，
   与我们的 2C/4T 门禁机同量级；16 线程的 5×/13× 是**多核数据，不得当作 2C/4T 结果引用**。
5. **crate 结论**：BGZF 用 `noodles-bgzf` **≥0.52**（MIT，默认纯 Rust `zlib-rs`，Windows-gnu 无需 C 工具链）；
   **不引入 `libdeflater`**（windows-gnu 风险最高、且只能整缓冲不能流式）；不引入 `seq_io::parallel`（实验性、乱序）、
   不引入 `needletail` 并行（无 API）；`thread_io` 休眠 → 自研 `std::thread + sync_channel`（~30 行）；
   `rayon` 必须固定线程数与归约顺序；`zlib-rs` 是 `Zlib` 许可，需过依赖白名单。
6. **测量口径必须升级**（并发改变了披露义务）：线程数、SMT/核数、governor/实际频率、缓存与内存、
   解压器身份（`dctx`）与池线程数（`RAYON_NUM_THREADS`）逐点披露；曲线按**强扩展 1/2/4**并预注册；
   4 线程是 **2 核 × 2 硬线程**，不得写成"4 核"。
7. **公开锚点**：SPEC CPU 规则当披露模板（不宣称合规）；PTS/OpenBenchmarking 可上传但**无 bgzip profile**；
   压缩对照用 Silesia/Squash/lzbench；生信数据用 GIAB HG002（唯一明确同意商业再分发的 trio）+ IGSR/1000G 阶梯；
   正确性真值（GIAB + hap.py）与速度**分开陈述**。

## 1. 现状事实（代码级，动工前基线）

| 项 | 事实 | 出处 |
| --- | --- | --- |
| 现有解压 | 全部是 `flate2::read::MultiGzDecoder` 单线程串行；多文件重复同一写法 | `linxira-bio-core/src/{alignment,annotation,cohort,coordinate,curve_fit,dataset,domain,expression,fastq,…}.rs:1` |
| 后缀识别 | `.bgzf` / `.bgz` / `.gz` 已识别（BGZF 走同一 gzip 解码器） | `linxira-bio-core/src/dataset.rs:471` |
| 依赖 | 仅 `flate2 = "1.1"`（core + output）；无 rayon/tokio/noodles/libdeflate（`Cargo.lock` 核对） | `linxira-bio-core/Cargo.toml:11` |
| 工具链 | `edition = "2024"`、`rust-version = "1.92"`（本机 rustc 1.97.1）→ noodles-bgzf 0.52 的 MSRV 1.91 可满足 | `Cargo.toml:17-18` |
| 报告字段 | `BenchmarkRun` 已有 `cpu_ms`/`peak_rss_mb`/`disk_read_bytes`/`disk_write_bytes`/`started_at`/`self_reported_wall_ms`；环境快照含 os/kernel/cpu_model/memory/storage/gpu | `linxira-bio-protocol/src/lib.rs:578-670` |
| CI 现实 | windows-gnu + debian:bookworm + archlinux:base 三平台；`gpu-lab` 被排除出 workspace | `.github/workflows/ci.yml:11-12,162,380,431`；`Cargo.toml:9` |
| 既有纪律 | DETERMINISM 红线：benchmark 数值比较须同输入/同线程数/同分块顺序（ROADMAP §9.4 注） | `ROADMAP.md` §9.4 |

## 2. 方向一：并行解压——BGZF 是唯一"构造上正确"的路

- **BGZF 规范**：「a series of concatenated BGZF blocks, each no larger than 64 KiB … Each BGZF block is itself
  a spec-compliant gzip archive」，且 gzip 允许成员串联，解压结果=各成员串联 ⇒ 按块并行无歧义，输出逐字节一致。
  出处：<https://samtools.github.io/hts-specs/SAMv1.pdf>（置信度：一手）
- **上游实现**：`bgzf_mt(fp, n_threads, n_sub_blks)` 参数写明 `#threads used for reading / writing`，
  `bgzf_thread_pool` 让**编解码共享同一工作池**；`bgzip` 在 `threads>1` 时对**解码**路径同样启用。
  出处：<https://github.com/samtools/htslib/blob/develop/htslib/bgzf.h>（一手）
- **公开量级**：15,077,640 reads BAM 的 `flagstat`，1 线程 9.028 s → `nthreads=4` 2.592 s（≈**3.5×**，user 时间 8.97→9.66 s）。
  出处：<https://github.com/samtools/htslib/issues/433>（维护者 jkbonfield，2016，N 小、机制未变；标注为一手但陈旧）
- **不得挪用**：htslib 论文的 16 线程 5×/13×（BAM→BAM / BAM→SAM）是多核结果，不适用于 2C/4T 门禁。
  出处：<https://academic.oup.com/gigascience/article/10/2/giab007/6139334>

**决定**：`decompress` 的并行能力**只对 BGZF 提供**；BGZF 走块并行，输出与 `io::Reader`（串行）逐字节一致作为验收断言。

## 3. 方向二：纯 gzip——不做"分块并行"，做"更快串行 + 可选索引"

- **权威否定**：pigz 手册（Mark Adler）：「Decompression can't be parallelized, at least not without specially
  prepared deflate streams … As a result, pigz uses a single thread for decompression」。
  出处：<https://manpages.ubuntu.com/manpages/focal/man1/pigz.1.html>（一手）
- **真正可行的替代机制**（都要额外前提，故列为可选路径而非默认）：
  - 索引辅助：`gzi`（BGZF 自带）/ `mim`（BSD-3，C++17，建索引 ~20% 开销、近线性扩展）——
    出处：<https://pubmed.ncbi.nlm.nih.gov/41394640>
  - 两段试探解码：`rapidgzip`（MIT，C++17）：128 核 33–55×；12C/24T 下带索引 2 线程 3.6×、12 线程 17.7×；
    `pugz`（研究原型）32 线程 611 MB/s vs libdeflate 118 MB/s。
    出处：<https://github.com/mxmlnkn/rapidgzip>、<https://arxiv.org/abs/1905.07224>
  - **xz / zstd 对任意文件都没有并行解码**（仅多块/自产文件）：<https://man.archlinux.org/man/xz.1>、
    <https://github.com/facebook/zstd/issues/2470>
- **不并行也能拿到的收益**：libdeflate 解压 vs vanilla zlib ~**2–2.3×**（htslib 单线程 `test_view -B`：
  原始 zlib 1m17s vs libdeflate 0m43s；TurboBench silesia 478→1127 MB/s）。出处：<http://www.htslib.org/benchmarks/zlib.html>
- **Rust 侧的等价收益**：`zlib-rs`（纯 Rust，`Zlib` 许可）解压较 zlib-ng 快 ~10%（silesia-small，多档 chunk 验证）。
  出处：<https://trifectatech.org/blog/zlib-rs-is-faster-than-c/>（维护者博客，标 secondary）

**决定**：纯 `.gz` 路径：① 先换更快的 DEFLATE 后端（默认仍是纯 Rust、无 C 工具链）；
② 索引路径（`gzi`/外部 `bgzip -r`）作为**可选加速**，没索引就退回串行，绝不假装能并行。

## 4. 方向三：crate 取舍与工程风险

| 角色 | 采用 | 版本/许可 | 关键理由与风险 |
| --- | --- | --- | --- |
| BGZF 并行+串行读写 | `noodles-bgzf` | **≥0.52**，MIT，MSRV 1.91 | 官方文档化的块并行、顺序输出、按实例建池；**必须 ≥0.51**（0.50.0 曾误用 rayon 全局池、把 buffer 数写成 `available_parallelism()+2`，见 issue #410）；默认后端是纯 Rust `zlib-rs`，Windows-gnu 无需 C 工具链 |
| 解压后端（串行基线） | `flate2`（现状保留） | 1.1，MIT/Apache | 作为 `--serial` 对照臂保留；比较"并行 vs 串行"必须同格式同后端 |
| DEFLATE 后端加速 | `zlib-rs`（经 noodles/flate2 feature） | 0.6.x，**Zlib 许可** | 纯 Rust、无 C；许可需过白名单（不是 MIT/Apache，必须显式登记） |
| 分块并行计算 | `rayon` | 1.80，MIT/Apache | 必须：固定 `RAYON_NUM_THREADS`；浮点 `sum/product/min_by/max_by` **非确定**；`fold` 分块边界非确定；用 `reduce`/固定分块顺序保 DETERMINISM |
| 解压↔计算流水线 | 自研 `std::thread + std::sync::mpsc::sync_channel` | — | `thread_io` 3 次发布/休眠、单维护者、错误延迟暴露（队长内）→ 换 ~30 行自研，少一个依赖与一个 bus factor |
| **不采用** | `libdeflater` | MIT | windows-gnu 风险最高：`build.rs` 强依赖 C 编译器、上游无 Windows CI、gzp README 有 LNK2019 警告；且**只能整缓冲**不能流式，不适配 `Read` 接口 |
| **不采用** | `seq_io::parallel` | MIT | 自述实验性、结果乱序、`parallel_records` 因 rust#42950 不可用、0.3 线冻结（0.4.0-alpha.0 停在 2020） |
| **不采用** | `needletail` | MIT | 无并行 API、记录借用读缓冲（并行须拷贝）、无并行吞吐证据 |
| **不采用（现阶段）** | `tokio` | 1.85，MIT | 同步引擎引入运行时/阻塞池/关停语义；`spawn_blocking` 也不能让 CPU 热区变并行 → 待有真实并发 IO 需求再加 |

**7z / 归档**：`7zz` 官方二进制为 **LGPL-2.1-or-later + unRAR restriction**，再分发必须复现许可声明
（<https://www.7-zip.org/license.txt>）；`p7zip` 已停维护、Arch 已移除 → **不捆绑**；
多线程解压仅在多块归档成立（LZMA2/xz 多块），非通用加速。`tools/catalog.json` 应登记探测别名与常见安装路径，
缺失时**自动回退**自研路径并在报告标注（避免"7z 可用/不可用"两套口径混排）。

**领域压缩器一律不作依赖**：Genozip（商业源码可见，非开源）、Illumina DRAGEN ORA（商业）、SPRING（非商业）、
DSRC（GPL-2.0）——许可均不支持在宽松许可 SDK 内链接。仅可作为**外部对照工具**并逐项披露
（公开对照：~68.3 GB、8 线程中位解压 ORA 228 s / Genozip 460 s / SPRING 2133 s / repaq 4785 s）。

## 5. 方向四：测量与验收口径升级（并发改变了披露义务）

采用 SPEC 的**报告规则**作为模板（只借用规则，**不宣称合规**）：

- 运行次数与统计：SPEC 1.2.1 = 3 次取中位（2 次取较慢者）；我们已是 N=10 中位数 + bootstrap CI ⇒ 更严。
- 必披露（我们当前缺）：**测试期启用的核/线程数**、SMT 状态、缓存与内存配置、电源管理设置、编译器与标志、系统状态；
  SPEC 4.7 = 结果必须来自被声明的**那一台**系统（我们已有"不跨机比较"纪律，此处作为规范依据）。
  出处：<https://www.spec.org/cpu2017/Docs/runrules.html>、<https://www.spec.org/products/fairuse/>
- 线程曲线：x 轴=请求线程数（预注册 1/2/4），y=wall 时间，副轴=加速比 `T(1)/T(N)`（同一次会话同二进制）；
  画理想线性与 Amdahl 参考线；报 parallel efficiency；每点报分布（中位数+CI+原始点）；
  **不外推**到未测线程数；2C/4T 上 3/4 线程点标注为 **SMT 硬线程**，绝不写"4 核"。
  出处：Bailey/SPEC/经验标准惯例（见评审收藏的 `thread_scaling_conventions` 列表）
- 环境控制：governor 固定在 `powersave` 时，**内核语义是"请求最低频率"** ⇒ 诚实做法是：
  ① 两臂同约束下比**比值**仍然有效；② 绝对值明确标注为"powersave 约束下的下界"；③ 逐次报告**实测频率**；
  ④ 下一窗口增测 `performance` 作为**同机对照臂**（不改历史数字，只加臂）。出处：内核 CPUFreq governor 文档（一手）
- 池与解压器身份：每个测量点固定并记录 `RAYON_NUM_THREADS`、工作池线程数、解压路径身份
  （`dctx: bgzf-multithreaded | zlib-rs | flate2-serial | 7z | index-assisted`），写入 `BenchmarkRun`/环境快照新字段。
- 预注册：曲线点集（1/2/4）与头条线程数**先写规则再测**，防止事后挑 N。

## 6. 方向五：公开锚点与可比性红线

- **引用锚点**：SPEC Fair Use（基础对比必须声明、取数日期、不得跨大版本比较）；
  PTS `pts/bioinformatics`（hmmer/mafft/mrbayes/qmcpack）与 `pts/compression`（compress-gzip/zstd/7zip/lzbench）——
  有公开 profile XML（运行命令、语料、`TimesToRun=3`、指标解析），且**第三方可上传结果**；但**没有 bgzip profile**，
  且 PTS 明示可比性仅限同版本+同硬件+同日（<https://openbenchmarking.org/suite/pts/bioinformatics>）。
- **压缩对照语料**：Silesia（<https://sun.aei.polsl.pl/~sdeor/index.php?page=silesia>，页面无许可声明）、
  Squash（单线程、每迭代 ≥5 s CPU、可导出 CSV）、lzbench（显式 `-i/-t/-p`）；
  Canterbury 的 E. coli（4,638,690 B）是经典语料里唯一的生物序列；
  Pizza&Chili DNA/PROTEINS（~385 MB/~1.1 GB，带 50/100/200 MB 前缀）适合门禁机规模。
- **生信数据**：**GIAB HG002/NA24385**（公开、无注册、NIST 条款、trio 明确同意商业再分发；
  <https://ftp-trace.ncbi.nlm.nih.gov/ReferenceSamples/giab/>）+ **IGSR/1000G**（公开、逐文件 MD5，可做 1×/4×/10× 阶梯，
  <https://ftp.1000genomes.ebi.ac.uk/vol1/ftp/>）。注意 HG001 不具商业再分发授权，故整篇统一用 HG002。
- **正确性 vs 速度分开**：GIAB + hap.py / GA4GH benchmarking-tools / nf-core/variantbenchmarking 只作**正确性真值**，
  绝不用来支撑吞吐声明。
- **红线（不宣称/不比较）**：不宣称 SPEC 合规；不宣称线性扩展；不外推到未测线程数；
  不把多核集群结果与 2C/4T 结果并列；不把 7z 原生多线程当作 Rust 卖点（须三方并列：Rust 并行 vs 7z 原生 vs 串行 flate2）；
  不把领域压缩器的比率/时间与 gzip 类混排。

## 7. 批次 J 任务集修订（据上述证据）

| 原任务 | 修订后 | 变更理由 |
| --- | --- | --- |
| M5-T1「普通 gzip→libdeflater 或 seq_io::parallel **分块并行**」 | **M5-T1a** BGZF 用 `noodles-bgzf ≥0.52` 块并行（默认纯 Rust 后端）；**M5-T1b** 纯 gzip：只换更快后端 + 可选索引路径，**不做**分块并行；`flate2` 串行保留为基线 | §2、§3：格式层面不可能；写死会做出"看起来并行、实则串行"的假象 |
| M5-T1 依赖 `libdeflater` | **删除** `libdeflater`；Windows-gnu 上改用纯 Rust 后端 | §4：C 工具链 + 只能整缓冲 + 上游无 Windows CI |
| M5-T2 用 `thread_io` | 自研 `std::thread + sync_channel` 流水线 | §4：休眠依赖、bus factor、错误延迟 |
| M5-T3 Rayon 并行热路径 | **保留**，并强制：固定 `RAYON_NUM_THREADS`、浮点归约用固定顺序、串行 vs 并行逐字节一致 | §4 确定性地雷 |
| M5-T5 benchmark 标注 `dctx` | **保留并扩展**：新增线程数与池线程数字段 + 实测频率 + governor；三臂口径（Rust 并行 / 7z / 串行 flate2） | §5 |
| （新增）M5-T10 | **强扩展曲线 1/2/4 + 理想线性 + efficiency + 分布**，2C/4T 上标注 SMT；预注册点集 | §5 |
| （新增）M5-T11 | `performance` governor 同机对照臂（只加臂，不改历史数字） | §5 |
| （新增）M5-T12 | 对照臂分类登记：`language-parity arm` vs `ecosystem-tool arm`（fastp/seqkit/pyfastx 只在 tool arm 出现，禁止混排） | 见 §8 关联更正 |
| M5-T7 评估记录 | **保留**：`docs/engine-evals/` 记录 G1–G5 结论 + Rust-bgzf vs 7z vs 串行三方对照 | §6 红线 |
| M5-T9 文档 | 增补：README/AGENTS 声明"BGZF 可并行、纯 gzip 不可并行、7z 为可选原生工具且许可受限" | §2–§4 |

**验收口径修订摘要**：`cargo test -p linxira-bio-io` 绿 + BGZF 并行解码与 `noodles-bgzf` 串行/`bgzip -d` 输出逐字节一致；
≥64 MiB BGZF：并行 wall < 串行 wall（门禁机只此一条降时断言，避免在 2C 上过度承诺）；
纯 gzip：后端替换后 wall 下降且输出逐字节一致（**不断言**并行加速）；
Rayon 热路径：并行与串行**逐字节一致**；曲线按 §5 口径产出且每点有分布。

## 8. 关联更正（本文评审期间发现，与批次 J 相邻）

以下属于**已发布文本的准确性问题**，独立于 M5，但同属"证据优先"纪律，需在动工前或同批处理：

1. 三语言基准文章 §8.6 把 `expression.pca` 描述为"两侧走的都是向量化库（numpy vs prcomp）"——**事实错误**：
   三个臂均为同一套手写幂迭代（`expression.rs:855/1343`；`expression_pca_v1.py:187`；`expression_pca_v1.R:242`），
   `prcomp`/`scikit-learn` 是被刻意排除的。该行应改述为"三臂同算法，差异来自运行时边界"（并修分层表该行分类）。
2. 同文 §6.4/§8.4 称"三种裁剪口径在**中位数**与判定上完全一致"——**过度声明**：判定 0/16 变化，
   但中位数 14/24 会按计时步长浮动（`statistics-trimmed-medians.tsv`）。
3. GPU 篇的"五类文件 SHA256 跨机完全相同"需补前提：**JSON 信封仅在相同 `--cpu` 下字节一致**
   （唯一机器相关字段 `result.thread_count`，`native_tools.rs:1195`）；`*_out*.pdbqt` 姿态文件与 `--cpu` 无关且逐字节一致。
   台账"双厂复现字节级成立"应改为"数值逐值相同 + 同 `--cpu` 时字节相同"。
4. `ROADMAP.md:229` 的"原生基准 = 该能力生态对标工具，不是自写脚本"与实际发货的 5/8 Python、6/8 R 手写臂**互斥**；
   应改为两类臂通则（parity arm / tool arm）并要求逐行标注。
5. `workflows/org.linxira.benchmark-python/src/implementations/expression_pca_v1.py:3-23` 的 docstring 声称
   "numpy.linalg 用于逐元素数学"，与代码不符（零行为变更的注释修正）。

## 9. 变更记录

- 2026-10-12 初版：据四路公开调查 + 本地勘察，重定批次 J 方向与验收；登记 5 项关联更正。
