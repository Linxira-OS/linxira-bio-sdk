# Environment Plan

## Purpose

Build an explicit platform-specific installation plan from audit evidence
without changing the computer.

## Inputs

No data files are required. The command performs an environment audit internally.

## Parameters

`PROFILE` is `local-core`, `scripting`, `managed-runtimes`, `containers`,
`sequence-search`, `genomics-cli`, or `full-local`. The default is `full-local`.

## Outputs

Each tool reports its current state, selected execution provider, installation
strategy, package or runtime, canonical and proxy-resolved sources, and
administrator requirement.

## Examples

```bash
linxira-bio environment plan <profile> --json
```

For example: `linxira-bio environment plan managed-runtimes --json`.

## Interpretation

`install` means that the catalog has a strategy for the platform. It does not
mean that the installer has been implemented.

On Windows, Unix-native genomics actions reuse an existing WSL Arch or WSL
Debian provider and expose that choice as `execution_provider`. The `containers`
profile lists alternatives when no backend exists; choose one, not all of them.

## Caveats

This capability is strictly read-only. `environment.apply.v1` remains planned,
so a plan must not be silently translated into system commands.

## Runtime Dependencies

Only the Linxira Bio CLI is required. GitHub links may be resolved through a
trusted `GITHUB_PROXY`.

## Citations

Plans come from `tools/catalog.json`; transaction requirements are documented
in `docs/RUNTIME_MANAGEMENT.md`.

## Troubleshooting

For an unknown profile, run `linxira-bio environment plan --json` or inspect
the `profiles` section of the tool catalog.
