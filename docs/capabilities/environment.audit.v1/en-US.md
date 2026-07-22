# Environment Audit

## Purpose

Read-only inspection of runtimes, tools, WSL, and GPU prerequisites on a
Windows, Debian, or Arch workstation.

## Inputs

No input files are required. The audit reads the operating system and commands
visible to the current process.

## Parameters

There are no capability parameters yet. `--json` emits the standard analysis
result envelope.

## Outputs

Returns platform data, command and version evidence for each tool, available
and missing counts, execution backend state, Conda/Bioconda configuration, and warnings.

## Examples

```bash
linxira-bio environment audit --json
```

## Interpretation

A tool is available only when its probe exits successfully and passes any
configured output match.
Windows is backend-ready when WSL Arch, WSL Debian, or Docker is available.
The two WSL distributions are reported separately. Debian and Arch hosts check
Docker and Podman separately and accept either backend.
Bioconda does not publish native Windows packages. On Windows, a configured
channel still requires WSL Arch, WSL Debian, or another supported Linux backend
for execution.

## Caveats

The audit does not install, upgrade, remove, or change environment variables.
It does not determine whether reference databases have been downloaded. On
Windows it can fall back to the R registry and registered Conda roots after a
PATH probe fails, and marks that discovery explicitly.

## Runtime Dependencies

Uses the built-in Rust auditor. WSL Arch and WSL Debian may provide Unix tools
on Windows, but this command does not create a WSL distribution.

## Citations

Tool probes come from `tools/catalog.json`; safety boundaries are in
`docs/EXECUTION_POLICY.md`.

## Troubleshooting

If WSL is enabled but both providers are missing, verify that
`wsl.exe --list --quiet` includes a distribution whose name contains `Arch` or
`Debian`. An empty list means that the Windows feature exists but no supported
distribution has been registered.
