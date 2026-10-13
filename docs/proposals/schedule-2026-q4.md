# Schedule: October repair, November content, release not before 2027-01-21

2026-10-13. Maintainer's decisions recorded here so the calendar is not re-litigated per task.

## The three dates

| When | What | Why this boundary |
| --- | --- | --- |
| **October 2026** | Structural repair: correctness of what already exists, environment/lock alignment, the benchmark window re-run, tool arms, anchors | "10月份是结构性代码修复期间" — no new capability surface, only making existing claims true |
| **November 2026** | Content production and publishing starts (小红书, B站, the criticism request) | the maintainer's call: content can go out before the product does, because content is about the work, not about a release |
| **≥ 2027-01-21** | Earliest product release and anything that reads as "we are shipping" (v1.1.4, JOSS submission, package publication) | the repository's first public commit is 2026-07-21, so six months of public history completes on **2027-01-21**; the maintainer's rule is "至少仓库公布 6 个月之后再做" |

JOSS follows the same floor: its review criteria want a public development history of at least
six months plus community evidence, and both point at the same date. Nothing about the code
needs to wait; only the *release-shaped acts* do.

## What "structural repair" means concretely (October)

1. **Environment truth**: the pack lock is fixed and CI asserts lock == environment
   (`9cf64c8`, `524381b`); the window re-run is what makes the published numbers come from the
   locked environment instead of "a venv that happened to be there".
2. **Component honesty**: arms must use the component they claim. The PCA pack's docstring
   claimed NumPy implemented its path (it only reported a version) — corrected in `2858eff`,
   and the tool-arm round now measures what the idioms actually cost (scikit-learn 725 ms vs
   our pure-Python 4010 ms on the same matrix).
3. **Claims vs archive**: every published number traced; five corrections landed (`2858eff`),
   the erratum happens in the same place the claim was made.
4. **Mechanisms that prevent recurrence**: two arm classes with a schema field and table
   guards; lock gate in CI; three-backend smoke in CI; correction ledger; issue template.

## Window re-run: what it costs (measured from the archive, not guessed)

`bench-20261011-001` totals **8.42 h** of pure measurement across three backends and eight
capabilities (N=10 each):

| backend | total | share |
| --- | --- | --- |
| rust | 41.5 s | 0.1% |
| python | 9.7 min | 1.9% |
| **r** | **8.24 h** | **97.9%** |

Two R legs are the entire bill: `sequence.ssr.v1` at 2211 s per run × 10 and `fastq.qc.v1` at
702 s × 10. Consequences:

- A **full window** (all legs, one environment, one timestamp) is **~9–10 h** including input
  regeneration, the lock install, granularity and condition passes.
- Re-measuring **only the Python arms** — the thing the lock mismatch actually invalidated — is
  **~10 minutes**.
- Re-measuring the R arms is unavoidable if the environment claim is to hold for them, and it is
  8.2 h of the 9–10.
- The R SSR leg being 36.9 min per run is itself a finding: our hand-written R implementation is
  where the window's cost lives, and a Biostrings-idiomatic arm would quantify how much of that
  is "R" versus "our R".

Held 2026-10-13 by the maintainer's call: the window waits until the host-side SRA refetch
(`aria2c`, started outside this window, 130 EBI runs, 153 GB at the time of writing) has finished, so the
published numbers are not a contended window. The driver can be armed with `WAIT_FOR_IDLE=1` to start by
itself once the host is free; the condition is the downloader's absence, not its idleness, because
`aria2c --max-tries=0` keeps retrying for days.

Interpreter correction (2026-10-13): the lock declares `Runtime: CPython 3.12` while the host runs 3.14.7;
the pinned `biopython 1.85` / `numpy 2.2.4` have no cp314 wheels, so the environment could never have
matched. The re-run installs CPython 3.12 user-locally (`uv python install 3.12`) and installs the lock
into a venv made from it, which is what makes lock == environment true rather than aspirational. See the
corrections ledger entry of the same date.

Recommendation: do the **full window once**, on a quiet machine, after the lock install, and
include the tool arms in the same session. Splitting it saves nothing (the interpreter legs
dominate) and produces two environments to explain.

## Promotion timing (November)

`promotion/plan.md` already lists the pieces; the dates move onto it: B站 01 and the first two
小红书 notes in November, then one piece per week alternating tracks. Every piece carries the
machine block, N, and the reproduction entry point, and no piece announces a release that the
calendar above forbids.
