# Promotion materials

Storage and rules for the material behind our public content: notes, videos and slides.
Text lives in git; binaries do not (see "What lives where").

## Two tracks, never mixed in one piece

| Track | Subject | Where it lands | What it must not do |
| --- | --- | --- | --- |
| **系统** | The toolkit: one CLI, 132 capabilities (129 available, 1 experimental, 2 planned), local-first, deterministic output, skills for agents | walkthroughs, architecture explainers, "run this analysis locally in one command" | imply model/GPU magic where the capability is a native tool |
| **工具** | The three-language benchmark and the tool arms: what Rust/Python/R and the ecosystem tools actually cost on this hardware | data-led notes and videos, the results themselves | claim generality beyond one machine, one input shape, N≥10 |

The two tracks share one rule, which is the whole reason anyone should believe either of
them: **every number in a promotion piece traces to an archived report or to a command a
viewer can run.** No number invented for a thumbnail, no "up to Nx" without the archive row
next to it, and the pieces where we lose (pre-fix readings: pyfastx is 27× faster than our own Rust arm on the
same FASTA) are the ones we lead with, because a benchmark that only reports wins reads as
advertising. We lead with them and we label them precisely: they are cross-class comparisons (the two sides do
not produce the same output set), so they are recorded losses rather than routing verdicts. The
boundary rule - call an existing implementation when it is already faster **on the same job** -
applies once a same-job comparison exists; measuring one for these rows is the open work item.

## What lives where

```
promotion/
  README.md                 this file: rules and index
  plan.md                   the program: platforms, pillars, backlog, cadence
  videos/<slug>/script.md   shot list + narration + on-screen commands (the source of truth)
  slides/<slug>/outline.md  slide-by-slide outline (text, not PowerPoint)
  notes/<platform>/<slug>.md  post text + tags + image plan
  assets/                   BINARIES: recordings, exports, .pptx — gitignored, see below
  assets/MANIFEST.md        what each binary is, its checksum, and where it is stored
```

- **Text in git**: scripts, outlines, post texts. They are reviewed like code, because they
  carry claims.
- **Binaries out of git**: `.mp4/.mov/.mkv/.pptx/.key/.psd` and raw screen recordings are
  gitignored under `promotion/assets/`. Record the artifact in `assets/MANIFEST.md` with its
  sha256 and where it actually lives (release attachment, object storage, local NAS). A 2 GB
  video inside a source repository is how a repository becomes unusable.
- **Published content** goes to the site repository (articles) or the platform itself (notes,
  videos). This tree holds the pre-publication source.

## Compliance rules for every piece

1. Numbers: archived or runnable, with the command in the description or the notes.
2. Limits: state the machine (2 cores / 4 threads, 15 GiB, `powersave`), the input size, and
   N in the piece itself, not only in a linked article.
3. Losses: if a competing tool beats us on the same input, that goes in the piece. We have
   three such results already (pyfastx, R's `prcomp`, scikit-learn).
4. Licences: AGPL-3.0-or-later for the code with a commercial path available
   (`COMMERCIAL.md`); third-party tools keep their own terms; never redistribute a dataset
   whose licence forbids it — link instead.
5. Authorship: the maintainers post as the maintainers. No sockpuppets, no vote requests, no
   astroturfed "user reviews".
6. No medical or clinical framing: the analysis capabilities are research tooling, and the
   medical ones are explicitly research-use-only. Promotion text follows the same boundary.
7. Corrections: a wrong claim in a published piece is corrected in the piece (edit note) and
   in `docs/outreach/corrections-log.md`, the same as any other error.

## Index

| Piece | Track | Platform | Status |
| --- | --- | --- | --- |
| `plan.md` | both | — | ready for the maintainer's review |
| `videos/01-three-way-benchmark/script.md` | 工具 | B站 | script ready, not recorded |
| `notes/xiaohongshu/01-基准实测.md` | 工具 | 小红书 | draft |
| `notes/xiaohongshu/02-一台旧电脑.md` | 系统 | 小红书 | draft |
| `notes/xiaohongshu/03-复现包.md` | 工具 | 小红书 | draft |
| `slides/01-overview/outline.md` | 系统 | talks / 面试 / 展会 | outline ready |
