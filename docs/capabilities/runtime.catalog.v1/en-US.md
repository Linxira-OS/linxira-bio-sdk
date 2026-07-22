# Runtime Catalog

## Purpose

List the Python, R, Java, and mixed analysis environment providers planned for
management by Linxira Bio.

## Inputs

No input files are required.

## Parameters

Use `--json` to return the complete stable catalog object.

## Outputs

Returns provider IDs, runtime types, managers, version policies, platforms,
licenses, sources, and health checks.

## Examples

```bash
linxira-bio runtime catalog --json
```

## Interpretation

`cataloged` means that the design and license boundary is registered. It does
not mean the current release can install that provider.

## Caveats

The catalog does not probe the machine; use `environment.audit.v1` for current
state. It also does not change the global environment.

## Runtime Dependencies

Only the Linxira Bio CLI is required. Default providers are uv, Pixi, rig, and
Eclipse Temurin; Miniforge is cataloged as the Conda/Bioconda compatibility provider.

## Citations

Definitions are in `runtimes/catalog.json` with
`schemas/runtime-catalog.schema.json` as the schema.

## Troubleshooting

If JSON parsing fails, run the repository validator to check the embedded
catalog and schema version.
