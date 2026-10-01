# 云租赁测试计划 —— 2026-10-02 第一轮（V100/T4 + MTT S4000）

> 目的：租赁窗口内的量化测试（G5 博客的"用户允许的量化测试"即本轮）；首要测试
> **分子对接**（chemistry.dock.v1），其次 wgpu 基线跨卡数据点、厂商栈边界、SSR 三方
> parity。每台机器 1-3 小时窗口，源码临场克隆编译。
> 传输包已构建：`release-artifacts-rental/linxira-rental-bundle-2026-10-02.tar.gz`
> （SHA256SUMS.txt 同目录；重建/校验：`python scripts/prepare-rental-bundle.py [--check]`）。

## 1. 机器配置（租用渠道与时价不入库，仅记技术配置）

| 机型 | CPU/内存 | 驱动与 CUDA | SM | 本轮角色 |
| --- | --- | --- | --- | --- |
| Tesla T4 / 16GB | 8 核 Xeon / 56GB | 550.90-580.65，CUDA ≤12.4-13.0 | sm_75 | wgpu/Vulkan 尝试 + 对接 + parity + cutile-rs sm 下限记录 |
| V100-32GB | 6 核 Xeon Gold 6130 / 25GB | 525.89-580.82，CUDA ≤12.0-13.0 | sm_70 | 同上（二选一，优选驱动 580 机型即 CUDA ≤13.0） |
| MTT S4000 / 48GB | 15 核 Xeon Gold 6430 / 100GB | MUSA 驱动 2.7.0 | MUSA mp | wgpu/Vulkan（官方 Vulkan 1.3）+ cudarc-musa git 尝试 + 对接 |

> cutile-rs 实测需 sm_80+ 档机型；本轮三张卡均在其下限之下（sm_70/75/MUSA），
> 故本轮只固化边界数据，cutile-rs 真实测顺延到下一个 sm_89 档租用窗口。

**SM 与 CUDA 版本口径（重要，决定验收预期）**：cutile-rs 的约束是 **SM 下限
sm_80+**（工具链声明），与 toolkit 支持面是两回事——CUDA 13 移除的是 pre-Turing
（sm_60 以下保留到 sm_60），V100（sm_70）/T4（sm_75）装 CUDA 13 没有问题，但
cutile-rs 在这两张卡上**预期不可用**（记录失败文本即验收）；cudarc 系无自身 SM
下限，跟随 toolkit/driver。750 Ti（sm_50）仅 CUDA 12.x，留学校机器回测，不进本轮。

## 2. 测试清单（每机时序，1-3 小时窗口）

| # | 测试 | 时限 | 内容 | 命令入口 |
| --- | --- | --- | --- | --- |
| T0 | 环境与构建 | ≤15min | 克隆 + release 构建 + doctor + 环境审计（Linux 正式工况首跑） | `git clone` → `cargo build --release -p linxira-bio-cli` → `environment audit` |
| T1 | **分子对接（首要）** | ≤30min | 官方 1iep 教程数据（伊马替尼/c-Abl 激酶域）经 chemistry.dock.v1 全链；同种子复跑 | `mamba install -c conda-forge autodock-vina` → `chemistry dock … --seed 42 --json` |
| T2 | wgpu 基线 | ≤30min | probe + gpu-bench（pearson v1/v2/v3 + 直方图）——**首批非 iGPU 数据点** | `gpu-lab probe` / `gpu-bench` |
| T3 | 厂商栈边界 | ≤40min | NVIDIA：gpu-cutile feature 编译（sm_70/75 预期边界记录）；S4000：cudarc-musa git 依赖尝试 + MUSA 驱动/SDK 版本核对 | `cargo build --features gpu-cutile`；scratch crate + git dep |
| T4 | SSR 三方 parity | ≤20min | rust vs pytrf（Linux 装锁）同输入逐字段一致 + 百万碱基基准 | `sequence ssr … --backend rust/python` |

## 3. 验收清单（PASS 判据，逐条勾选回填台账）

| 测试 | 判据 | 记录物 |
| --- | --- | --- |
| T0 | doctor/audit status=ok；audit 列出 samtools/vina 类工具探测行 | audit-<host>.json |
| T1a 对接功能 | exit 0；JSON status=ok；`docking.modes ≥ 3`；`best_affinity_kcal_per_mol ∈ [-14.5, -12.0]`（官方预期 **-13.234**） | dock.json |
| T1b 可复现 | 同种子（42）复跑：最佳亲和力与 mode 数**完全一致**；out.pdbqt 字节级一致 | dock-2.json、out.pdbqt |
| T2 wgpu | probe 列出租用卡（型号+backend）；gpu-bench 全 kernel pass=true（直方图整数精确、pearson |Δ|≤1e-5）；分相中位数入表 | gpu-bench-<card>.json |
| T3a NVIDIA 边界 | sm_70/75 上 cutile-rs 编译/运行结果与官方 sm_80+ 声明一致，错误原文记录（**预期失败也是验收**） | build log |
| T3b MUSA | S4000 上 musa-smi 版本、cudarc-musa git 构建结果、与 MUSA SDK 5.2 ABI 的匹配结论（驱动 2.7.0 与 SDK 5.x 的错位是首批窗口检查点） | build log + 版本快照 |
| T4 parity | 同输入 rust/pytrb 两路 `summary` 逐字段相等（records 含 start/motif/repeats/compound）；1M bases 计时入表 | ssr_*.json/tsv |

台账回填：`docs/engine-evals/gpu-rental-2026-10-02.md`（按 gpu-wgpu 系列格式：3 次
取中位、upload/compute/readback 分相、容差显式、Windows/临时值口径替换为 Linux 正式值）。

## 4. 风险与预案

| 风险 | 预案 |
| --- | --- |
| Tesla 数据中心驱动可能不暴露 Vulkan → T2 无 non-CPU adapter | 该结果本身入台账（回退层覆盖面结论）；S4000 的 Vulkan 1.3 是本轮 Vulkan 主数据点 |
| MUSA 驱动 2.7.0 与 cudarc-musa 的 MUSA SDK 5.2 ABI 错位 | 只记录结论不改代码；issue 素材归档（提案 §2.2） |
| 本轮无 sm_80+ 机型 → cutile-rs 实测顺延 | 本轮先固化 sm_70/75 边界数据；sm_89 档机型租用窗口直接执行 cutile 执行卡 |
| GitHub 拉取慢 | 租用平台的 GitHub 加速或镜像；bundle 走 FTP/网页上传 |
| 25GB 内存的 V100 机型构建吃紧 | 只构建 `-p linxira-bio-cli`（跳过 workspace 全量）；数据盘 50GB 足够 |

## 5. 博客（G5）规范 —— 技术论文体 + 参考文献

本轮量化测试完成后方可动笔（G5 红线）。结构按技术论文：摘要 → 背景与相关工作 →
方法（机器/驱动/版本/命令，全部可复现口径）→ 结果（分相表、跨卡对照、对接验收）→
讨论（回退层覆盖面、厂商栈边界、统一内存视角）→ **参考文献**。参考文献初稿（成文时
逐条核对页码/DOI 后引用）：

1. Trott, O.; Olson, A. J. AutoDock Vina: improving the speed and accuracy of
   docking with a new scoring function, efficient optimization, and
   multithreading. *J. Comput. Chem.* **2010**, 31(2), 455-461.
2. Eberhardt, J.; Santos-Martins, D.; Tillack, A. F.; Forli, S. AutoDock Vina
   1.2.0: New Docking Methods, Expanded Force Field, and Python Bindings.
   *J. Chem. Inf. Model.* **2021**, 61(8), 3891-3898.
3. Schindler, T.; Bornmann, W.; Pellicena, P.; Miller, W. T.; Clarkson, B.;
   Kuriyan, J. Structural mechanism for STI-571 inhibition of abelson tyrosine
   kinase. *Science* **2000**, 289(5486), 1938-1942.（PDB 1IEP 结构）
4. Thiel, T.; Michalek, W.; Varshney, R.; Graner, A. Exploiting EST databases
   for the development and characterization of gene-derived SSR-markers in
   barley (*Hordeum vulgare* L.). *Theor. Appl. Genet.* **2003**, 106(3), 411-422.（MISA）
5. Benson, G. Tandem repeats finder: a program to analyze DNA sequences.
   *Nucleic Acids Res.* **1999**, 27(2), 573-580.
6. Du, L.; et al. pytrf: a Python package for finding tandem repeats from
   sequences. **2025**. PMC12135533.
7. Li, H.; Handsaker, B.; Wysoker, A.; Fearnhead, P.; Hetzel, N. et al. The
   Sequence Alignment/Map format and SAMtools. *Bioinformatics* **2009**,
   25(16), 2078-2079.
8. Pages, H.; Aboyoun, P.; Gentleman, R.; DebRoy, S. Biostrings: Efficient
   manipulation of biological strings. Bioconductor R package.
9. NVIDIA. CUDA Rust: cutile-rs and cuda-oxide（官方公告，2026-09-08，
   developer.nvidia.com）。
10. Moore Threads. cudarc-musa（cudarc 官方 fork，2026-09-23；MUSA SDK 文档
    mthreads.com）。
11. Intel. oneapi-rs / sycl-rs. https://github.com/oneapi-src/oneapi-rs
12. gfx-rs. wgpu. https://wgpu.rs ；Khronos. Vulkan 1.3 Specification.
13. Halgren, T. A. Merck molecular force field. I. Basis, form, scope,
    parameterization, and performance of MMFF94. *J. Comput. Chem.* **1996**,
    17(5-6), 490-519.（若文章涉 conformers 链）

数据出处声明：对接测试数据为 AutoDock Vina 官方教程 `example/basic_docking`
（Scripps，Apache-2.0 仓库内文件），bundle MANIFEST.SHA256 固定哈希；模拟序列由
本 SDK `simulate.sequence.v1` 固定种子生成（种子随台账记录）。

## 6. 待办回填

- [ ] 租用后回填：各机 T0-T4 结果到 `gpu-rental-2026-10-02.md`
- [ ] sm_89 档机型租用窗口 → cutile-rs 执行卡（提案 §1.1/§2.3）
- [ ] 博客草稿 → `benchmark-results/2026-10-xx/`（本轮验收全 PASS 后）
