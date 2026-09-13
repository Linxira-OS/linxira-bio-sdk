# Benchmark summary — 2026-09-13（三端基线 / three-way baseline）

> **范围（Scope）**：这是仓库内置 fixture 上的 **Rust + Python + R 三端基线**，用于验证三实现
> 数值一致并量化 Rust 相对解释器实现的启动/内存优势。**不是** 4TB 真实数据集跑分（那批等
> Linux 服务器空闲后按 `docs/BENCHMARK_DATA_SOURCES.md` 的来源清单执行）。
> This is the repository-fixture **three-way baseline** (Rust vs Python vs R) for
> numerical-consistency validation and start-up/memory comparison. It is **not**
> the 4TB real-dataset benchmark.

- 后端（backends）：`rust,python,r` — 重复（repeat）：5（每后端先做 1 次不计时的预热 / one
  untimed warmup per backend before the timed repeats）
- 页缓存（page cache）：warm — warm-cache 数字不得与 cold-cache 混用（ROADMAP 7.1）
- 数据集（datasets）：全部来自仓库 fixture，见下表 `dataset` 列（no external data）

## 结果（Results）

| capability | dataset | dataset_class | 一致性 | rust (ms) | python (ms) | r (ms) | speedup | memory saving |
|---|---|---|---|---|---|---|---|---|
| sequence.stats.v1 | tiny.fa（3 条 FASTA） | sequence | Consistent | 40.0 | 330.0 | 2110.0 | 8.25x | 86.5% |
| expression.pca.v1 | deseq2-counts.csv（components=3） | matrix | Consistent | 40.0 | 300.0 | 460.0 | 7.50x | 85.1% |
| set.venn.v1 | sets.tsv（默认参数） | other | Consistent | 40.0 | 310.0 | 400.0 | 7.75x | 85.7% |
| structure.pdb.summary.v1 | alphafold-style.pdb（默认模式） | structure | Consistent | 40.0 | 320.0 | 410.0 | 8.00x | 86.3% |

- `speedup` = `median_wall(首个非 rust 后端) / median_wall(rust)`；此处对比后端为 **python**。
  对比 R 的话：sequence.stats 52.75x、pca 11.5x、venn 10.0x、pdb 10.25x。
- `memory saving` = `1 - median_peak_rss(rust) / median_peak_rss(python)`。
- 中位峰值 RSS（median peak RSS）：rust ≈ 5.9–6.5 MB；python ≈ 43.8–44.0 MB；
  r ≈ 70.8–70.9 MB（sequence.stats 的 R 走 Biostrings，237.6 MB）。
- 一致性（consistency）均为 **Consistent（高精度）**：三端结果包在 1e-6 相对容差内逐字段
  一致（pack 对照测试对 Rust golden 的最大相对误差：PCA/Venn 2.26e-11，PDB 0.0）。
- 微小 fixture 上解释器启动占主导：`wall_ms` 的主体是 Python/R 进程冷启动，这也是
  self-reported（进程内）计时表存在的原因——见各 `*.benchmark.md`。

## Environment（环境披露 / disclosure）

报告生成于 **WSL2 内**，因此同时披露 Windows 宿主机与 WSL 客户机两层环境。

- os: linux 6.18.33.2-microsoft-standard-WSL2
- distro: Arch Linux（WSL distro: arch-linux-current, wsl2）
- guest cpu: Intel(R) Core(TM) Ultra 5 225H
- guest memory: 7940 MB（WSL2 分配额，非物理内存）
- host os (Windows): Microsoft Windows [Version 10.0.26200.9168]（Windows 11）
- host model: XIAOMI REDMI Book Pro 14 2025
- host cpu: Intel(R) Core(TM) Ultra 5 225H（14 logical processors）
- host memory: 32189 MB（≈31.4 GiB 物理内存）
- engine: linxira-bio-cli 1.0.1（release 构建，Rust）
- python: Python 3.14.6（Arch Linux 系统 python，pack 依赖纯标准库 + Biopython 1.88）
- R: R version 4.6.1 (2026-06-24) "Happy Hop"（pack 依赖 jsonlite/digest/Biostrings 2.80.2）
- container: no（原生 WSL2，无 Docker 层）
- page cache: warm；timing precision: high（`/usr/bin/time -v`）

### 采集中文 Windows 版本串的说明（localized `ver` note）

`cmd.exe /c ver` 的 "版本" 一词随系统区域设置以 OEM 代码页输出；引擎只提取括号内稳定的
ASCII build 号并规范化为 `[Version <build>]`，避免任何区域设置下出现乱码。

## 文件（Files）

- `summary.json` — 本摘要的机器可读版（含全部 entries 与完整 environment 对象）
- `<capability>.benchmark.json` / `.md` — 各能力完整报告（逐次运行、min/max/IQR、
  self-reported 计时、一致性 findings）

## 复现（Reproduce）

```bash
# WSL2 (Arch) 内，仓库根目录：
export LINXIRA_BIO_WORKFLOW_ROOT=/tmp/lx-workflows   # workflows 副本，避开 repo 内 Windows 构建缓存
cargo build --release -p linxira-bio-cli             # CARGO_TARGET_DIR=/tmp/lt
target/release/linxira-bio benchmark run structure.pdb.summary.v1 \
  pdb=tests/fixtures/structure-pdb-summary/alphafold-style.pdb \
  --backends rust,python,r --repeat 5 --output benchmark-results/2026-09-13 --dataset-class structure
# 其余三个能力同理（sequence.stats.v1 / expression.pca.v1 / set.venn.v1）
```
