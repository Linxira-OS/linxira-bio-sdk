# Benchmark tooling

Everything used to produce `benchmark-results/<date>/` lives here, grouped by
function. A measurement window is only reproducible when three things are
committed together: the **inputs** (`inputs/`), the **stage scripts actually
executed that day** (`windows/<date>/`), and the **archive** the run produced
(`benchmark-results/<date>/`).

Each script carries a header comment naming its role, the window(s) that used
it, and the direction of the data flow. This file is the index that ties a
window to the scripts it ran.

## Layout

| Path | Purpose | Shipped? |
| --- | --- | --- |
| `inputs/` | Deterministic input generators. Byte-identical output across machines/OSes, so `sha256` in an archive is meaningful. | no |
| `run/` | Capability runners: product-CLI orchestration and batched cross-check suites. | no |
| `analysis/` | Archive assembly, statistics tables, runtime-preference write-back. | no |
| `figures/` | Self-contained SVG rendering (no scripts, no external references in the output). | no |
| `windows/<YYYY-MM-DD>/` | The exact stage scripts of one window, kept verbatim as executed. | no |
| `rental/` | Provisioning material for rented cloud GPU instances. | no |

Nothing under `scripts/benchmark/` is staged into the release bundle; these are
repository-development assets. The agent documentation that *does* ship lives in
`docs/capabilities/` and `skills/`.

## Scripts

| Script | Role | Windows used |
| --- | --- | --- |
| `inputs/generate-inputs.py` | Synthetic tables (`random.seed(42)`), granularity FASTAs (numpy `default_rng(42)`), engine-simulated FASTA/FASTQ. | 2026-10-11 |
| `run/run-linux.sh` | One-command Linux matrix: dataset manifest → `benchmark run` → archive + runtime-preference write-back (ROADMAP M2-T6). | 2026-09-14, 2026-09-18, and every later Linux window |
| `run/tier23-bench.sh` | Batched `expression.quantify.v1` cross-check vs stored reference quant outputs, per-run wall/user/sys accounting. | 2026-09-14 |
| `run/quantify-bench.sh` | Single-sample quantify cross-check (identifier parity, NumReads within 1%, TPM Pearson ≥ 0.995). | 2026-09-14 |
| `run/tier1v2-biolab.sh` | tier1_v2 batch `bench-20260914-002` against the thesis pipeline's own salmon 2.7.0 outputs. Git-ignored: hardcodes that lab machine's mounts. | 2026-09-14 |
| `analysis/summarize-reports.py` | Builds `summary.json` + `summary.md` from per-capability reports and an environment descriptor; validates against the summary schema. | 2026-09-13, 2026-09-14, 2026-09-18, 2026-10-02, 2026-10-11 |
| `analysis/compare-conditions.py` | Repository-side stopgap for cross-condition/cross-window comparison; superseded by ROADMAP M2-T11 (`benchmark compare`). | 2026-10-11 |
| `analysis/statistics.py` | Standalone TSV statistics: N, median, IQR, CV, ratio, bootstrap 95% CI, two-part verdict, plus trimming-robustness and cross-window tables. | 2026-10-11 |
| `analysis/tier23-summarize.py` | Aggregates the tier23-bench CSV into a comparison table. | 2026-09-14 |
| `analysis/update-runtime-preferences.py` | Writes the measured backend rows of `runtime-preferences.json` from reports. | 2026-10-04, 2026-10-11 |
| `figures/render-svgs.py` | Renders the publication figure set — 12 per language: absolute medians, memory, ratio CI, repeat stability, dispersion (CV), layered speedup, the 1.10× ladder, memory ratio, two-window series, record granularity, condition comparison (needs `--contended`), and the protocol/decision diagram. | 2026-10-11 (compares against 2026-09-13; ladder/memory-ratio/dispersion/condition added 2026-10-12) |
| `rental/prepare-bundle.py` | Assembles the upload bundle for a rented GPU instance. | 2026-10-02 |

## Windows

| Window | Machine | What ran | Stage scripts | Archive |
| --- | --- | --- | --- | --- |
| 2026-09-13 | laptop | first archived three-backend pass (small fixtures, N=5) | `run/run-linux.sh` | `benchmark-results/2026-09-13/` |
| 2026-09-14 | laptop | tier1_v2 quantify cross-checks (batched) | `run/tier23-bench.sh`, `run/quantify-bench.sh`, `run/tier1v2-biolab.sh`, `analysis/tier23-summarize.py` | `benchmark-results/2026-09-14/` |
| 2026-09-18 | laptop | reproducibility pass written up as the first article | `run/run-linux.sh` | `benchmark-results/2026-09-18/` |
| 2026-10-02 | rented cloud GPU | dual-vendor GPU validation and docking study | `rental/*` | `benchmark-results/2026-10-02/` |
| 2026-10-04 | NAS (Pentium Gold G7400) | first real execution of the R backend, N=3, three capabilities' r-leg data written back | `windows/2026-10-04/*` | `docs/engine-evals/local-nas-g7400-window-2026-10-04.md` |
| 2026-10-11 | NAS (Pentium Gold G7400) | archived N=10 three-backend window: six fast capabilities, `fastq.qc`, `sequence.ssr`, plus the record-granularity experiment | `windows/2026-10-11/*` (`fast.sh`, `fastq.sh`, `ssr.sh`, `granularity.sh`, `descriptor.json`, `archive.sh`), `inputs/generate-inputs.py`, `analysis/statistics-tables.py`, `figures/render-svgs.py` | `benchmark-results/2026-10-11/` |

## Adding a window

1. Put the day's stage scripts in `windows/<YYYY-MM-DD>/` **as executed** (they
   are evidence, not tidy examples) and add a row to the tables above.
   A window that produced an archive also keeps its `descriptor.json` (the
   environment block and entry list the summarizer consumes) and a single
   `archive.sh` that reproduces the whole assembly — staging, input digests,
   summary, statistics, figures, figure check — in one command.
2. Generate inputs with `inputs/` rather than ad-hoc code, so the archive's
   digests can be reproduced.
3. Archive with `analysis/summarize-reports.py` (validating against
   `schemas/benchmark-summary.schema.json`), then statistics and figures.
4. Re-run `figures/verify-figures.py`: every number a figure shows must be
   re-derivable from the archive, and the script fails otherwise.
5. Commit inputs, stage scripts, descriptor, archive and figures in the same
   change, so the window can be re-checked without the machine it ran on.
