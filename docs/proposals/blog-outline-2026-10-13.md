# Second article — outline and evidence anchors (2026-10-13)

Working outline for the article that follows the locked-environment re-run. The first
article (`three-language-benchmark-2026-10-11`) reported what the three backends cost; this
one reports what happened when we tried to make those numbers reproducible. The spine is the
interpreter discovery, so the article can be written in full once the 2026-10-13 window
lands; every section below names the archive or commit a reader can check, per the standing
rule that no number enters prose that a command cannot re-derive.

Working title: *We said the lock pinned the environment — the lock named an interpreter the
machine did not run* (zh: 《我们以为锁钉住了环境：锁声明的是机器没在跑的那个解释器》)

## Sections and their anchors

1. **The claim that broke.** The first article described the archived window as measured in a
   locked environment; the archive's environment block says `biopython 1.88 / numpy 2.5.4`
   while the lock pins `1.85 / 2.2.4`.
   Anchor: `benchmark-results/2026-10-11/*.benchmark.json` environment blocks;
   `workflows/org.linxira.benchmark-python/requirements.lock`.

2. **The deeper error.** The lock carries `Runtime: CPython 3.12` and CI installs it into
   3.12; the measuring host runs 3.14.7. The pins cannot install on 3.14 at all —
   `pip download --only-binary=:all: biopython==1.85` answers `from versions: 1.86, 1.87,
   1.88`, `numpy==2.2.4` answers `from versions: 2.3.2, …`, and the 1.85 source build fails.
   So `1.88 / 2.5.4` were the newest Python 3.14 accepts; the mistake was two interpreter
   generations between lock and host, then calling it a mismatch.
   Anchor: `docs/outreach/corrections-log.md` entry of 2026-10-13; commit `60f2bb1`.

3. **What the re-run does differently.** A user-local CPython 3.12 (`uv python install 3.12`,
   no system package), `uv pip install --require-hashes -r <lock>`, and the existing gate
   (`verify-lock-env.py`) proving lock == environment before any number is measured. Every
   report now carries `code_branch` / `code_revision` / `code_describe`, and the window
   writes `code-label.txt`; with no release published, `git describe` returns the short
   commit hash, so the label states the project's release status by itself.
   Anchor: commit `58b744a`; `benchmark-results/2026-10-13/code-label.txt` (written by the run).

4. **The second arm class, measured.** Ecosystem tools on the same host, input and protocol:
   `pyfastx stat` 30.0 ms (RSS 17.4 MB) against our Rust arm's 810 ms on the same ~100 MB
   FASTA; R `prcomp` 150.0 ms against the hand-written R arm's 2770 ms; scikit-learn `PCA`
   725.0 ms against the hand-written Python arm's 4010 ms. These are calls a user would
   actually make; the parity arms exist to isolate the runtime, not to win. Cross-class
   ratios stay in labelled prose — the report format keeps `speedup`/`memory_saving` null for
   tool arms and refuses to render both classes in one table.
   Anchor: `benchmark-results/2026-10-12/tool-arms/` and the re-run's
   `benchmark-results/2026-10-13/tool-arms/`; ROADMAP §7.1.

5. **Is the machine ordinary?** zstd 1.5.7 on this host: `-1` 242 MB/s, `-19` 1.13 MB/s
   (199 MB RSS), decompression ≈1070 MB/s; against zstd's own published table (i7-9700K,
   Silesia) that is 0.47× and 0.69×. lzbench built from source reproduces `-1` = 242 MB/s
   independently. The Silesia corpus itself is unreachable from this network (measured: 32 KB
   in 60 s, parts reset), so no comparison against a public compression number was made —
   stated as a limit, not hidden.
   Anchor: `benchmark-results/2026-10-12/machine-anchor/`, `docs/engine-evals/`.

6. **Failures first.** What did not work, in the order it failed: the release CDN truncating
   downloads from two networks; `fastp` publishing no binary asset (`~/bin/fastp` turned out
   to be a nine-byte `Not Found` page); the Silesia mirrors; and a host-side SRA refetch
   (130 EBI runs) that forced the window to wait rather than publish a contended machine.
   Anchor: this archive's `window-log.txt`; `docs/proposals/schedule-2026-q4.md`.

7. **How to check any of it.** `scripts/benchmark/reproduce.sh` (9/9 inputs byte-identical,
   289 s measured), the CI three-backend smoke, and the issue template for "I cannot
   reproduce X".
   Anchor: `REPRODUCE.md`, `.github/workflows/ci.yml`, `.github/ISSUE_TEMPLATE/reproduction-failure.yml`.

8. **What we are not doing yet.** No product release before the repository is six months
   public (2027-01-21): no crates.io publish, no JOSS submission, no announcement. Reason
   stated plainly: the correction rate is still high enough that publishing a version number
   would freeze claims we are still finding errors in. (`v1.1.4` exists only as a technical
   tag used to verify the packaging sync; it is not a shipping announcement.)

## Constraints carried from the first article

Bilingual pair with an `# H1` first line, `tag: technical`, `/assets/...` resources,
`npx astro build` green before pushing, mirrored into `benchmark-results/<date>/blog-mirror/`
before the site copy, figures zh+en from one dataset, and no sentence that a reader cannot
trace to an archive directory or a commit.
