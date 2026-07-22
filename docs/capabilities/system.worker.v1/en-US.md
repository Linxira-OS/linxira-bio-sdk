# Local Job Worker

## Purpose

Invoke registered local capabilities through a common JSON request for
workflows, SDKs, and agents.

## Inputs

One UTF-8 JSON file conforming to `schemas/job-request.schema.json` (legacy v1)
or `schemas/job-request-v2.schema.json` (artifact-aware v2). V2 inputs declare
stable file IDs, paths, formats, compression, sizes, and optional SHA-256 values.

## Parameters

The only command argument is the request path. Relative input paths resolve
from the directory containing the request file.

## Outputs

Standard output returns one version-matched `AnalysisResult` JSON object. A
validly identified v2 request returns `status: error` and an error diagnostic
for semantic validation or execution failure. Malformed JSON and requests that
cannot establish a reliable schema, job ID, and capability go to standard
error with a nonzero exit code.

## Examples

```bash
linxira-bio-worker <job-request.json>
```

The repository fixture runs as
`linxira-bio-worker tests/fixtures/jobs/sequence-stats.json`.

## Interpretation

A successful result contains capability and job IDs, structured results, and
provenance. V2 also records input hashes and output artifact hashes. Consumers
must inspect `status` and diagnostics instead of treating transport success as
scientific success or parsing human-readable text.

## Caveats

The current worker supports only `local-cpu` and explicitly registered
capabilities. Planned features and remote modes are rejected. V2 checks input
cardinality, unique file IDs, size, optional SHA-256, declared format and
compression when content can determine them, and detects files changed during
execution.

## Runtime Dependencies

Requires the Linxira Bio Worker executable and runtimes declared by the selected capability.

## Citations

Request and result contracts are defined in `linxira-bio-protocol` and repository `schemas/`.

## Troubleshooting

For an unsupported schema or capability, verify `schema_version`, `capability`,
and the status in the capability catalog. For v2, read the `job-failed`
diagnostic before checking process stderr.
