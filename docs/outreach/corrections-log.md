# Self-correction ledger

Append-only. Entries are added when *we* find an error, not when someone else does; they
are not removed if the correction turns out to be inconvenient. Each entry says what was
wrong, what changed, and where a reader can check the difference. Repository-side
references are commits in this repository; article-side references are the site history.

This file exists because a benchmark whose authors do not publish their own errors is
asking to be trusted instead of checked. The correction rate below is high; that is the
honest state of a project that started measuring in public.

---

## 2026-10-12 — the Python pack lock could not be installed by anything

The lock in `workflows/org.linxira.benchmark-python/` pinned `pytrf==1.5.0` with an
escaped line-break sequence instead of real line breaks, and omitted `pyfastx`, which
`pytrf` declares. `uv pip install --require-hashes -r <lock>` therefore stopped at
`pyfastx`, which means the environment was never installed from the lock — and the CI step
that installs it never reached the pack tests behind it.

Fixed by generating the lock instead of hand-editing it (the pin set now lives in
`requirements.in`, hashes are resolved per platform), asserting lock-versus-environment in
CI, and making the measurement scripts refuse to run when the two disagree.

Commits `9cf64c8` (gate), `524381b` (regenerated lock). Check: `scripts/benchmark/analysis/verify-lock-env.py`.

## 2026-10-12 — a pack test expectation was wrong, and no CI run had ever executed the suite

`test_matches_the_rust_engine_on_mixed_repeats` expected `summary.total_bases == 22` for a
23-base fixture. The engine and both parity arms report 23, because `total_bases` counts
bases examined (the sum of sequence lengths), not bases covered by records. The
expectation was corrected to 23 with the definition written down; the published numbers do
not move, because the arms are compared on whole result objects and the window inputs are
dominated by repeat content, where the two definitions coincide.

Commit `05a0c90`. The same commit history shows why this survived: the suite could not run
in CI until the lock above was repaired. The first real execution is what found it.

## 2026-10-12 — "peak memory is measured" was an assumption, not a fact

The harness only shells out to `/usr/bin/time -v` when that binary exists; otherwise it
falls back to in-process timing, which records wall time and leaves `peak_rss_mb` null.
Nothing in the published tables said so, because every machine we measured on had GNU
`time` installed. Now: CI installs it, the smoke assertion fails with a diagnostic that
names the missing package, and `REPRODUCE.md` states the requirement.

Commits `6e17952`, `3583b99`.

## 2026-10-12 — three claims in the published articles were wrong

1. The benchmark arms were described as each language's idiomatic library use (NumPy,
   `prcomp`). They are not: all three implement the same hand-written power iteration, and
   NumPy only contributes a version string.
2. "The three toolchains agree to the median" was overstated: the values are identical as
   an envelope, but medians float at the 10 ms timing resolution.
3. The Vina cross-machine byte-equality claim omitted its precondition: the JSON is
   identical only at equal thread counts (`--cpu`); the output `.pdbqt` is not
   thread-count dependent at all.

Commit `2858eff`; the same corrections were published on the site (site history `f04aeb6`).

## 2026-10-12 — a repository rule contradicted the shipping position

`ROADMAP.md` §7.1 defined the native benchmark baseline as "ecosystem tools", which cannot
be true for a table whose arms are our own parity implementations. The rule now defines two
labelled classes — parity arms and tool arms — and forbids mixing them in one table.

Commit `2858eff` (same commit, separate rule).

## 2026-10-12 — a pack docstring claimed something the code does not do

The PCA pack's Python implementation documented NumPy as the implementation of its
eigenvector path; the module only reports the NumPy version, and the computation is pure
Python. Docstring corrected and the pack manifest refreshed.

Commit `2858eff`.

## 2026-10-12 — **retracted**: an unverifiable vendor claim

An earlier vendor matrix stated that Moore Threads ships a Rust kernel path via
`cudarc-musa`. Secondary sources repeated the name; no primary artifact could be
reproduced, so the claim was **withdrawn** (`fc0e304`), the remaining rows naming it were
removed (`ff3e4fe`), and the matrix was re-checked against primary sources only. The
retraction is recorded in the article's errata as well.

## 2026-10-11 — the R arm had six defects, found only when it was finally run end to end

The R backend had never been exercised as a whole; wiring it up surfaced six defects,
including SSR widths taken at vector level where Biostrings defines `width()` for
`XStringSet`, and an environment-detection path that could not succeed. Fixed and tested in
`90d982a`, `dd8e226`, `c567f94`, `9b9e4fa`, `005404c`, with the CI step that runs both pack
suites added in `9b9e4fa` — the step that then failed for the reason described at the top
of this file.

## 2026-10-11 — a wide numeric matrix was being parsed as SAM

Matrix inputs were accepted as SAM because the detector matched a loose pattern; the fix
requires FLAG/RNAME/CIGAR/SEQ structure before treating a file as SAM.

Commit `bfe6fd0`.

## 2026-10-12 — the first archive could not attribute timing contamination

The first clean window had no per-run start timestamps, so a busy machine could not be
attributed after the fact. Per-run `started_at` was added (`12e9f32`) and the window was
re-run from scratch into `bench-20261011-001` (`51c9892`) rather than patched.

## Figure-side corrections during the 2026-10-11/12 rework

Several figure defects were found and fixed after publication of the first set: a row-tick
convention that disagreed with the data, clipped Chinese captions, a verifier that could
pass on an empty parse, and four decision-relevant views that were missing entirely. The
ledger for that work is the verifier itself, which now decodes every plotted token back to
the archive instead of trusting the renderer (`3290c13`).
