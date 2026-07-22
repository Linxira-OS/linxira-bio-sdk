# Environment Plan

## Purpose

Build an explicit platform-specific installation plan from audit evidence
without changing the computer.

## Inputs

No data files are required. The command performs an environment audit internally.

## Parameters

`PROFILE` is `local-core`, `scripting`, `managed-runtimes`, `containers`,
`sequence-search`, `genomics-cli`, or `full-local`. The default is `full-local`.

`--mode` accepts `use-existing`, `managed-user`, `project-isolated`, or
`system-missing-only`; the default is `managed-user`. Project mode requires
`--project-root PATH`.

## Outputs

Each tool reports its current state, selected execution provider, installation
strategy, package or runtime, canonical and proxy-resolved sources, and
administrator requirement.

The result also includes a dry-run transaction preview with target, cache and
lock paths; checksum, license, and atomic activation policies; system mutation
and administrator flags; planned stages; and hard blockers. The payload follows
`schemas/environment-plan.schema.json`.

## Examples

```bash
linxira-bio environment plan <profile> --mode <mode> --json
```

For example:

```bash
linxira-bio environment plan managed-runtimes --mode managed-user --json
linxira-bio environment plan sequence-search --mode project-isolated --project-root ./analysis --json
```

## Interpretation

`install` means that the catalog has a strategy for the platform. It does not
mean that the installer has been implemented.

`missing` means `use-existing` mode found no usable command and intentionally
did not propose installation. `unsupported` means the platform or selected
mode cannot safely use the registered strategy. Existing tools are preserved
in every mode; `system-missing-only` proposes only missing items.

`alternative` marks mutually exclusive execution backends. Select exactly one
before treating the result as an install transaction.

On Windows, Unix-native genomics actions reuse an existing WSL Arch or WSL
Debian provider and expose that choice as `execution_provider`. The `containers`
profile lists alternatives when no backend exists; choose one, not all of them.

## Caveats

This capability is strictly read-only. `environment.apply.v1` remains planned,
so `apply_available` and `ready_to_apply` are false and a plan must not be
silently translated into system commands. A system plan is not execution
approval.

## Runtime Dependencies

Only the Linxira Bio CLI is required. GitHub links may be resolved through a
trusted `GITHUB_PROXY`.

## Citations

Plans come from `tools/catalog.json`; the result contract is
`schemas/environment-plan.schema.json`; transaction requirements are documented
in `docs/RUNTIME_MANAGEMENT.md`.

## Troubleshooting

For an unknown profile, run `linxira-bio environment plan --json` or inspect
the `profiles` section of the tool catalog. If project mode fails, provide a
non-empty `--project-root` path.
