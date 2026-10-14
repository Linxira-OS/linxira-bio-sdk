# One-page reviewer brief

The shortest path from a fresh clone to a verdict on our benchmark. `REPRODUCE.md` is the
long version; `corrections-log.md` is what we have already got wrong.

Machine the numbers come from: 2 cores / 4 threads, 15 GiB RAM, Linux, CPU governor
`powersave`, tmpfs working directory, interpreter packages installed from the pack lock
(`workflows/org.linxira.benchmark-python/requirements.lock`).

**Two windows are archived**: `bench-20261011-001` (`benchmark-results/2026-10-11/`) and
`bench-20261013-001` (`benchmark-results/2026-10-13/`, re-run under the interpreter the lock
describes). This brief walks the first and names the second's equivalents at each step.

## 1. Are the inputs the archive claims?

```bash
scripts/benchmark/reproduce.sh --smoke
```

Run this at the tip of the repository. The smoke check regenerates the synthetic and
granularity inputs and re-derives them deterministically; the revision each archive was
produced with is recorded inside its own reports (`89882a5a…` for 2026-10-11, `f3b9f17a…`
for 2026-10-13). **Do not check those revisions out to run this route**: they predate both
this script and the archive files, which landed later.

Expect: exit 0, and every generated input matching `benchmark-results/2026-10-11/inputs.sha256`
byte for byte (the synthetic and granularity sets; the two large inputs regenerate in
`--full` after a release build). A `WARN` about lock-versus-environment is expected for
this archive and is explained in `REPRODUCE.md` §3: the lock was regenerated after the
window, and the next window installs from it.

For the 2026-10-13 archive, compare against
`benchmark-results/2026-10-13/inputs.sha256`, again from the tip of the repository (the
window's own revision carries the code, not the archive files). The two windows generate the
same inputs, checked hash by hash.

## 2. Do the three implementations agree?

```bash
cargo build --release -p linxira-bio-cli -p linxira-bio-worker   # pack interpreter via LINXIRA_BIO_WORKFLOW_PYTHON
./target/release/linxira-bio benchmark run sequence.stats.v1 fasta=<input.fa> \
  --backends rust,python,r --repeat 2 --json
```

Expect `consistency: consistent`. The comparison is over the whole result object with a
relative tolerance of 1e-6, not over a hand-picked scalar.

## 3. Do the published medians reproduce?

The stage scripts are committed verbatim, one directory per window
(`scripts/benchmark/windows/2026-10-11/`, `.../2026-10-13/`), each with a rerun driver that
regenerates inputs after a reboot. `scripts/benchmark/reproduce.sh --full` prints them with
the archived medians to compare against.

`sequence.stats.v1` reads **810 / 825 / 1815 ms** (rust / python / r) in the 2026-10-11
archive and **810 / 1845 / 1810 ms** in the 2026-10-13 archive. That window is where the
one verdict moved: `sequence.stats` is `detectable-immaterial` against the Python arm in
2026-10-11 (1.0185, CI [1.0123, 1.1111]) and `material` in 2026-10-13 (2.2778, CI
[2.2716, 2.3210]), because that capability's Python leg is sensitive to the interpreter
generation the environment is built from.

A rerun is expected to land inside the archived bootstrap intervals (10 000 resamples,
seed `20261011`, one constant for both windows), not on the same millisecond.

## 4. Do the figures say what the tables say?

```bash
python3 scripts/benchmark/figures/verify-figures.py \
  --figures benchmark-results/2026-10-11/blog-mirror/figures/zh \
  --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 \
  --comparison benchmark-results/2026-09-13/summary.json \
  --granularity benchmark-results/2026-10-11 \
  --contended benchmark-results/2026-10-11/contended-window
```

Expect 1055/1055 checks for the Chinese set and 1067/1067 for the English set. For the
2026-10-13 archive the same verifier reads 986/986 (zh) and 997/997 (en), recorded in
`benchmark-results/2026-10-13/figure-check.txt`. The verifier decodes every plotted token
back to the archive and fails on decorative geometry that is not marked as decoration; it
exists because we found figures that disagreed with their tables.

## 5. What counts as falsifying us

- a regenerated input whose sha256 differs from `inputs.sha256`;
- a capability whose arms differ by more than the tolerance on the archived inputs;
- a rerun on the same machine, quiet, whose median falls outside the archived interval;
- any number in an article or figure that cannot be traced to a row in the archive;
- a claim in the text that the archive contradicts.

Please open an issue with the command you ran and the values you saw.

## 6. What is *not* claimed

- The arms are **same-algorithm parity implementations** written in three languages, not
  each language's ecosystem tooling (no `scikit-learn`, no `prcomp`, no `seqkit`, no
  `fastp`). A **tool-arm round has since been run** (2026-10-12 and 2026-10-13; four tools
  registered across the two rounds, archived under each window's `tool-arms/`). A tool arm
  stores no ratio against our arms, and the two classes never share a table or a figure.
- The numbers are **not** extrapolations to other hardware, other input sizes or
  distributed execution. Every capability was measured with N ≥ 10 repeats on one
  machine, and the machine is small.
- No third-party benchmark suite (PTS, OpenBenchmarking, SPEC) and no correctness gate
  against a truth set (GIAB + hap.py) has been run yet. Both are planned; neither has
  contributed a number to what is published today.
- Timing resolution is 10 ms, so a sub-10 ms run is recorded as `0.0` and small CVs are
  partly quantisation.
- **After** the 2026-10-13 window we found a per-base branch-misprediction defect in the
  Rust leg of `sequence.stats` (about 518 ms of its 810 ms) and fixed it on 2026-10-14.
  The corrected numbers require a new window and are not in any archive yet; the archived
  810 ms is the code that produced it.
