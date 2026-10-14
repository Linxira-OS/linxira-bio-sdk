# Open work (mirror of the working backlog)

**What this file is.** A durable copy of the items that are open but *waiting on
something* — a decision, a machine, or a date — kept in the repository so the list
survives the session that produced it. Items that are simply next steps live in
[ROADMAP.md](../ROADMAP.md) (§7.2 measurement infrastructure, §9.4 engine work);
this file holds only what is blocked and why.

**Rule for this file.** An item leaves when it is done or when its blocker
disappears; it is never deleted to make the list look shorter. Each row names the
next concrete action, not a wish.

Last updated: 2026-10-14.

## Waiting on the maintainer

| Item | Blocker | Next action |
| --- | --- | --- |
| `COMMERCIAL.md` contact channel | Needs the actual address (mailbox or form); the file deliberately publishes none rather than invent one | Give the address; it goes in the file and the release bundle |
| Publish the criticism request | Needs a date and a site slot; the bilingual draft is in `docs/outreach/criticism-request-post.md` and the mechanism it asks people to attack is in place | Name a date; publishing is then mechanical |
| Record and publish the promotion material | Needs the maintainer on camera/voice and account access; scripts, three notes, slide outline and the asset policy are in `promotion/` | Record Bilibili 01 and post the first two notes in the November slot |
| fastp: which route | Package, source build, or `cutadapt` in its place — the decision was left to the maintainer. Evidence: upstream ships no binary asset, and the file that looked like a download in `~/bin` is a nine-byte `Not Found` page | Pick one; seqkit already covers the streaming-reference question |
| GIAB HG002 + hap.py correctness gate | Needs approval of the $25 rental window (`docs/proposals/rental-window-plan-2026-10-12.md`: chr20 scope, 24 h, inputs pinned) | Approve the budget; the run is then scripted |
| Third-party anchors beyond zstd/OpenSSL | Silesia is unreachable from this network (measured: 32 KB in 60 s, parts reset). PTS/OpenBenchmarking uploads are public actions | Drop `silesia.zip` onto the measuring host (~2 minutes of work), or approve the uploads |
| Release-shaped acts | Rule: nothing before the repository is six months public (2027-01-21) — no `v1.1.4`, no crates.io, no JOSS | Nothing to do until the date; the prerequisites are tracked in `docs/proposals/schedule-2026-q4.md` |

## Waiting on a machine

The measuring host is currently serving other work, and the maintainer's standing
instruction is that measurements run in idle daytime slots, not at night. Two items that
were here are done as of 2026-10-14: the tool arms (pyfastx 10.0 ms, seqkit 30.0 ms, R
`prcomp` 120.0 ms, N=10, each report carrying `arm_class=tool` and a null cross-class
ratio) and the Python-leg environment isolation (interpreter generation, not package
version: cells A and B agree, cell C differs — see the window archive).

| Item | Blocker | Next action |
| --- | --- | --- |
| A same-output-set comparison for the tool-arm rows | The cross-class rows (`pyfastx stat`, R `prcomp`) do not produce the same output set as our arms, so no routing judgement can be written from them — the boundary rule needs a same-job pair first. Needs the measuring host | Measure one: make the tool emit our columns (`au_n`, `gc_percent`, `n_count`) or run `pyfastx` without its prebuilt `.fxi` index, same input, same slot. Until then those two rows stand as recorded numbers only |
| A gate for a silently missing tool arm | The registry has four arms; on 2026-10-13 the first pass produced three and said nothing, because the runner resolves `python3` through the locked venv and the version probe for scikit-learn failed there (`tool-arms.sh` prepends that venv to `PATH`). The arm was measured on 2026-10-14 in the environment that has the package, but a registry arm that produces no report is still invisible to the pipeline | Make `tool-arms.sh` compare the registry's tool ids against the reports it produced and fail on a shortfall, then re-run in the next window |
| The mechanism behind the interpreter lever | The 2026-10-13 isolation experiment showed CPython 3.14.7 is 2.24× faster than 3.12.15 on `sequence.stats.v1` (and 1.23× slower on `expression.pca.v1`) with the same packages, but not *why* — a C-extension build difference and a parsing-path difference are both consistent with the data. Deciding what to do about it (documented versions, not code) needs no machine; explaining it does | Profile the two interpreters on the same host and input (`py-spy`/`perf` on the python leg), then either record the mechanism or write it down as unexplained |
| Re-measure the window after the `sequence.stats` fix | The fix (branchless classification) changes this capability's numbers: the pipeline went 810 -> 280 ms on the same host and input. Published numbers must come from a window, not from a one-off probe. Needs the measuring host for a full window (~9 h with `sequence.ssr`), and the old archive stays as the record of the old code | Run the window driver in the next idle daytime slot; `benchmark-results/2026-10-13/` keeps the pre-fix readings |

Two items that were here are done as of 2026-10-14: the archive (summary, statistics
tables with bootstrap CIs, 11 bilingual figures each re-checked value by value, the
tool-arm table and the isolation cells, all recorded in
`benchmark-results/2026-10-13/README.md`) and the re-review of the pre-publication
fixes (the first pass's 21 findings were fixed, the fixes were re-checked, and the
new archive README went through its own claims review).

## Waiting on a decision

| Item | Blocker | Next action |
| --- | --- | --- |
| The benchmark hashes every input twice per timed repetition | `validate_v2_inputs` and `finalize_v2_input_hashes` each run a full SHA-256 over the input (66 ms measured for 101.7 MB), so 130 ms of every measured run is provenance rather than capability. Changing it changes what a report's timed window covers | Decide whether provenance is computed once per window (or declared by the caller, which `BenchmarkRequest::write` deliberately avoids today) and, if so, re-measure in a window rather than editing the harness quietly |

## Waiting on an event

| Item | Blocker | Next action |
| --- | --- | --- |
| The English mirror of the 2026-10-13 article | The Chinese draft carries the archive's numbers; the site publishes bilingual pairs, and the English side has not been written | Needs the publish decision first — if the piece goes out, the English mirror is part of that step, not a separate piece of work |
| Registry sweep for the pre-publication marker | It is present in `README.md`, the `plot.render.v1` capability pages and the 2026-10-13 article mirror; the engine ledger carries a "not applicable" note instead. It is absent from `COMMERCIAL.md`, the other capability pages and `docs/outreach/corrections-log.md` | Add the marker when each is next touched, not as a separate sweep |

## Follow-ups from the 2026-10-14 claim sweep (open work, not blocked)

| Item | Where | Next action |
| --- | --- | --- |
| Window dated 2026-10-11 quoted without a window label | `promotion/notes/xiaohongshu/01-基准实测.md:29-30`; `promotion/notes/xiaohongshu/02-一台旧电脑.md:17,20,32,44-45`; `promotion/slides/01-overview/outline.md:15-16`; `promotion/videos/01-three-way-benchmark/script.md:15,18,21,22` | Label each set with its window, or replace with the current window's numbers |
| Reviewer route reproduces only the 2026-10-11 archive | `docs/outreach/reviewer-brief.md:16-17,36,39,42-43,47-52` | Add the 2026-10-13 window and its moved verdict |
| "the 2026-10-13 window will be re-measured" | `docs/outreach/criticism-request-post.md:22` (EN) and `:100` (ZH) | The 2026-10-13 window is archived; what is pending is a post-fix window |
| Undated capability snapshot ("117 available", "112 of 117 Rust-only") | `docs/TOOLKIT_GAPS.md:5,44,81,86` | Date-stamp the snapshot or refresh it against the catalogue (129 available) |
| Shipped surfaces still read as future-tense | `docs/CAPABILITY_ROADMAP.md:43-45` (milestone 0.2: "Python bindings", "MCP server") | Mark the Python client and the MCP server as shipped, or scope the milestone to native bindings |
