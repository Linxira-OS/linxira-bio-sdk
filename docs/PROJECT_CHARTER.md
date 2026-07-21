# Project Charter

## Mission

Build a local-first bioinformatics SDK whose released capabilities can be used
without writing a new analysis script. Every released capability must expose a
stable command or SDK contract, structured output, a small fixture, scientific
equivalence tests, provenance, Windows verification, and Debian/Arch Linux
verification.

Project-owned components use `AGPL-3.0-or-later` so modified network services
remain subject to the AGPL source-availability obligations. Third-party
components keep their original compatible licenses and notices.

## Product Boundary

Linxira Bio SDK owns executable bioinformatics leaf capabilities and the skills
that invoke them. It supports four consumers through one capability contract:

1. Researchers use the CLI.
2. Workflows use structured requests and results.
3. Python programs use the planned native bindings.
4. Agents use the skill pack and planned MCP server.

`linxira-skills` remains the general control plane for cross-discipline routing,
installation, Linux, HPC, cloud, browser, research governance, and delivery.
It may import this repository's released skill pack, but it must not maintain a
second copy of the executable skill bodies.

## Source Roles

| Source | Role | Release rule |
| --- | --- | --- |
| `GPTomics/bioSkills` | Primary methods and examples | Adapt only with MIT provenance and scientific review |
| `BioTender-max/awesome-bio-agent-skills` | Broad discovery index | Resolve each nested source and license before adaptation |
| `Linxira-OS/linxira-skills` | General skill platform and extension knowledge | Reuse routing and policy; do not duplicate its installer |

## Non-Goals

- Do not claim complete coverage merely because a source skill is indexed.
- Do not rewrite mature tools such as samtools, bcftools, bedtools, minimap2,
  Salmon, Kraken2, MMseqs2, or Foldseek without benchmarked justification.
- Do not make clinical decisions or automate publication.
- Do not provision paid resources or use authenticated services implicitly.
- Do not make the native desktop GUI mandatory for CLI, SDK, worker, or agent
  use, and do not introduce a browser/WebView runtime as its implementation.

## Release Gate

A capability is release-ready only when it has:

- a versioned capability ID and stable input/output contract;
- deterministic implementation or a pinned maintained backend;
- representative local fixtures and error cases;
- differential or golden-result tests;
- Windows GNU verification without Visual Studio;
- Debian Bookworm and current Arch Linux verification;
- measured performance and memory behavior where performance is claimed;
- provenance, license, data-governance, and execution-mode metadata;
- a concise agent skill that does not overstate biological conclusions.
