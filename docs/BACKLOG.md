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
instruction is that measurements run in idle daytime slots, not at night.

| Item | Blocker | Next action |
| --- | --- | --- |
| Window archive: summary, statistics tables, bilingual figures, figure check | Needs a short idle slot on the measuring host (the step hashes the staged inputs, so it runs where they live). The measurement itself is finished — eight capabilities × three backends × N=10, reports staged and copied off the machine | Run `STAGES="toolarms"` through the driver, then `archive.sh`, in an idle daytime slot |
| Tool arms in this window — pyfastx, R `prcomp`, scikit-learn, seqkit | Same slot. seqkit v2.14.0 is installed and digest-verified; its `stats` argv still needs checking against the installed version before the registry entry is used | One command, ~10 minutes |
| Python-leg environment isolation | Same slot; script ready (`scripts/benchmark/windows/2026-10-13/env-isolation.sh`), three cells, ~3 minutes | Run it; the result separates interpreter from pin versions for the −19.0%…+123.6% swing |
| `sequence.stats` hot-path profile | Same slot (same machine, same input, or the numbers are not comparable) | Ranked in ROADMAP §9.4 as M5-T10 |

## Waiting on an event

| Item | Blocker | Next action |
| --- | --- | --- |
| Re-review after the fixes | The 21 findings from the first pre-publication pass are applied, but the fixes themselves were not re-reviewed (`benchmark-results/2026-10-13/blog-mirror/locked-environment-rerun-2026-10-13.zh.md`, header marker) | Spawn one review pass after the archive lands and the numbers in the draft are final |
| Registry sweep for the pre-publication marker | Only the 2026-10-13 article mirror carries the marker so far; `README.md`, `COMMERCIAL.md` and the capability pages predate the rule | Add the marker when each is next touched, not as a separate sweep |
