---
name: configure-bio-environment
description: Audit and prepare a local bioinformatics software environment with Linxira Bio environment capabilities. Use when an agent must check or configure Python, R, NCBI BLAST+, DIAMOND, samtools, bcftools, bedtools, minimap2, WSL Debian, Docker, Rust, or local GPU availability on Windows, Debian, or Arch Linux.
---

# Configure Bio Environment

Audit first, generate a platform-specific plan second, and apply changes only
after the user approves the exact actions.

## Audit

Run:

```bash
linxira-bio environment audit --json
```

Use the returned command and version evidence. Do not infer availability from
an installer, directory name, or PATH entry alone.

## Select A Profile

- `local-core`: built-in Rust capabilities with no external requirement.
- `scripting`: Python and R.
- `sequence-search`: NCBI BLAST+ and DIAMOND.
- `genomics-cli`: samtools, bcftools, bedtools, and minimap2.
- `full-local`: every currently registered local analysis tool.

Generate a plan without changing the machine:

```bash
linxira-bio environment plan PROFILE --json
```

On Windows, prefer official native archives for BLAST+ and DIAMOND. Route
Unix-native genomics tools through a managed WSL Debian environment. On Debian
use registered `apt` packages; on Arch use registered `pacman` packages.

Set `GITHUB_PROXY` when GitHub release downloads must pass through a trusted
proxy. `LINXIRA_GITHUB_PROXY` remains a compatibility fallback. Keep canonical
GitHub URLs in provenance and apply the proxy only to the resolved download
URL.

## Installation Boundary

Treat `environment.apply.v1` as unavailable until the capability catalog marks
it available. Do not translate an installation plan into shell commands and
execute them silently.

Before any installation, present the tool, version or source, strategy,
administrator requirement, license, download location, checksum policy, and
expected filesystem changes. Require explicit approval for the final plan.
Never replace an existing Python or R installation without a separate user
decision.
