---
name: configure-bio-environment
description: Audit and prepare a local bioinformatics software environment with Linxira Bio environment capabilities. Use when an agent must check tools, select a workload and installation scope, review a transaction preview, or configure managed Python, R, Java, uv, Pixi, rig, Miniforge, Conda/Bioconda, NCBI BLAST+, DIAMOND, samtools, bcftools, bedtools, minimap2, WSL Debian, WSL Arch, Docker, Podman, Rust, or local GPU availability on Windows, Debian, or Arch Linux.
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
- `scripting`: Python, R, and Java analysis runtimes.
- `managed-runtimes`: uv, Pixi, rig, Miniforge, Python, R, and Java.
- `containers`: Windows WSL/Docker or Linux Docker/Podman backends.
- `sequence-search`: NCBI BLAST+ and DIAMOND.
- `genomics-cli`: samtools, bcftools, bedtools, and minimap2.
- `full-local`: every currently registered local analysis tool.

Generate a plan without changing the machine:

```bash
linxira-bio environment plan PROFILE --mode MODE --json
```

Select one mode:

- `use-existing`: report missing tools without proposing installation.
- `managed-user`: preserve detected tools and stage missing unprivileged tools
  below the user data root. Use this by default.
- `project-isolated`: require `--project-root PATH` and target
  `PATH/.linxira-bio` with a project runtime lock.
- `system-missing-only`: preserve detected tools and preview only missing
  system packages. Treat every proposed change as privileged.

Inspect `transaction.target_root`, `cache_root`, `lock_path`, `stages`,
`checksum_policy`, `license_policy`, `activation_policy`, `requires_admin`,
`system_mutation`, and `blockers`. Do not interpret a transaction preview as
executable approval.

Treat actions marked `alternative` as mutually exclusive. Require a provider
choice before building a transaction; never install every backend option.

Inspect the supported user-scoped providers separately:

```bash
linxira-bio runtime catalog --json
```

A provider marked `cataloged` is not installable until
`environment.apply.v1` becomes available. Prefer uv-managed CPython, Pixi for
mixed R/Bioconda environments, rig-managed R, and Eclipse Temurin 21 with 17
only for compatibility. Do not change global `PATH`, `JAVA_HOME`, Python, or R
defaults as part of an application-managed plan.

On Windows, prefer official native archives for BLAST+ and DIAMOND. Route
Unix-native genomics tools through WSL Arch for the current platform or WSL
Debian for compatibility. Prefer an already configured provider; when both are
available, prefer Arch unless the workflow requires a Debian-only component.
On Debian use registered `apt` packages; on Arch use registered `pacman`
packages.

On Windows, report the execution backend ready when WSL Arch, WSL Debian, or
Docker is available. Keep the two WSL providers separate in structured output.
On Debian and Arch, do not probe WSL; probe Docker and Podman separately and
accept either as a local container backend. When Conda exists, report its
distribution, root, channels, Bioconda presence, and strict channel priority.
Keep `conda-forge` ahead of `bioconda`; do not add `defaults` to a Miniforge
environment.

Treat Linxira WSL as planned. Do not claim it is installable until a versioned
rootfs, provenance record, upgrade policy, and `environment.apply.v1` provider
are published.

Set `GITHUB_PROXY` when GitHub release downloads must pass through a trusted
proxy. `LINXIRA_GITHUB_PROXY` remains a compatibility fallback. Keep canonical
GitHub URLs in provenance and apply the proxy only to the resolved download
URL.

## Installation Boundary

Treat `environment.apply.v1` as unavailable until the capability catalog marks
it available. Do not translate a transaction preview into shell commands and
execute them silently.

Before any installation, present the tool, version or source, strategy,
administrator requirement, license, download location, checksum policy, and
expected filesystem changes. Require explicit approval for the final plan.
Never replace an existing Python or R installation without a separate user
decision.
