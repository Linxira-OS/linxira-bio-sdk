# Linxira Bio SDK

This repository builds executable, local-first bioinformatics capabilities and
the agent skills that select and validate them.

## Skill Routing

- Start biological analysis requests with `skills/route-bio-analysis/SKILL.md`.
- Read `skills/select-bio-execution/SKILL.md` when local, GPU, HPC, cloud, or
  browser execution must be selected.
- Read `skills/analyze-sequence-statistics/SKILL.md` for the implemented
  `sequence.stats.v1` capability.
- Read `skills/configure-bio-environment/SKILL.md` to audit Python, R, BLAST,
  DIAMOND, native command-line tools, WSL Debian, or GPU prerequisites.
- Do not use a capability marked `planned` as though it were available.

## Repository Rules

- Treat `skills/` as concise agent-facing procedures, not a place for shared
  implementation code.
- Put deterministic shared computation in `engine/` and expose it through a
  versioned capability, CLI command, result schema, and test fixture.
- Keep `.research/` source clones untracked and do not edit upstream bodies.
- Record provenance before adapting an upstream method or example.
- Prefer maintained native tools over reimplementing mature algorithms.
- Add C++ only after a benchmark identifies a kernel that Rust or an existing
  native dependency does not handle adequately.

## Execution Safety

- Execute locally by default.
- Require explicit approval before provisioning cloud resources, incurring
  cost, uploading data, or opening authenticated browser services.
- Never automate password entry, MFA, CAPTCHA, or acceptance of service terms.
- Keep protected, clinical, and controlled-access data out of public services
  unless the user supplies an approved data-governance path.

## Validation

Run these checks before reporting a capability complete:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
```

Use the skill creator validator for every changed folder under `skills/`.
