# 2026-10-13 window — `bench-20261013-001`

The locked-environment re-run of the 2026-10-11 window: same host, same inputs,
eight capabilities × three backends × N=10, this time with the python arm inside an
interpreter installed from `requirements.lock`. The measurement finished on
2026-10-13; the tool arms, the interpreter-isolation experiment and the archive ran
after the host rebooted on 2026-10-14 and wiped `/tmp`.

Read `summary.md` for the numbers, `statistics.md` for the medians and verdicts,
`statistics-comparison.tsv` for this window against 2026-10-11, and
`blog-mirror/` for the article drafts and the figure set. This file records what
those artifacts are, what they can be used for, and what was lost or is still open.

## 1. What ran

| Stage | Artifact | Notes |
| --- | --- | --- |
| Eight capabilities, three backends, N=10 | `*.benchmark.json` / `.md` + `summary.json` / `.md` | `sequence.stats.v1`, `expression.pca.v1`, `set.venn.v1`, `enrichment.overrepresentation.v1`, `structure.pdb.summary.v1`, `variant.stats.v1`, `fastq.qc.v1`, `sequence.ssr.v1` |
| Record-granularity experiment | `sequence.stats.v1.granularity-{one,twenty,many}.benchmark.json` | Three shapes of the same capability, archived beside the summary because three entries for one capability key would collide inside it |
| Ecosystem tool arms | `tool-arms/`, `tool-arms.tsv` | The second arm class, four arms; see §6 |
| Python-leg interpreter isolation | `env-isolation/` | Three cells, python leg only; see §7 |
| Figures + figure check | `blog-mirror/figures/{zh,en}/` | 11 per language; see §8 |
| Input digests | `inputs.sha256` | See §4 |

## 2. Code and environment self-label

`code-label.txt` is written by the window driver, not by hand: its first four lines
come from the same git values the binary embeds at build time (`build.rs` bakes in
revision, branch and `describe`), and the rest record the lock digest and the
interpreter versions the window actually ran.

- branch `main`, commit `f3b9f17a13ee79b5875f8ff1fc17b85050f2200b`, `describe` `f3b9f17`, tree clean.
  There is no release tag in this project yet, so `describe` carries the short commit hash
  instead of a tag.
- lock digest `sha256 9b396669f36620825f657b9859dc8b82baa6fd8c9fbf6561f2ef95573da484bb`.
- python `3.12.15` from the uv-installed interpreter at `~/lx-locked-venv`, with
  `biopython 1.85`, `numpy 2.2.4` (the locked versions; the previous window ran
  1.88 / 2.5.4 against a lock that pinned 1.85 / 2.2.4, which is the contradiction
  this window exists to remove — and which CI now gates).
- R `4.6.1` with `Biostrings 2.80.2`.

Host: Intel Pentium Gold G7400 (2 cores / 4 threads), 15756 MB, CachyOS, kernel
7.2.9, cpufreq governor `powersave`, page cache warm, serial execution with no
concurrent build or unrelated job.

## 3. Timeline, and the log that no longer exists

The window's log opens with the preflight at 2026-10-13T05:05:59Z; the host is
declared free at T05:51:03Z and the build starts at T05:52:19Z, which is what "the
window started" means here. `fast` and `granularity` finished
T06:07:49Z; `fastq` T08:25:43Z; `ssr` started T08:25:43Z and its last report was
written T15:09:58Z (its ten R runs alone are about 6.1 hours; the median is
2200.8 s, see `statistics.tsv`).

The window's own log (`window-log.txt`) ends at `stage ssr: start`, and there is no
`RUN-WINDOW-DONE` line: the handover guard stopped the window when the SSR reports
were on disk and took the machine for the maintainer's night jobs. The guard's own
log lived in `/tmp` and was lost when the host rebooted at 2026-10-14 07:41 CST, so
the two facts it would have carried are evidenced elsewhere:

- **The SSR stage really finished** — proven by `sequence.ssr.v1.benchmark.json`
  itself (`sampled_at 2026-10-13T15:09:58Z`, ten R runs, three backends), which is a
  stronger artifact than a log line.
- **When the machine was handed over** — the staged copy carries mtime
  `2026-10-13 23:10:23 +0800`, i.e. the moment the reports were copied out of
  tmpfs, which is the same moment the guard took the machine.

Nothing in this archive claims a completion line it does not have.

## 4. Inputs

`inputs.sha256` lists eleven paths: `big.fa` first, then the tables, structure, VCF
and the three granularity FASTAs, then the 200 MB FASTQ. They were regenerated on
2026-10-14 by the committed generators with the same seeds, because the originals died
with tmpfs — and they are the same bytes: **11 of 11 digests are identical to
`benchmark-results/2026-10-11/inputs.sha256`**, so this window's tools and arms read
the same inputs the earlier window did.

## 5. The headline result

`statistics-comparison.tsv` compares this window with `bench-20261011-001`:

- The Rust leg reproduces: `median_wall_ms[rust]` for `sequence.stats.v1` is 810 ms
  both times (ratio 1.0000), and no Rust leg moves more than 0.64%.
- The R leg reproduces: no leg moves more than 1.23%.
- The **python leg does not reproduce**: `sequence.stats.v1` goes 825 ms → 1845 ms
  (ratio **2.2364**), while `expression.pca.v1` moves the other way. §7 explains that
  with a controlled experiment rather than a guess: package versions are irrelevant,
  the interpreter generation is the variable.

## 6. Tool arms (the second arm class)

Four arms ran, N=10 each, `arm_class=tool`, `speedup` and `memory_saving` null by
construction (a tool arm never stores a ratio against our engine). The table holds
tool-side facts only; comparisons against our own arms follow as prose, because a
cross-class ratio in a table column is exactly what the two-class rule forbids.

| Tool | Version | Capability | Median (N=10) | Peak RSS |
| --- | --- | --- | ---: | ---: |
| `pyfastx stat` | 2.3.1 | `sequence.stats.v1` | 10.0 ms | 15.2 MB |
| `seqkit stats` | v2.14.0 | `sequence.stats.v1` | 30.0 ms | 26.9 MB |
| R `prcomp` | 4.6.1 | `expression.pca.v1` | 120.0 ms | 79.0 MB |
| scikit-learn `PCA` | 1.9.1 | `expression.pca.v1` | 700.0 ms | 139.3 MB |

Against our own arms on the same inputs: `pyfastx stat` and `seqkit stats` are 81x and
27x the Rust arm's 810 ms; R `prcomp` is 23x the hand-written R arm's 2745 ms;
scikit-learn `PCA` is 4.6x the hand-written Python arm's 3250 ms. All four arms ran on
this window's inputs (each report carries path, bytes and sha256 — `big.fa` hashes to
the same `9f8a4cb4…044e78` as `inputs.sha256`), on the same host, in the sessions
adjacent to the window's own measurements.

The fourth arm is the one that shows how an arm can go missing without anything
breaking: `tool-arms.sh` puts the locked venv first on `PATH`, so the registry's
`python3 -c "import sklearn"` version probe resolves to an interpreter that does not
carry scikit-learn, and the runner skipped that arm silently. The package was never
absent from the host — the 2026-10-12 archive measured scikit-learn 1.9.1 there — it
lives in the 3.14 environment, so the arm was run on 2026-10-14 with that interpreter
first on `PATH` (recorded in the report's environment block). Three arms out of a
four-arm registry is a silent failure mode; a gate for it is in `docs/BACKLOG.md`.

Four caveats, all of them stated because the numbers invite more than they support:

1. **These are not the same job.** Our `sequence.stats.v1` also reports `au_n`,
   `gc_percent` and `n_count`; `pyfastx stat` reads a FASTA it has already indexed
   (it writes a `.fxi`). A routing judgement — "this job should call the existing
   implementation" — needs a comparison on the *same output set*, and that
   measurement has not been made. These rows are recorded numbers, not verdicts.
2. **The timer is quantized.** GNU `time -v` reports hundredths of a second, so all
   ten runs of the fast arms are identical to the 10 ms digit (IQR 0.000 ms,
   CV 0.00%). The parity arms use the engine's own millisecond timer, which is why
   one class is quoted as prose and the two are never rendered in one table or figure.
3. **Our own side of these ratios was defective.** The Rust arm's 810 ms in this archive was
   dominated by a per-base branch that mispredicts on random sequence; found by decomposing the
   number and fixed on 2026-10-14 (CLI path 0.66 s -> 0.14 s, pipeline 810 -> 280 ms, probe
   readings). This archive keeps the pre-fix readings as the record of the code that produced
   them; the post-fix window is pending. See the 2026-10-14 ledger in `docs/engine-evals/`.
4. **One tool measured differently in the previous round.** The 2026-10-12 round
   recorded `pyfastx stat` at 30.0 ms on the same nominal input
   (`benchmark-results/2026-10-12/README.md`); this window records 10.0 ms. Both
   archives are correct about their own session; **the difference is not yet
   explained**, and the earlier article's "27x" and this window's "81x" are two
   numbers for two sessions, not a correction of one by the other.

## 7. Interpreter isolation — why the python leg moved

`env-isolation/` holds three cells, each ten timed repetitions, python leg only,
never merged into the window summary:

| Cell | Interpreter | Packages | `sequence.stats.v1` | `expression.pca.v1` |
| --- | --- | --- | --- | --- |
| A | CPython 3.12.15 | the four locked pins | 1840 ms | 3250 ms |
| B | CPython 3.12.15 | latest available | 1840 ms | 3235 ms |
| C | CPython 3.14.7 | latest available | **820 ms** | **3985 ms** |

Reading it: **A and B agree, so package versions are not the variable; the
interpreter generation is.** Cell C's 820 ms sits where the 2026-10-11 window's
825 ms was, so the 2.2364× in §5 is an interpreter-generation difference, not drift,
not the lock, and not a change in our code. It is also not monotone — the newer
interpreter is 2.2× faster on `sequence.stats.v1` and 23% slower on
`expression.pca.v1` — which is exactly why the article states the split rather than
a direction. A fourth cell (3.14 with the locked pins) does not exist: those pins do
not install on 3.14, and that absence is itself the fact.

## 8. Figures

11 per language under `blog-mirror/figures/`, rendered from `summary.json` and the
reports on the measuring host, then re-checked by
`scripts/benchmark/figures/verify-figures.py`, whose own output is stored beside the
figures as `figure-check.txt`: **986/986 (zh) and 997/997 (en) figure values
re-derive from the tables.** Both languages come from the same numbers.

The condition figure is intentionally absent: it plots a deliberately contended
batch, and that experiment belongs to the 2026-10-11 window (`contended-window/`
there). This window's condition story is the interpreter isolation, which is
recorded as tables in `env-isolation/` instead of being dressed up as a plot.

Two corrections were applied while assembling this archive, recorded here because
the archive is evidence:

- The first assembly passed `--comparison` this window's *own* summary, which made
  every ratio 1.0000 and plotted one window twice. It now compares against
  `bench-20261011-001`, which is what the tables above show.
- That first assembly also rendered the contended-batch figure into
  `blog-mirror/figures/`, and the renderer does not delete files it no longer
  writes; the stale `fig-condition.svg` was removed from both languages before the
  figure check above was run.

## 9. How to re-check this archive

```bash
# figures: every value in every figure must re-derive from the tables
python3 scripts/benchmark/figures/verify-figures.py \
  --figures benchmark-results/2026-10-13/blog-mirror/figures/zh \
  --summary benchmark-results/2026-10-13/summary.json \
  --reports benchmark-results/2026-10-13 \
  --comparison benchmark-results/2026-10-11/summary.json \
  --granularity benchmark-results/2026-10-13

# schemas and the archive as schema instances
python3 scripts/validate-repository.py

# the window's stage scripts, exactly as executed
scripts/benchmark/windows/2026-10-13/{run-window,fast,granularity,fastq,ssr,tool-arms,env-isolation,archive}.sh
scripts/benchmark/windows/2026-10-13/resume-after-reboot.sh --post-only
```

## 10. Known limits

- The tool-arm timer is quantized to 10 ms (§6.2), so those medians carry at most
  three digits of information.
- The interpreter-isolation cells are a separate experiment and are not entries in
  `summary.json`.
- A missing tool arm fails quietly: on this window's first pass the registry's
  version probe resolved to an interpreter without scikit-learn, and the runner
  produced three arms instead of four with no error (§6). It is recorded as a gate to
  add, not as a package to install.
- `pyfastx stat` reading 30.0 ms in the 2026-10-12 round and 10.0 ms here is
  unexplained (§6.3).
- The handover guard's log is gone (§3); the completion and handover evidence is
  the report's own timestamps and the staged copy's mtime.
