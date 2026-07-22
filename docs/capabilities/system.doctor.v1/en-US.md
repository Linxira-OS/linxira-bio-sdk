# System Doctor

## Purpose

Quickly identify the Linxira Bio SDK platform and availability of key commands.

## Inputs

No input files are required.

## Parameters

Use `--json` for the stable machine-readable result.

## Outputs

Reports the operating system, CPU architecture, and probe state for Rust,
Python, and common local tools.

## Examples

```bash
linxira-bio doctor --json
```

## Interpretation

`available: true` means that the probe command succeeded. It does not prove
that every dependency for an analysis workflow is ready.

## Caveats

This command preserves the early doctor JSON shape. Use
`environment.audit.v1` for the complete environment inventory.

## Runtime Dependencies

Only the Linxira Bio CLI is required. External tools being checked may be absent.

## Citations

Probe definitions come from the repository `tools/catalog.json`.

## Troubleshooting

If an installed tool is missing, run its probe directly in the same terminal
and inspect the `PATH` visible to that process.
