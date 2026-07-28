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

## Current Capabilities

The current local core audits bioinformatics prerequisites, identifies and
previews common biological files, calculates deterministic FASTA, FASTQ, SAM,
VCF, BED-intersection, expression-matrix, and PDB metrics, and exports
structured result tables as CSV, TSV, JSON, JSONL, or XLSX:

```bash
cargo run -p linxira-bio-cli -- environment audit --json
cargo run -p linxira-bio-cli -- environment plan sequence-search --mode managed-user --json
cargo run -p linxira-bio-cli -- runtime catalog --json
cargo run -p linxira-bio-cli -- workflow packs --json
cargo run -p linxira-bio-cli -- dataset inspect tests/fixtures/data-inspection/variants.vcf --json
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
cargo run -p linxira-bio-cli -- fastq qc tests/fixtures/fastq-qc/valid.fastq --json
cargo run -p linxira-bio-cli -- alignment qc tests/fixtures/alignment-qc/valid.sam --json
cargo run -p linxira-bio-cli -- variant stats tests/fixtures/variant-stats/mixed.vcf --json
cargo run -p linxira-bio-cli -- interval intersect tests/fixtures/interval-intersect/left.bed tests/fixtures/interval-intersect/right.bed --json
cargo run -p linxira-bio-cli -- expression matrix-qc tests/fixtures/expression-matrix/counts.tsv --json
cargo run -p linxira-bio-cli -- structure pdb tests/fixtures/structure-pdb-summary/alphafold-style.pdb --alphafold-plddt --json
cargo run -p linxira-bio-cli -- export table result.json result.xlsx
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
cargo run -p linxira-bio-worker -- tests/fixtures/jobs/dataset-inspect.json
cargo run -p linxira-bio-ui
cargo run -p linxira-bio-ui -- tests/fixtures/structure-pdb-summary/alphafold-style.pdb
```

The native GUI provides capability-aware result charts for FASTA, FASTQ, SAM,
BED intersections, expression matrices, VCF, and PDB summaries. It renders
local plain or gzip-compressed PDB/mmCIF coordinates as backbone,
ball-and-stick, or space-filling representations. Structure files stay local
and are bounded to 128 MiB after decompression and 100,000 atoms. The current
view can be exported atomically as a 1600 by 1000 PNG. PDB analysis and
optional explicit AlphaFold pLDDT handling use
`structure.pdb.summary.v1`; mmCIF is currently a viewer input, not an available
analysis capability.

First-party Python and R workflow scripts ship under `workflows/` with their
schemas, tests, locks, and license notices. They are local optional backends:
the release does not bundle third-party interpreters, packages, databases, or
models. `linxira-bio workflow run` verifies each pack before directly invoking
an approved local interpreter. A cataloged pack is not an installed runtime or
an available analysis capability.

Release bundles are staged from `packaging/bundle-manifest.json`, which always
includes the canonical bilingual `docs/` tree, schemas, catalogs, skills, and
license notices. Repository contract checks use the pinned Draft 2020-12
validator in `requirements-ci.txt`:

```bash
python -m venv .venv-ci
# Activate .venv-ci, then run:
python -m pip install --requirement requirements-ci.txt
python scripts/validate-repository.py
python scripts/generate_third_party_notices.py --check-config
python -m unittest discover -s tests/python -p "test_*.py"
python scripts/stage-release.py --check
```

Platform packaging calls the same release staging script with its compiled
binary directory. Staging resolves the locked target-specific release graph
and generates `THIRD_PARTY_DEPENDENCIES.json` plus
`THIRD_PARTY_DEPENDENCIES.txt`; missing, ambiguous, stale, or modified license
text fails the release. See `docs/DEPENDENCY_NOTICES.md`.

## Execution Model

Local execution is the default. Move work to a local GPU, an institutional
scheduler, or approved cloud compute only when measured CPU time, memory, GPU,
database, or storage requirements exceed the local execution envelope.

Browser-only services are connectors, not compute kernels. They require an
explicit user action gate, human-controlled authentication, and compliance with
the service terms. The project never stores or auto-fills account credentials.

See `docs/ARCHITECTURE.md`, `docs/RUNTIME_MANAGEMENT.md`,
`docs/AI_AND_SDK.md`, `docs/DOCUMENTATION_POLICY.md`, and the existing policy
documents for the product boundary, staged scope, supported data formats, and
non-Visual-Studio build direction. The exact read, inspect, analysis, and
export matrix is in `docs/DATA_FORMATS.md`.

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
