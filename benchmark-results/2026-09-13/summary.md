# Linxira Bio SDK — three-backend baseline benchmark (2026-09-13)

> Audience: open-source contributors and anyone evaluating the SDK's
> performance claims. Everything needed to reproduce the numbers is below:
> exact hardware, software versions, data sources (public SRA accessions and
> repository fixtures), command lines, and acceptance criteria.
>
> 报告双语：英文为主，文末附中文摘要（For the Chinese summary, see the last
> section）。

**Report ID**: `bench-20260913-001` · **Code revision**: `9052bcf` · **Engine**: 1.0.1

## 1. What was measured

Rust is the SDK's native engine; the same four capabilities were also
re-implemented in pure Python and pure R inside benchmark packs, so all
three backends run the identical analysis on identical inputs. This run
establishes (a) three-way numerical parity and (b) the start-up/memory
overhead an interpreter pays on small inputs. It is **not** a comparison
against external bioinformatics tools.

| capability | dataset | rust | python | r | speedup (rust→python) | memory saving |
|---|---|---|---|---|---|---|
| `sequence.stats.v1` | 3-record FASTA | 40 ms | 330 ms | 2110 ms | 8.25× | 86.5% |
| `expression.pca.v1` | counts CSV (3 components) | 40 ms | 300 ms | 460 ms | 7.50× | 85.1% |
| `set.venn.v1` | identifier-set TSV | 40 ms | 310 ms | 400 ms | 7.75× | 85.7% |
| `structure.pdb.summary.v1` | AlphaFold-style PDB | 40 ms | 320 ms | 410 ms | 8.00× | 86.3% |

Consistency: all four capabilities **Consistent** across backends at 1e-6
relative tolerance (pack parity tests vs the Rust golden outputs: PCA/Venn
max relative error 2.26e-11, PDB exactly 0.0).

On fixtures this small the wall time is dominated by interpreter start-up;
that is precisely what these numbers characterize. Throughput on real data
is measured separately (see `2026-09-14`).

## 2. Environment (full disclosure)

| item | value |
|---|---|
| OS | Linux (WSL2 guest), kernel `6.18.33.2-microsoft-standard-WSL2` |
| Distro | Arch Linux (`arch-linux-current`), wsl2 |
| CPU | Intel Core Ultra 5 225H, 14 logical processors |
| RAM | 7940 MB visible to the WSL2 guest (host physical: 32189 MB) |
| RAM modules | 32 GiB LPDDR5X-8533 MT/s (SMBIOS reports 8×4 GiB SK Hynix) |
| Storage | NVMe SSDs: Samsung PM991a 512 GB (system) + YMTC PC411 1 TB; the WSL2 guest filesystem lives on NVMe SSD |
| GPU | Intel Arc 130T (integrated, driver 32.0.101.8991) — **not used**; all workloads are CPU-only |
| Host OS | Windows 11, build 26200.9168 (`cmd.exe /c ver`, disclosed via WSL interop) |
| Rust | engine 1.0.1, release profile |
| Python | 3.14.6 (pack: stdlib + Biopython 1.88) |
| R | 4.6.1 (pack: jsonlite, digest, Biostrings 2.80.2) |
| Container | none |
| Page cache | warm (one untimed warmup run per backend, then 5 timed repeats) |
| Timing | `high` (external `/usr/bin/time -v`: wall/CPU/peak RSS) |

## 3. Methodology

- One untimed warmup per backend, then `--repeat 5`; medians reported,
  min/max/IQR in the JSON reports.
- Cross-backend consistency: structured field-level diff, numeric relative
  tolerance 1e-6; any drift lists the differing fields.
- `speedup = median_wall(python) / median_wall(rust)`;
  `memory saving = 1 − median_peak_rss(rust) / median_peak_rss(python)`.
- Warm-cache numbers must not be mixed with cold-cache runs.

## 4. Data sources

All inputs are repository fixtures committed in this repo:

- `tests/fixtures/sequences/tiny.fa` (3-record FASTA)
- `tests/fixtures/expression-matrix/deseq2-counts.csv`
- `tests/fixtures/set-analysis/sets.tsv`
- `tests/fixtures/structure-pdb-summary/alphafold-style.pdb`

No external data was used; nothing here requires credentials or controlled
access.

## 5. Reproduce

```bash
linxira-bio benchmark run sequence.stats.v1 \
  fasta=tests/fixtures/sequences/tiny.fa \
  --backends rust,python,r --repeat 5 \
  --output benchmark-results/2026-09-13 --dataset-class sequence
# repeat per capability: expression.pca.v1 / set.venn.v1 / structure.pdb.summary.v1
```

Raw reports (`<capability>.benchmark.json|.md`) and the machine-readable
`summary.json` live in this directory.

---

## 中文摘要

**测了什么**：四个分析能力（序列统计 / PCA / 韦恩图 / PDB 摘要）在 Rust、
Python、R 三个后端上以相同输入各跑 5 次。三端结果在 1e-6 容差内逐字段一致
（对 Rust golden 的最大相对误差 2.26e-11 / 0.0）。小样本上解释器启动占主导：
Python 慢 7.5–8.25×，峰值内存多约 6 倍；速度随真实数据规模的变化另见
`2026-09-14`。

**硬件存储口径**：WSL2 文件系统位于 **NVMe 固态硬盘**（三星 PM991a 512 GB
+ YMTC PC411 1 TB）；内存 32 GiB **LPDDR5X-8533**；GPU 为 Intel Arc 130T 集成
显卡（未使用）。

**环境**：WSL2（Arch Linux guest，内核 6.18.33.2-microsoft-standard-WSL2），
Windows 11 主机，Intel Core Ultra 5 225H（14 逻辑核），GPU 为 Intel Arc 130T
集成显卡（**未使用**，全部为 CPU 负载），guest 内存 7940 MB / 主机 32189 MB，
Rust 1.0.1 / Python 3.14.6 / R 4.6.1，warm cache，`/usr/bin/time -v` 高精度
计时。

**数据**：全部为仓库内置 fixture（见上），无外部数据、无受控数据。
**复现**：见 §5 命令行。
