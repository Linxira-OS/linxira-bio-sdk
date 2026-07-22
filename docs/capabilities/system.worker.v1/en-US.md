# Local Job Worker

## Purpose

Invoke registered local capabilities through a common JSON request for
workflows, SDKs, and agents.

## Inputs

One UTF-8 JSON file conforming to `schemas/job-request.schema.json`.

## Parameters

The only command argument is the request path. Relative input paths resolve
from the directory containing the request file.

## Outputs

Standard output returns one `AnalysisResult` JSON object. Errors go to standard
error with a nonzero exit code.

## Examples

```bash
linxira-bio-worker <job-request.json>
```

The repository fixture runs as
`linxira-bio-worker tests/fixtures/jobs/sequence-stats.json`.

## Interpretation

A successful result contains capability and job IDs, structured results, and
provenance. Consumers should read fields instead of parsing human-readable text.

## Caveats

The current worker supports only `local-cpu` and explicitly registered
capabilities. Planned features and remote modes are rejected.

## Runtime Dependencies

Requires the Linxira Bio Worker executable and runtimes declared by the selected capability.

## Citations

Request and result contracts are defined in `linxira-bio-protocol` and repository `schemas/`.

## Troubleshooting

For an unsupported schema or capability, verify `schema_version`, `capability`,
and the status in the capability catalog.
