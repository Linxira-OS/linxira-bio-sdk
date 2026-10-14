# Self-correction ledger

Append-only. Entries are added when *we* find an error, not when someone else does; they
are not removed if the correction turns out to be inconvenient. Each entry says what was
wrong, what changed, and where a reader can check the difference. Repository-side
references are commits in this repository; article-side references are the site history.

This file exists because a benchmark whose authors do not publish their own errors is
asking to be trusted instead of checked. The correction rate below is high; that is the
honest state of a project that started measuring in public.

---

## 2026-10-14 — we explained a ratio before splitting the number, and our side of it was defective

**What we said.** Section 7.1 of the 2026-10-13 draft explained the `sequence.stats` gap to
`pyfastx` as "我们的记录级开销与逐碱基扫描", and the promotion material carried the 27x / 81x
ratios as a loss to own. (The 2026-10-11 article's verdict for that capability - "可分辨但不构成
实质差距" - concerns a different quantity, Rust against Python inside that window: it stands as that
window's record under that window's interpreter, and the re-measured window decides what holds now.)

**What was actually true.** Splitting 810 ms closes in three terms: the in-process CLI path 0.66 s (which already contains
~10 ms of input reading and ~518 ms of per-base branch misprediction - a defect, now fixed), plus
two SHA-256 passes that the benchmark harness runs per timed repetition (~130 ms, a measurement
convention, open decision), plus under 10 ms of worker spawn and request construction (below the
timer resolution): 0.66 + 0.13 + <0.01 = 810 ms. Our first explanation - one `String` allocation per line - was killed by
measurement: unwrapping the same bases moved the CLI path by tens of milliseconds.

**Why it happened.** Neither the measurement nor the code review was wrong; the *explanation*
was written before the number was decomposed. That is now a standing rule: any "why is it slow"
claim is preceded by per-phase readings.

**What changed.** `fasta_stats` classifies through a compile-time 256-byte table and accumulates
branchlessly (case equivalence structural, stats JSON byte-identical on the real input). Measured
on the same host and input: CLI path 0.66 s -> 0.14 s, benchmark pipeline 810 -> 280 ms. Both are
probe readings; numbers for publication require a new window, and the 2026-10-13 archive keeps
the pre-fix readings as its own record.

**What was relabelled.** Every place that quotes those ratios - the article draft, the promotion
notes, the slide and video outlines, the outreach post (both language tables) - now carries the
pre-fix label; the artefact behind it is cited from the article (§7.2) and from `promotion/plan.md`.

**Check.** `git show 1000091` (the fix, the ledger and the probe);
`docs/engine-evals/sequence-stats-branch-cost-2026-10-14.md`;
`scripts/benchmark/analysis/profile-sequence-scan.rs`.

**Still open.** The redundant second hash per repetition (decision), the same classifier shape in
`fastq.rs::add_base` (registered as M5-T11), and the re-measured window.

## 2026-10-14 — internal contradictions we found while answering an outside critique

We asked a reviewing agent to check an outside critique claim by claim, and had it report against the
repository rather than in our favour. Five of its findings are contradictions *inside* our own documents; all
five are fixed in the same pass. They are recorded here because a disagreement between two of our own files
is the cheapest thing for a hostile reader to quote.

- **The 2026-10-11 article contradicted itself about the comparison arms.** The §4 arm table caption read
  "third-party implementations, not ours" while §18 read "same-algorithm implementations we wrote (parity
  arms)"; §4's construction paragraph says the latter (it was added there on 2026-10-12). The caption now
  states it. Wording only.
- **A promotion row overstated the catalogue.** `promotion/plan.md` said "132 available capabilities"; the
  same wording sat in `promotion/README.md` and in `promotion/notes/xiaohongshu/02-一台旧电脑.md` (only that
  note). All three now state the breakdown: `capabilities/catalog.json` holds 132 in total — 129 available,
  1 experimental, 2 planned.
- **The charter described shipped surfaces as planned.** `docs/PROJECT_CHARTER.md` said "planned native
  bindings" and "planned MCP server" while the Python SDK ships a subprocess client at v0.1.0 and the MCP
  server is an implemented stdio JSON-RPC binary; `README.md` (both languages) said the Python SDK was
  planned. Both now state what exists, without path-style references into the core tree (the methods tree is
  checked for self-containment).
- **A backlog line about the pre-publication marker was stale**, claiming only one document carried the
  marker. It also named two skill bodies that do not carry it — an earlier commit message claimed they did,
  which is where the line went wrong. The line now names the files that do (`README.md`, the
  `plot.render.v1` capability pages), and the article mirror and the engine ledger are recorded separately.
- **The README's benchmark summary named an earlier window as current.** `README.md` (both languages) said
  the current N=10 window was `bench-20261011-001` and quoted only its verdicts. The current window is
  `bench-20261013-001`, re-measured under the interpreter the pack lock describes, and one verdict moved
  there: `sequence.stats` goes from detectable-but-immaterial (1.02x) to material (2.2778x, CI
  [2.2716, 2.3210]). Both halves now name both windows, state the moved verdict, and add that the Rust
  leg's branch-misprediction defect (found 2026-10-14) needs a further window before its corrected numbers
  can be quoted.

**Check.** `git show c02a220`; `capabilities/catalog.json`; `promotion/plan.md`;
`promotion/README.md`; `promotion/notes/xiaohongshu/02-一台旧电脑.md`; `docs/FIGURE_COVERAGE.md`;
`benchmark-results/2026-10-11/blog-mirror/three-language-benchmark-2026-10-11.{en,zh}.md` §4 and §18;
`benchmark-results/2026-10-13/` (the moved verdict); `docs/PROJECT_CHARTER.md`; `README.md`;
`docs/BACKLOG.md`.

**Still open.** The published copy of the 2026-10-11 article on the site still carries the old caption;
correcting it needs the site checkout, and the same pass will re-run the site build.

## 2026-10-13 — the lock declared an interpreter the measuring host does not run

`workflows/org.linxira.benchmark-python/requirements.lock` carries the line `Runtime: CPython 3.12`, and
CI installs it into 3.12. The measuring host runs CPython 3.14.7. Nobody had compared those two numbers.

The consequence is sharper than "the environment drifted": the pins cannot install on 3.14 at all.
`pip download --only-binary=:all: biopython==1.85` answers `from versions: 1.86, 1.87, 1.88`, and the
1.85 source build fails; `numpy==2.2.4` answers `from versions: 2.3.2, …`. So the library versions that
window ran with (`biopython 1.88 / numpy 2.5.4`) were not a mistake during setup — they were the newest
versions Python 3.14 accepts. The error was leaving the lock and the host two interpreter generations
apart, and then describing the result as an environment that merely "did not match".

What changes: the re-run installs the interpreter the lock declares (a user-local CPython 3.12 via `uv`,
no system package), creates the environment with `uv pip install --require-hashes -r <lock>`, and the
existing gate (`scripts/benchmark/analysis/verify-lock-env.py`) proves lock == environment before any
number is measured. The window's `code-label.txt` records the interpreter that actually ran.

Check: `uv python install 3.12`, then `uv pip install --require-hashes -r workflows/org.linxira.benchmark-python/requirements.lock`,
then `python3 scripts/benchmark/analysis/verify-lock-env.py --lock workflows/org.linxira.benchmark-python/requirements.lock`.

## 2026-10-13 — we attributed a version pair to an archive that never recorded it

Both the second article's draft and the entry above originally said the *archived* environment blocks
carry `biopython 1.88 / numpy 2.5.4`. They do not: every report in `benchmark-results/2026-10-11/`
records only the interpreter (`Python 3.14.7`) and the R version. The library versions were real — they
were what that environment had installed — but their only written source is the previous article's
environment note and `REPRODUCE.md`, not the measurement archive.

A reader following the citation would have found the claim false. Found by the article review pass
(the first run of the pre-publication checks now recorded in `AGENTS.md`), fixed in the draft and here.

What changes: the wording now names the real source, and the underlying gap is registered as work —
the environment block should record the measured library versions (interpreter *and* pinned packages),
so the next window's archive can be checked on that point without reading prose.

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
