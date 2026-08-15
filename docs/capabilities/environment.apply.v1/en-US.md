# Environment Apply

## Purpose

Audit the local environment against a capability profile and install missing
runtime dependencies across Python, R, Java, and native tools.

## Inputs

None required. The profile is specified via the `--profile` parameter.

## Parameters

`--profile` selects the profile (default `local-core`). `--mode use-existing`
skips installation and only reports status.

## Outputs

JSON reports installed, missing, and skipped tools with per-platform package
commands ready for review.

## Examples

```bash
linxira-bio environment apply --profile local-core --json
```

## Interpretation

Review the missing list before approving installation. Use `--mode use-existing`
to audit without modifying the system.

## Caveats

Installation requires network access and may prompt for system package manager
credentials. Always review proposed changes before applying.

## Runtime Dependencies

Python 3.10+, R 4.3+, Java 17+, and system package managers (winget, apt, pacman).

## Citations

Cite the Linxira Bio SDK and the specific tools installed by the profile.

## Troubleshooting

If a tool fails to install, verify network connectivity and package manager
configuration. Run with `--mode use-existing` first to audit the current state.