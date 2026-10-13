# Tool provisioning: should the product download third-party tools?

2026-10-13. Status: design note, for the maintainer's direction. Written because this session
spent hours on exactly this problem, and the lessons are the design.

## What actually happened when we tried to install four tools

| tool | what worked | what failed |
| --- | --- | --- |
| `pyfastx` | already in the pack venv (PyPI) | — |
| R `prcomp` | ships with R | — |
| `scikit-learn` | `pip install` (PyPI reachable from the measurement host) | — |
| `seqkit` | GitHub **git over https** (clone at a tag) and, once, the release CDN | release CDN truncated, reset, and returned 0 bytes on other attempts |
| `fastp` | — | **no binary assets in any release**; needs a package manager or a source build |
| `lzbench` | `git clone` + `make` | — |
| Silesia corpus | — | official and mirror hosts at 0.5–16 KB/s: unusable |

Three conclusions that shape any provisioning feature:

1. **Release CDNs are the least reliable part of the chain**; git-over-https and package
   registries were reliable, university mirrors were not.
2. **Not every tool has a downloadable artifact.** `fastp` is the example: a provisioning feature
   that assumes "there is always a URL" fails on real tools.
3. **Provenance must come from the publisher, not the transport.** The one download we could
   fully verify was seqkit, because the GitHub API exposes an asset `digest`
   (`sha256:3d664ffb…`) that matched our file byte for byte. A checksum fetched alongside the
   artifact from the same mirror proves nothing.

## Options

**A. Do not provision; verify only (recommended as the product default).** The catalog already
records which tools a capability needs (`tools/catalog.json`) and `configure-bio-environment`
already audits what is installed. Add to that: per tool, the *instructions* that work on each
platform (conda/pip/package manager/source), and a `verify` mode that checks presence, version
and, where the publisher provides one, an artifact checksum. The product never downloads
binaries; it tells the user exactly how to get them and refuses to run a capability whose tool
is missing or the wrong version.

**B. A provisioning capability (`tools.provision.v1`).** Fetch, verify, stage into a
project-local directory, no root required. Requirements learned above:
- a pinned version **and** a publisher-provided digest per platform, stored in the catalog (the
  GitHub API `digest` field is the model);
- an ordered source list per tool, because the primary source will sometimes be unusable —
  registry first, git-over-https second, release CDN last, and never a random mirror without a
  publisher digest;
- resumable, bounded downloads with an explicit failure message rather than a silent retry loop;
- **no silent installation**: a command the user runs, printing what it will fetch and where it
  will put it;
- tools with no distributable artifact (`fastp`) are marked `source-build` or `not-provisionable`
  and the capability says so.

**C. Shell out to conda/mamba/pixi and never touch downloads.** Lowest maintenance, but it makes
the product depend on a package manager being installed, and it does not help for tools that
are not packaged (`lzbench`, `seqkit` on some distros) or for corpora like Silesia.

## Recommendation

**A now, B later and narrowly.** A is honest today and costs little: the failure modes we hit are
exactly what a verification list prevents ("you need `seqkit >= 2.14`; here are the three ways to
get it; this is the digest to check"). B becomes worth building when the same three or four tools
keep blocking users, and only with digests from publishers — with the catalog carrying
`provision: {versions, digests, sources}` so the data is reviewable in one place.

Also worth writing down now, because it will bite later: **a tool arm's report must record how the
tool got there** (version, digest, source). Otherwise "measured with seqkit 2.14.0" is a claim
nobody can check, and provenance is the whole reason the tool-arm records carry the verbatim argv.

## What this does not change

The product's execution policy: local execution, no cloud, no unexpected network access. A
provisioning command is user-initiated or it does not exist.
