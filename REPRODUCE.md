# Reproducing the published benchmark

This file is the shortest path from "I read your benchmark" to "I checked it myself".
It covers the archived window **`bench-20261011-001`** (the 2026-10-11 three-language
measurement: 8 capabilities × 3 backends, N=10 timed repeats each).

If something here does not reproduce, that is the interesting case — please open an issue at
<https://github.com/Linxira-OS/linxira-bio-sdk/issues> with the command you ran, your
environment, and the expected-vs-actual numbers. We treat a failed reproduction as a defect in
this repository, not as user error.

## 0. What you need

| Requirement | Notes |
| --- | --- |
| git, Python ≥ 3.11, Rust ≥ 1.92 (`edition 2024`) | Rust only for the `--full` path and the CLI smoke |
| numpy | for the granularity inputs (they are skipped without it) |
| ~2 GB free disk and ~2–5 min for the smoke path | the granularity set writes ~100 MB and dominates the runtime (289 s measured on the 2C/4T machine); the full window needs ~10 GB |
| R 4.6.x + Biostrings (full window only) | the R arm; CRAN/Bioconductor |

## 1. Five-minute check (measured: 289 s, inputs only)

```bash
git clone https://github.com/Linxira-OS/linxira-bio-sdk && cd linxira-bio-sdk
git checkout 89882a5          # the revision the archive was produced with (see step 2)
bash scripts/benchmark/reproduce.sh --smoke
```

It performs three checks and exits non-zero on any failure:

1. **Revision identity** — prints the `code_revision` recorded inside the archived reports and
   compares it with your `HEAD`.
2. **Input reproducibility** — regenerates the archived inputs from the fixed seed
   (`scripts/benchmark/inputs/generate-inputs.py`, seed 42) and compares `sha256` with
   `benchmark-results/2026-10-11/inputs.sha256`. Currently verified byte-identical:
   `matrix.tsv`, `sets.tsv`, `structure.pdb`, `variants.vcf`, `genes.txt`, `associations.tsv`,
   `g_one.fa`, `g_twenty.fa`, `g_many.fa` (9/9). `big.fa` and `reads200.fq` are produced by the
   engine's own simulator and belong to the `--full` path.
3. **Environment honesty** — compares the installed Python packages against
   `workflows/org.linxira.benchmark-python/requirements.lock`. A mismatch means a reproduction
   claim is invalid until it is reconciled; see **Known gaps** below.

Optional CLI smoke (compiles the workspace once):

```bash
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
```

## 2. The full window (hours, on a quiet machine)

```bash
bash scripts/benchmark/reproduce.sh --full     # prints the exact commands and expected medians
```

The long run is deliberately **not** wrapped in a convenience command: an N=10 three-backend
window is a scheduled measurement. The commands are the ones in
`scripts/benchmark/windows/2026-10-11/` (`fast.sh`, `fastq.sh`, `ssr.sh`, `granularity.sh`) and
the accounting is `scripts/benchmark/analysis/statistics-tables.py`. Expected headlines and every
per-row statistic are in [`statistics.tsv`](benchmark-results/2026-10-11/statistics.tsv); the
figures are re-derived from the same numbers by `scripts/benchmark/figures/verify-figures.py`
(1055/1055 checks in the Chinese set, 1067/1067 in the English set).

Measurement conditions that must match for the comparison to mean anything (all disclosed in the
articles and in `summary.json`): same inputs (digests above), same machine class, `governor`
pinned (`powersave` in this window — a conservative floor, disclosed as such), warm page cache,
one untimed warm-up plus 10 timed repeats, medians (not means) with bootstrap CIs.

### Peak memory needs GNU `time`

`peak_rss_mb` and the disk-throughput columns come from `/usr/bin/time -v`. Without the
`time` package installed, the harness automatically falls back to in-process timing: wall
time is still recorded, `peak_rss_mb` is `null`, and every run says so in
`timed_with_time_v: false`. Every memory number we publish comes from a machine with
`time` installed. If you compare our RSS numbers with your own, install `time` first —
otherwise you are comparing a measurement against the absence of one.

### Timing resolution (read this before judging a small fixture run)

The harness measures wall time with a 10 ms clock, so on a tiny fixture the native arm can be
recorded as `wall_ms: 0.0` while the interpreter arms show ~160 ms (Python) and ~1260 ms (R) —
the interpreter arms start a process, the native arm finishes inside one tick. That is why every
value in `statistics.tsv` is a multiple of 10 ms, and it is why the articles say a `sequence.stats`
CV of 1.95% is quantisation rather than jitter. A `0.0` here means "faster than one tick", not
"not measured" (a missing measurement is `null`, never `0.0`).

## 3. Known gaps (stated here rather than discovered by you)

| Gap | Status |
| --- | --- |
| The archived environment reports `biopython 1.88` / `numpy 2.5.4` while the lock pins 1.85 / 2.2.4 | open — the lock states the intended environment; the next window installs from it, so lock == environment |
| **The lock in the archived revision could not be installed at all** | fixed on `main` (2026-10-12): the `pytrf` block held an escaped line-break sequence instead of real line breaks, and the pin set omitted `pyfastx`, which `pytrf` declares, so `uv pip install --require-hashes -r …` stopped at `pyfastx` — the CI step that installs this lock never reached the tests behind it |
| The lock is now generated, not hand-edited: `scripts/generate-python-lock.py` resolves `requirements.in` with `uv pip compile --generate-hashes` for x86_64-manylinux and x86_64-windows and merges the hash sets | verified by a real `--require-hashes` install on Windows (`pytrf 1.5.0`, `pyfastx 2.3.1`, `biopython 1.85`, `numpy 2.2.4` import) and by a platform-targeted dry-run for manylinux |
| CI does not yet run `benchmark.run.v1` end-to-end | fixed 2026-10-12: the debian job runs a three-backend smoke on the tiny fixture and asserts consistency + recorded wall_ms/peak_rss_mb |
| No installer and no published package | open — `cargo install --git …` is the current path; installers are planned |
| No third-party benchmark suite was run (PTS/OpenBenchmarking/SPEC/GIAB+hap.py) | open — anchors are scheduled; every number published so far comes from our own harness |
| The arms are same-algorithm parity implementations, not each language's idiomatic tooling | by design; tool arms (fastp/seqkit/pyfastx/scikit-learn) are the next pass |
| CI does not yet run `benchmark.run.v1` end-to-end | open — only the harness unit tests run today |

## 4. Before or while you report

- `docs/outreach/reviewer-brief.md` — the shortest path from a clone to a verdict, with the
  exact commands and the values to expect.
- `docs/outreach/corrections-log.md` — every error we have found in our own published
  claims, including a retracted vendor statement, with the commit that fixed each one.
- [`.github/ISSUE_TEMPLATE/reproduction-failure.yml`](.github/ISSUE_TEMPLATE/reproduction-failure.yml)
  — the issue form: it asks for the revision, the command, both outputs, and your
  environment, because those four things decide whether the difference is ours or the
  machine's.

## 5. What this file is not

It is not a claim that our numbers generalise beyond this machine, these inputs and this protocol,
and it is not a substitute for the articles' limitations sections. If you find that a claim in the
article does not match the archive, that is the highest-value issue you can open.
