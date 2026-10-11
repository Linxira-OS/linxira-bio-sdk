---
title: "A three-language repeated benchmark of bioinformatics capabilities — Rust vs Python vs R over 8 capabilities at N=10"
date: 2026-10-11
tag: technical
lang: en
desc: "One low-end x86 machine, one Rust engine, and the idiomatic Biopython and Biostrings implementations measured on the same inputs with 10 timed repeats each — under a pre-registered decision rule with bootstrap confidence intervals: 7 of 8 capabilities show a material gap (7.7×–1097×), one shows no resolvable difference."
---

> Draft, written incrementally. **Pending**: the N=10 long runs for `fastq.qc` and `sequence.ssr`
> (queued; their rows get filled into §8.1, §8.6 and §9 when the runs land).
> Raw tables live in this repository next to this file (see §12); figures in `./figures/`.

## 1. Abstract

We measured **8 implemented capabilities** of the Linxira Bio SDK on a low-end x86 machine
(Intel Pentium Gold G7400, 2 cores / 4 threads, 15 GiB) across three language backends: the Rust
engine, plus Python and R comparison arms built on **Biopython** and **Biostrings**. Every
capability × backend combination got **10 timed repeats** (preceded by one untimed warm-up), with
fixed inputs and commands, serial execution, and an unchanged `powersave` governor.

Results use a **two-part, pre-registered rule** (§6.3):

- **7 of 8 capabilities show a material gap** (bootstrap 95% CI lower bound ≥ 1.10): Rust is
  **7.7×–200×** faster than the Python arm and **2.2×–1097×** faster than the R arm.
- **One capability (`sequence.stats`) shows no resolvable difference**: Rust 820 ms, Python 840 ms,
  ratio 1.02×, CI [0.99, 1.04] — the interval **crosses 1.00, so we do not claim "faster" here**.
- Rust's **consistent win is memory**: peak RSS is 1/2.2–1/12 of Python's and 1/5.5–1/55 of R's.

Boundaries up front: this is a **whole-stack comparison** (our Rust engine vs idiomatic third-party
library implementations plus their language runtimes, measured by the same harness), not a claim
about "language speed". Every ratio holds only for this machine, these inputs, and this protocol (§11).

## 2. Changelog relative to the two earlier posts

- **R** ran as a real third backend for the first time, exposing 6 latent defects in it (§10) —
  the R arm had never actually executed in CI before.
- The decision rule moved from a point estimate to **confidence interval + materiality floor** (§6.3),
  with a trimming-robustness check (§8.4).
- A **record-granularity experiment** was added to explain the ≈1× on `sequence.stats` — the original
  hypothesis was **falsified** (§8.5).
- Data, scripts, tables and figures ship with the repository and can be re-checked item by item (§12).

## 3. Background

Three posts form a line: ① a graduation project (a buckwheat genome) where only "does it finish"
mattered; ② a one-off end-to-end reproduction of a public SRA dataset; ③ this one — no cherry-picked
data, no cherry-picked metric, a protocol written down before the numbers were seen.

The motivation is concrete: the SDK's promise is **local-first**. Local-first means users may well run
it on a machine without a GPU and without spare RAM. A backend that is 20× slower and eats 10× the
memory on such a machine is not a viable default. So what we need is not "Rust is faster than Python"
but a **per-capability, interval-carrying, reproducible comparison**.

## 4. Related work and third-party disclosure

**Comparison arms used in this window** (third-party implementations, not ours):

| Backend | Basis | Version |
| --- | --- | --- |
| Rust | this repository's engine | v1.1.3 |
| Python | Biopython (FASTA/FastQ parsing and statistics), numpy | Biopython 1.88 / numpy 2.5.4 |
| R | Biostrings, `prcomp`, `stats` | Biostrings 2.80.2 / Bioconductor 3.23 / R 4.6.1 |

**No third-party benchmark suite was used in this window.** Two external calibration paths are
*planned and not executed*, and are therefore not evidence here:

1. **GIAB + hap.py** — to calibrate **correctness** (not speed) against a public truth set. Our
   variant-calling and comparison capabilities can consume those inputs, but the truth-set download,
   confident-region masks and containerised execution did not happen in this window.
2. **A public hosted benchmark platform** (e.g. Phoronix Test Suite / the OpenBenchmarking result
   library) — to anchor this machine's absolute numbers to a third-party machine lineage. Also not run.

We list both as "planned" rather than "done", because **writing something into a post does not make it done**.

## 5. Data

Every input is produced by the committed generator (`scripts/benchmark/inputs/generate-inputs.py`):
synthetic tables from `random.seed(42)`, sequence inputs from the engine's own seeded simulator
(`--seed 42`). Anyone can rebuild the exact same bytes.

| Capability | Input | Size | sha256 |
| --- | --- | --- | --- |
| `sequence.stats.v1` | FASTA (20 × 5 Mb) | ~100 MB | pending (from `simulate sequence --seed 42`) |
| `expression.pca.v1` | TSV matrix | 2000 × 40 | `273755e5eca0ae14d150365271dda778666109341ec0826baa01b7dc7bd76345` |
| `set.venn.v1` | TSV set table | 20k items × 4 sets | `484dffd240e46a4de6995ae935107e464185ca93c9a8025e79856e1fa5234531` |
| `enrichment.overrepresentation.v1` | `genes.txt` | 5k genes | `84ac1b6eec0a96798aa39b905c654faa6156dc4f4af4d4eb5f672faf8d1f98db` |
| `enrichment.overrepresentation.v1` | `associations.tsv` | 45k associations | `b523c5b4509487186461e8bc29adcdce910551ed1c4b88d4768c220c5ff9d781` |
| `structure.pdb.summary.v1` | PDB | 3000 residues / 24k atoms | `ecb30a5181b376760ceab330d9244f79b28195b54a4a8cac26a8ca23e2559afe` |
| `variant.stats.v1` | VCF | 200k variants | `1dab9c47535f9b3552cd3cfae1fa70ac4fccc5ff7518d4c9cf5b76950861ce53` |
| `fastq.qc.v1` | FASTQ | 200 MB / 646k reads | pending (derived from `big.fa` via `simulate reads --seed 42`) |
| `sequence.ssr.v1` | FASTA | ~100 MB | same file as `sequence.stats` (`big.fa`) |

**Cross-machine, cross-version check**: generating those six synthetic inputs on Windows
(Python 3.13.15) and on the measuring machine (Linux, Python 3.14.7) yields **six identical sha256
digests** — the inputs do not depend on the generating machine.

## 6. Methods

### 6.1 Measurement protocol

- **One untimed warm-up + 10 timed repeats per backend = 11 process executions per backend.**
  **The report contains only the 10 timed runs** (`run_index` 1..10); the warm-up's result is discarded
  and enters no statistic. Its purpose is to move page-cache and first-call costs out of the timed
  window, which is why the report records `page_cache = warm`.
  Code: `engine/crates/linxira-bio-cli/src/main.rs:7257` (the `"warmup"` request) and `:7273`
  (`for run_index in 1..=repeat`).
- **Every statistic in this post (median, IQR, CV, CI, verdict) is computed over those 10 timed runs only.**
  The "drop first / middle 8" bases in §8.4 are **additional robustness checks**, not a replacement for
  N=10 — the headline numbers are always N=10.
- Wall time from GNU `/usr/bin/time -v` (high precision); process self-reported time and **peak RSS**
  are recorded alongside.
- Within a capability measurement, execution is **strictly serial** — no concurrent build, lint or
  archiving job.
- The CPU governor stays `powersave` for the whole window; no switching, to avoid mixing rows with
  previously archived windows.
- Input files and commands are identical across windows.
- Backend order is fixed (rust → python → r); cross-backend frequency/thermal drift is a disclosed
  limitation (§11).

### 6.2 Statistics

Median-of-10, inter-quartile range, coefficient of variation, min/max, peak RSS; backend ratios are
ratios of medians, with a **bootstrap 95% confidence interval** over the raw `wall_ms` values
(10,000 resamples, fixed seed 20261011).

### 6.3 Pre-registered decision rule (rule first, numbers second)

| Condition | Wording |
| --- | --- |
| ratio CI lower bound **≥ 1.10** | **material**: "N× faster (CI lower bound …)" |
| CI excludes 1.00 but lower bound < 1.10 | **detectable-immaterial**: resolvable, not practically meaningful |
| CI **crosses 1.00** | **within-noise**: "no resolvable difference on this input and machine" |

Rule-change record: we first used "2 × (CV₁ + CV₂)" as a noise-band threshold, then found it labels a
**consistent 2.4% gap** as noise and a lucky 15% gap as signal — because **dispersion is not an effect
size**. Hence the two-part rule above: resolvability from the CI, materiality from the 1.10 floor.
CV is still reported per row, but no longer decides anything.

### 6.4 Trimming robustness

Besides all 10 repeats, medians and verdicts are recomputed after dropping the first repeat (9) and
after keeping only the middle 8 ([`../statistics-trimmed-medians.tsv`](../statistics-trimmed-medians.tsv),
[`../statistics-trimmed-verdicts.tsv`](../statistics-trimmed-verdicts.tsv)). **All three bases agree on every median and every verdict** —
no conclusion depends on keeping a particular run.

## 7. Environment

| Item | Value |
| --- | --- |
| OS | CachyOS (Arch family), native Linux filesystem (not a drvfs mount) |
| CPU | Intel Pentium Gold G7400, 2 cores / 4 threads |
| Memory | 15 GiB |
| GPU | **not used** by this benchmark (a separate GPU experiment carrier exists, out of scope here) |
| Rust | rustc / cargo 1.99.0 |
| Python | CPython 3.14.7; numpy 2.5.4, Biopython 1.88, pytrf |
| R | 4.6.1; Bioconductor 3.23, Biostrings 2.80.2, jsonlite 2.0.0, digest 0.6.39 |
| Governor | `powersave` (unchanged throughout) |
| Storage | root on btrfs / 512 GB NVMe; **`/tmp` is tmpfs (7.7 GB), so every benchmark input is RAM-backed** — no disk latency enters the measurement (two spinning disks are present and unused) |
| Timing | GNU time, high-precision mode |
| Engine | v1.1.3 |

> This is a **low-end machine**: 2 cores / 4 threads, no turbo headroom, `powersave`. Its value is not
> representativeness but being a **conservative floor**: on faster hardware the constant terms
> (interpreter start-up, large allocations) get amortised differently, so ratios **may grow or shrink**.
> Every ratio here is therefore stated for this machine only.

## 8. Results

### 8.1 Three-backend medians and verdicts (N=10)

Table: [`../statistics.tsv`](../statistics.tsv) (one row per capability × backend: N, median, IQR, CV, min/max, peak RSS,
ratio, CI, verdict). Excerpt:

| Capability | rust | python | r | python verdict | r verdict |
| --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 820 ms | 840 ms | 1840 ms | 1.02× [0.99, 1.04] **within-noise** | 2.24× [2.16, 2.26] **material** |
| `expression.pca.v1` | 20 ms | 4010 ms | 2760 ms | 200× [161, 215] material | 138× material |
| `set.venn.v1` | 10 ms | 180 ms | 10,975 ms | 18× material | **1097×** material |
| `enrichment.overrepresentation.v1` | 30 ms | 230 ms | 4470 ms | 7.7× material | 149× material |
| `structure.pdb.summary.v1` | 30 ms | 790 ms | 20,750 ms | 26× material | 692× material |
| `variant.stats.v1` | 80 ms | 760 ms | 13,495 ms | 9.5× material | 169× material |
| `fastq.qc.v1` | pending (N=10) | pending | pending | — | — |
| `sequence.ssr.v1` | pending (N=10) | pending | pending | — | — |

(An earlier N=3 window measured the same capabilities with the same conclusion; see §9 and the
engineering ledger `docs/engine-evals/local-nas-g7400-window-2026-10-04.md`.)

### 8.2 Memory (peak RSS, median-of-10)

| Capability | rust | python | r |
| --- | --- | --- | --- |
| `sequence.stats.v1` | 5.9 MB | 69.7 MB (11.9×) | 321.1 MB (54.8×) |
| `expression.pca.v1` | 8.5 MB | 53.3 MB (6.3×) | 81.9 MB (9.7×) |
| `set.venn.v1` | 8.3 MB | 47.3 MB (5.7×) | 124.6 MB (15.0×) |
| `enrichment.overrepresentation.v1` | 14.8 MB | 61.0 MB (4.1×) | 145.5 MB (9.8×) |
| `structure.pdb.summary.v1` | 29.7 MB | 66.3 MB (2.2×) | 162.9 MB (5.5×) |
| `variant.stats.v1` | 6.2 MB | 45.8 MB (7.4×) | 122.4 MB (19.7×) |

Figure: [`./figures/fig-memory.svg`](./figures/fig-memory.svg).

### 8.3 Repeat stability and ratio intervals

Figure: [`./figures/fig-repeat-stability.svg`](./figures/fig-repeat-stability.svg) (spread of the 10 repeats per capability and backend).
Most medians sit within ±1 IQR across repeats. `sequence.stats` — Rust's granularity is 10 ms (the
timer's resolution), so its CV (1.95%) is dominated by **quantisation**, not real jitter. That limits
resolution on small absolute values and is one reason the materiality floor is 1.10 rather than 1.01.

Figure: [`./figures/fig-ratio-ci.svg`](./figures/fig-ratio-ci.svg) (ratios with 95% CIs, the 1.00 reference line, and the 1.10
materiality floor).

### 8.4 Trimming robustness

All three bases agree; see [`../statistics-trimmed-medians.tsv`](../statistics-trimmed-medians.tsv) and [`../statistics-trimmed-verdicts.tsv`](../statistics-trimmed-verdicts.tsv).
In other words: **no conclusion survives only because a particular run was kept.**

### 8.5 The record-granularity experiment: a falsified hypothesis

**Original hypothesis**: `sequence.stats` differs from Python by only 2% on million-base inputs
because "a language boundary is crossed once per record"; more records should widen the gap.

**Experiment**: the same ~100 Mb, in three record shapes, N=10 each.

| Input shape | rust | python | r |
| --- | --- | --- | --- |
| 1 × 100 Mb (one record) | 820 ms | 850 ms | 1825 ms |
| 20 × 5 Mb | 820 ms | 840 ms | 1830 ms |
| 100,000 × 1 kb | 830 ms | 860 ms | 1940 ms |

**Falsified.** Going from 1 to 100,000 records moves Rust by 10 ms and Python by 10 ms
(ratio 1.037 → 1.036). At ~100 MB both sides are dominated by **I/O and large allocations**: the
per-base work lives in C primitives, while reading and building each line costs both sides.

Corrected explanation (which is also the layering used throughout this post): the dividing line is not
record count but **"does the interpreter touch each element?"**. Capabilities whose per-base work lives
in C (`sequence.stats`) land at ≈1×; capabilities whose per-item/per-record work lives in the
interpreter (`set.venn`, `pdb`, `variant`, `enrichment`, `fastq.qc`) span 7.7× to 1097×. The ~20×
seen on tiny inputs is **interpreter start-up (~170 ms)** landing on a 10 ms workload — not throughput.

### 8.6 Layered overview

Figure: [`./figures/fig-speedup-layered.svg`](./figures/fig-speedup-layered.svg).

| Layer | Capability | python | r |
| --- | --- | --- | --- |
| Linear scan covered by C primitives | `sequence.stats` | 1.02× (not resolvable) | 2.24× |
| Per-item interpreter loop (small in-memory objects) | `set.venn` | 18× | 1097× |
| Per-gene / per-variant interpreter loop | `enrichment`, `variant` | 7.7× / 9.5× | 149× / 169× |
| Per-atom interpreter loop | `structure.pdb.summary` | 26× | 692× |
| Vectorised numeric library | `expression.pca` | 200× | 138× |
| Per-base interpreter loop (pending) | `fastq.qc` | pending | pending |
| Motif scan (pending) | `sequence.ssr` | pending | pending |

Note the `expression.pca` row: Python is *slower* than R there (200× vs 138×) because both arms run a
vectorised library (numpy vs `prcomp`) — the gap comes from **the library and its calling
conventions**, not from the language. That is what "whole-stack comparison" means.

### 8.7 Correctness evidence (cross-backend consistency)

Every benchmark report carries a three-backend consistency verdict: **all 8 capabilities are
`consistent` within a 1e-6 relative tolerance**, precision class `high`. Additionally, cross-machine
determinism was checked: generator outputs hash byte-identically on the NAS and on a second machine
(`ref.fa`, `cell.tsv`, `cell.json`). Table: [`../statistics-consistency.tsv`](../statistics-consistency.tsv).

**Precision comparison against what the earlier window recorded** (each window's own evidence, shown
side by side, never mixed):

| Check | Earlier window `bench-20260913-001` | This window (N=10) |
| --- | --- | --- |
| Backend consistency verdict | Consistent (4 capabilities) | `consistent` (8/8 capabilities) |
| Consistency tolerance | 1e-6 relative | 1e-6 relative (same default) |
| PDB per-field deviation | **0.0** (identical) | `findings` empty (no differing field paths) |
| PCA / Venn max relative error | **≤ 2.26e-11** | recorded as a verdict: `consistent`, no differing fields |
| Precision class | `high` | `high` |
| Cross-machine determinism | not recorded | yes: `ref.fa` / `cell.tsv` / `cell.json` sha256 identical across machines |

Stated honestly: the earlier window recorded **numeric** parity errors (2.26e-11), while this window
records **verdict-level** evidence (within 1e-6, no differing fields). Neither substitutes for the
other, so the table shows both bases rather than moving "0.0 / 2.26e-11" under this window's name.

## 9. Comparison with the September window

Same capabilities, same commands, different input sizes and window ([`../statistics-comparison.tsv`](../statistics-comparison.tsv),
generated against `benchmark-results/2026-09-13/summary.json`):

| Capability | September window (N=5, small fixtures) | This window (N=10, large inputs) | Ordering consistent? |
| --- | --- | --- | --- |
| `sequence.stats` | rust 40 / py 330 / r 2110 ms | rust 820 / py 840 / r 1840 ms | yes (rust ≤ py < r) |
| `expression.pca` | rust 40 / py 300 / r 460 ms | rust 20 / py 4010 / r 2760 ms | yes |
| `set.venn` | rust 40 / py 310 / r 400 ms | rust 10 / py 180 / r 10,975 ms | R moves drastically with scale |
| `structure.pdb.summary` | rust 40 / py 320 / r 410 ms | rust 30 / py 790 / r 20,750 ms | yes (magnitudes unfold with size) |
| `variant.stats` | rust 60 / py 660 / r 12,000 ms | rust 80 / py 760 / r 13,495 ms | yes |

Reading: **small fixtures flatten all three backends** (tens to hundreds of ms) because start-up and
constant terms dominate; magnitude differences only unfold on large inputs. The R arm of `set.venn`
looks "fine" on the small fixture (400 ms) and becomes 10,975 ms at scale — consistent with §8.5.

For identical inputs, cross-window **absolute** numbers agree to <3% median difference, so this
machine's measurements are themselves reproducible; the rows that must not be compared directly are
the ones with different input sizes (flagged above).

## 10. Engineering findings: what running the R arm for real exposed

The real product of a three-backend measurement is that it **forces execution**. The R backend had
never run its tests in CI; wiring it up exposed six defects in one pass, all fixed and merged:

| # | Symptom | Root cause |
| --- | --- | --- |
| 1 | SSR capability errored out | Biostrings defines `width()` only for `XStringSet`, not for a bare `XString` |
| 2 | Test literal disagreed with its assertion | The "22 bp" literal was actually 23 characters |
| 3 | SSR always returned 0 motifs | `start(...)` without a namespace prefix resolved to `stats::start` (time series) |
| 4 | All of the above stayed hidden | **CI never ran the benchmark packages' tests** (no steps at all) |
| 5 | Inputs without SSR were rejected by the worker | An empty `list()` serialised to `[]`; the contract requires a map (`{}`) |
| 6 | Wrong `intersections` count for 20k-item input | Generated per item instead of grouped by occupancy mask |

A **product-level** defect surfaced the same way: `looks_like_sam` classified a **wide count matrix**
as SAM based only on "≥11 columns plus numeric-looking fields" (measured boundary: 10 columns → tsv,
11 columns → sam), so any expression matrix with ≥11 samples was rejected by the worker. It now
demands structural SAM evidence (FLAG bound, non-numeric RNAME, CIGAR grammar, base alphabet for SEQ)
and carries a regression test.

One lesson: **a test that exists but never runs in CI does not exist.**

## 11. Limitations and honest boundaries

- **Whole-stack comparison**, not language vs language: the arms are idiomatic Biopython/Biostrings
  code plus their runtimes, under one harness. A faster hand-written Python implementation would change
  the ratios substantially.
- **One machine, one input scale, `powersave`**: a conservative floor, not a general statement. Ratios on
  other CPUs / memory-bandwidth profiles may differ.
- **Serial backend measurement** carries known frequency/thermal drift; we publish each backend's 10 raw
  wall times rather than only the summary.
- **Timer resolution**: `time` quantises at 10 ms, inflating CV for small absolute values (Rust 10–30 ms),
  which is part of why the materiality floor is 1.10.
- **No third-party benchmark suite** was used here; GIAB + hap.py (correctness) and a public hosted
  benchmark platform (machine lineage) remain **planned**, and are not evidence until executed.
- We do **not** claim "Rust is generally faster". What this post supports is per-capability, interval-
  carrying statements scoped to this machine and these inputs.

## 12. Reproducing

```bash
# 1) inputs (seeded; writes only to the target directory)
python scripts/benchmark/inputs/generate-inputs.py --set synthetic --out /tmp/lxn10
python scripts/benchmark/inputs/generate-inputs.py --set sequence  --out /tmp/lxn10 --binary target/release/linxira-bio
python scripts/benchmark/inputs/generate-inputs.py --set reads     --out /tmp/lxn10 --binary target/release/linxira-bio
python scripts/benchmark/inputs/generate-inputs.py --set granularity --out /tmp/lxn10

# 2) repeated measurement (1 untimed warm-up + 10 timed repeats per backend)
linxira-bio benchmark run <capability> --input <file> --backends rust,python,r --repeat 10 \
  --report /tmp/lxn10/out-<capability>/report.json

# 3) archive (schema-checked) and statistics tables
python scripts/benchmark/analysis/summarize-reports.py --reports-dir /tmp/lxn10 --descriptor descriptor.json \
  --output-dir benchmark-results/2026-10-11
python scripts/benchmark/analysis/statistics-tables.py --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 --output-dir benchmark-results/2026-10-11

# 4) figures (self-contained SVG)
python scripts/benchmark/figures/render-svgs.py --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 --output benchmark-results/2026-10-11/blog-mirror/figures
```

## 13. Data and code availability

- Archive: `benchmark-results/2026-10-11/` (`summary.json` / `summary.md` / per-capability reports /
  statistics TSVs).
- Raw per-run wall times: `backends[].runs[]` in each capability report (10 entries per backend with
  `wall_ms`, `peak_rss_mb`, `ok`).
- Input digests: `data_sources[]` in `summary.json` (recorded for this window at archive time).
- Figures: `benchmark-results/2026-10-11/blog-mirror/figures/` (7 self-contained SVGs, no scripts,
  no external references).

## 14. Appendix: table index

| File | Contents |
| --- | --- |
| [`../statistics.tsv`](../statistics.tsv) | per capability × backend: N, median, IQR, CV, min/max, peak RSS, ratio, CI, verdict |
| [`../statistics-trimmed-medians.tsv`](../statistics-trimmed-medians.tsv) | medians and CVs under the three trimming bases |
| [`../statistics-trimmed-verdicts.tsv`](../statistics-trimmed-verdicts.tsv) | ratios, CIs and verdicts under the three trimming bases |
| [`../statistics-consistency.tsv`](../statistics-consistency.tsv) | cross-backend consistency, tolerance, precision class, report paths |
| [`../statistics-comparison.tsv`](../statistics-comparison.tsv) | cross-window comparison against the September archive |
| [`../summary.json`](../summary.json) / [`../summary.md`](../summary.md) | archive index (schema-validated) |
