# Linxira Bio SDK

Linxira Bio SDK is a local-first, agent-native bioinformatics execution
toolkit. It combines concise skills with stable CLI, SDK, and tool contracts so
that routine analyses use tested implementations instead of generating a new
script for every run.

Windows is the primary desktop and beginner-facing platform. Debian and Arch
are the supported Linux families for workstation, server, and HPC use. macOS is
not currently a tested or packaged target.

On Windows, WSL Debian is the compatibility provider for older bioinformatics
components, while WSL Arch is the current-platform provider and the future
Linxira WSL foundation. Linxira WSL installation remains planned until a
versioned rootfs and upgrade contract are published.

The repository is the canonical home of executable bioinformatics skills and
their shared runtime. It does not replace `linxira-skills`, which remains the
cross-discipline skill router and installer for research, Linux, HPC, cloud,
browser, and delivery workflows.

## Product Surfaces

- `linxira-bio`: command-line interface for people and workflows
- `skills/`: agent instructions bound to versioned capabilities
- `capabilities/`: machine-readable capability catalog
- `engine/`: Rust runtime and future benchmark-justified C++ kernels
- `skill-pack.json`: import boundary for agent runtimes and `linxira-skills`
- `linxira-bio-ui`: native Rust desktop application without a WebView
- Python SDK: planned after the CLI contract stabilizes
- MCP server: planned after the capability and result schemas stabilize

## Current Capability

The current local core audits bioinformatics prerequisites and calculates
deterministic FASTA sequence and assembly statistics:

```bash
cargo run -p linxira-bio-cli -- environment audit --json
cargo run -p linxira-bio-cli -- environment plan sequence-search --mode managed-user --json
cargo run -p linxira-bio-cli -- runtime catalog --json
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
```

Environment plans support `local-core`, `scripting`, `managed-runtimes`,
`containers`, `sequence-search`, `genomics-cli`, and `full-local`. They are
read-only. Installation remains a separate, explicitly approved capability.
Set `GITHUB_PROXY` to resolve canonical GitHub release URLs through a trusted
download proxy.

Planning modes are `use-existing`, `managed-user`, `project-isolated`, and
`system-missing-only`. Every plan includes a dry-run transaction boundary;
`environment.apply.v1` remains planned and cannot execute that preview.

Inspect the runtime and capability catalog:

```bash
cargo run -p linxira-bio-cli -- doctor --json
cargo run -p linxira-bio-cli -- capabilities --json
cargo run -p linxira-bio-worker -- tests/fixtures/jobs/sequence-stats.json
cargo run -p linxira-bio-ui
```

Release bundles are staged from `packaging/bundle-manifest.json`, which always
includes the canonical bilingual `docs/` tree, schemas, catalogs, skills, and
license notices. Validate those inputs with
`python scripts/stage-release.py --check`; platform packaging calls the same
script with its compiled binary directory.

## Execution Model

Local execution is the default. Move work to a local GPU, an institutional
scheduler, or approved cloud compute only when measured CPU time, memory, GPU,
database, or storage requirements exceed the local execution envelope.

Browser-only services are connectors, not compute kernels. They require an
explicit user action gate, human-controlled authentication, and compliance with
the service terms. The project never stores or auto-fills account credentials.

See `docs/ARCHITECTURE.md`, `docs/RUNTIME_MANAGEMENT.md`,
`docs/AI_AND_SDK.md`, `docs/DOCUMENTATION_POLICY.md`, and the existing policy
documents for the product boundary, staged scope, and non-Visual-Studio build
direction.

## Source Policy

`GPTomics/bioSkills` is a primary method and example source.
`BioTender-max/awesome-bio-agent-skills` is a discovery index with per-source
license boundaries. Upstream bodies remain research inputs until provenance,
license, scientific correctness, and executable behavior have been reviewed.

The ignored `.research/` directory contains disposable source clones and must
not enter a release.

## License

Project-owned code, skills, GUI, SDK, worker, and network-facing services are
released under `AGPL-3.0-or-later`. Modified versions offered to users over a
network must provide the corresponding source as required by the AGPL.
Third-party components retain their own notices and terms; see
`THIRD_PARTY.md`.
