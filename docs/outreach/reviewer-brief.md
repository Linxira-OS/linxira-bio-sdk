# One-page reviewer brief

The shortest path from a fresh clone to a verdict on our benchmark. `REPRODUCE.md` is the
long version; `corrections-log.md` is what we have already got wrong.

Machine the numbers come from: 2 cores / 4 threads, 15 GiB RAM, Linux, CPU governor
`powersave`, tmpfs working directory, interpreter packages installed from the pack lock
(`workflows/org.linxira.benchmark-python/requirements.lock`).

## 1. Are the inputs the archive claims?

```bash
git checkout 89882a5a31714e12e0dee4333d20f2b50b0c99c5   # archived revision
scripts/benchmark/reproduce.sh --smoke
```

Expect: exit 0, and every generated input matching `benchmark-results/2026-10-11/inputs.sha256`
byte for byte (the synthetic and granularity sets; the two large inputs regenerate in
`--full` after a release build). A `WARN` about lock-versus-environment is expected for
this archive and is explained in `REPRODUCE.md` §3: the lock was regenerated after the
window, and the next window installs from it.

## 2. Do the three implementations agree?

```bash
cargo build --release -p linxira-bio-cli -p linxira-bio-worker   # pack interpreter via LINXIRA_BIO_WORKFLOW_PYTHON
./target/release/linxira-bio benchmark run sequence.stats.v1 fasta=<input.fa> \
  --backends rust,python,r --repeat 2 --json
```

Expect `consistency: consistent`. The comparison is over the whole result object with a
relative tolerance of 1e-6, not over a hand-picked scalar.

## 3. Do the published medians reproduce?

The three stage scripts in `scripts/benchmark/windows/2026-10-11/` are the commands that
were executed, committed verbatim, including the rerun driver that regenerates inputs
after a reboot. `scripts/benchmark/reproduce.sh --full` prints them with the archived
medians to compare against, e.g. `sequence.stats.v1` at 810 / 825 / 1815 ms for
rust / python / r. A rerun is expected to land inside the archived bootstrap intervals
(10 000 resamples, seed `20261011`), not on the same millisecond.

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

Expect 1055/1055 checks for the Chinese set and 1067/1067 for the English set. The
verifier decodes every plotted token back to the archive and fails on decorative geometry
that is not marked as decoration; it exists because we found figures that disagreed with
their tables.

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
  `fastp`). A tool-arm round is planned and will be labelled separately; parity and tool
  arms are never mixed in one table.
- The numbers are **not** extrapolations to other hardware, other input sizes or
  distributed execution. Every capability was measured with N ≥ 10 repeats on one
  machine, and the machine is small.
- No third-party benchmark suite (PTS, OpenBenchmarking, SPEC) and no correctness gate
  against a truth set (GIAB + hap.py) has been run yet. Both are planned; neither has
  contributed a number to what is published today.
- Timing resolution is 10 ms, so a sub-10 ms run is recorded as `0.0` and small CVs are
  partly quantisation.
