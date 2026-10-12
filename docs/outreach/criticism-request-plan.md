# Criticism request: gate, targets, and rules of engagement

Prepared 2026-10-12. Nothing here is posted until the maintainer picks a date (the
suggested earliest is 2026-10-27) **and** the gate below is green. This file is the
internal plan; `reviewer-brief.md` is what an outsider actually reads.

## Why ask at all

The three-language benchmark is our loudest public claim and the easiest to falsify. The
useful outcome is not applause: it is a list of counterexamples we have not thought of,
delivered by the people who wrote the libraries we timed — or by practitioners whose
pipelines look nothing like our single-file, 2-core, tmpfs fixture. A praise-only thread
means we knocked on the wrong door and must change venue, not interpretation.

## Gate before posting

| Precondition | State |
| --- | --- |
| One-command reproduction package (`REPRODUCE.md`) | **done** 2026-10-12 (`11e9f06`) |
| The pack lock installs and CI asserts lock == environment | **done** (`9cf64c8`, `524381b`) |
| `benchmark.run.v1` smoke in CI, all three backends | **done** (`e4355f2`, `3583b99`) |
| Self-correction ledger published (this directory) | **done** |
| Honest known-gaps table | **done** (`REPRODUCE.md` §3) |
| At least one third-party anchor (lzbench/Squash/PTS) | **open** — P2 |
| GIAB HG002 + hap.py correctness gate | **open** — needs a rented window and an approved budget |

Posting is allowed with the anchors open **only if the message says so**: no claim of
external validation, no "validated against community benchmarks". Claiming an anchor we
have not run would be the one mistake this whole exercise cannot survive.

## Targets, and what each one is being asked

| Target | Why them | Channel | The ask |
| --- | --- | --- | --- |
| `lmdu/pytrf`, `lmdu/pyfastx` (Lianming Du) | our Python SSR arm *is* pytrf; the paper (`doi:10.1186/s12859-025-06168-3`) makes the performance claim citable | GitHub issues | "is this comparison fair to pytrf, and where do you expect it to beat us?" |
| Bioconductor / Biostrings (Hervé Pagès) | our R arm uses Biostrings (`doi:10.18129/B9.bioc.Biostrings`) | `Bioconductor/Biostrings` issues, then support.bioconductor.org | same question, R side — including whether our hand-written power iteration is a fair stand-in for `prcomp` |
| Biopython | our Python FASTQ sequence arm is Biopython | `biopython/biopython` issues | whether the measured gap reflects an idiomatic use of Biopython or our own hot loop |
| htslib / samtools (James K Bonfield) | the BGZF/gzip question was already argued in public (`samtools/htslib` issue #433); they know where parallel decompression is real | `samtools/htslib` issues | whether our planned direction (parallel BGZF, single-threaded gzip with a faster backend) matches their experience |
| Biostars | practitioners who produce "your benchmark is wrong because X" for free | biostars.org | post the one-page brief; ask for the strongest counterexample |
| r/bioinformatics | tool-selection audience; will say plainly if the protocol makes the numbers useless | reddit.com/r/bioinformatics | same |
| Bioinformatics Stack Exchange | format rewards a precisely stated falsifiable question over an announcement | bioinformatics.stackexchange.com | ask a question: "is this comparison protocol sound?" |
| nf-core / Nextflow community | the people who run this class of tool at scale, with different I/O patterns | nf-core Slack, GitHub discussions | whether a 2-core, tmpfs, single-file profile is relevant to them at all |
| rust-bio | Rust-side reviewers of the "Rust is 10-1000× faster" reading | GitHub discussions | whether the parity-arm framing is honest about what is compared |
| Hacker News | broad technical attack on protocol and framing | Show HN (one post) | same brief; expect the protocol to be the target |
| Zenodo | a citable DOI so criticism cites a frozen artifact | zenodo.org | deposit after the criticism round |
| bioRxiv | full protocol for reviewers | biorxiv.org | optional, after the anchors land |

## The message

Short, specific, no marketing, and it leads with the weakest part of our own work:

> We published a three-language (Rust / Python / R) benchmark of bioinformatics
> capabilities on one deliberately modest machine, with a reproduction package
> (`REPRODUCE.md`), a self-correction ledger, and a known-gaps list that says what we have
> not measured. Before this becomes something people cite, we would rather find out where
> it is wrong.
>
> Concretely: the arms are same-algorithm implementations, not your idiomatic tooling; the
> machine is 2 cores / 4 threads with a 10 ms timing clock; there is no third-party
> benchmark anchor yet. If you have five minutes, the brief lists the commands and the
> values to expect. If you have more, tell us the counterexample we missed — the inputs,
> the command, the number.

## Rules of engagement

- Disclose authorship and affiliation in every venue, in the post itself.
- No vote requests, no cross-posting the same text into unrelated threads, no asking
  allies to amplify.
- Every incoming objection is logged in `corrections-log.md` — including the ones we
  reject, with our reasons. The log is append-only; entries are never deleted.
- Answer within 48 hours, even if only to say "we need a week".
- Never argue a number against a measurement: if a critic reports a number that
  contradicts ours, reproduce theirs before defending ours.
- No claim in a reply that is not in the archive or reproducible by a command we can post.
