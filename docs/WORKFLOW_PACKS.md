# Workflow Packs

Workflow packs adapt maintained Python, R, Java, and native ecosystems to the
same versioned job and result contracts as built-in Rust capabilities. They are
not arbitrary scripts copied into the application directory.

Every installable pack must provide a manifest conforming to
`schemas/workflow-pack-manifest.schema.json`, an immutable dependency lock,
file checksums, SPDX-compatible license metadata, input and output schemas,
platform declarations, resource requirements, and an explicit network policy.
The manifest declares a `runtime.core_compatibility` semver range (for example
`">=0.1.0,<1.0.0"`); the CLI and worker refuse to launch a pack whose range
excludes the running core build, and every result envelope records
`provenance.core_version` so consumers can audit which core produced it.
The pack is installed into an application-owned user directory and never
changes global Python, R, Java, `PATH`, or package libraries by default.

## Trust Levels

- Before becoming installable, official packs must be reviewed in this
  repository, signed, fixture-tested, and gated on Windows GNU, Debian, and
  Arch when their platform list includes those targets.
- Community packs come from a separate signed index that is disabled by
  default. Installation always displays publisher, license, source, checksum,
  dependencies, requested network access, and the exact entry point.
- Trust affects presentation and approval requirements, not filesystem or
  network isolation. Both levels are verified before activation.

Application-managed pack installation and execution remain unavailable until
`environment.apply.v1` implements download verification, staging, health
checks, atomic activation, runtime locks, and rollback. Entries marked
`planned` in `workflows/catalog.json` are product commitments, not runnable
workflows. Entries marked `cataloged` have reviewable first-party source,
contracts, locks, and manifests, but are not installable or dispatchable.

A catalog entry has one primary `capability` and may declare
`capability_aliases`. The primary identifier and every alias must be unique
across the complete catalog. The runner authorizes the capability found in the
request and requires the result to repeat that exact identifier; it never
silently rewrites an alias to the primary identifier.

The first cataloged official adapters are Biopython sequence conversion and an
R DESeq2 bulk-expression workflow. Their pack directories include strict CLI
entry points, artifact-aware schemas, atomic output handling, offline tests,
dependency and source notices, and exact file checksums. A developer may invoke
them manually in an audited isolated runtime for review. The R adapter follows
the current stable R release and requires a project package library; the
interpreter and library can be selected independently so multiple versions can
coexist. It serves `expression.differential.v1` and the explicitly
research-use-only `medical.bulk-rnaseq.v1`, while retaining
`expression.deseq2.v1` as a compatibility alias. Product availability still
requires the managed installer, a complete
resolved direct-and-transitive environment lock with source hashes and license
evidence, cross-platform fixture runs, and an executor that validates the
manifest before launch.

## Resume

A manifest may declare `resume: {"enabled": true, "state_file": "..."}`. When
enabled, the worker writes a completion-state file (request identity, input
hashes, dependency lock hash, core version, and the recorded result envelope)
inside the output directory after a successful run. A later run with identical
inputs, core build, and dependency lock replays the recorded envelope without
re-invoking the pack, provided every recorded artifact still exists and
verifies. Any mismatch — changed inputs, a stale or absent state file, an
incomplete result, or missing artifacts — falls through to a fresh pack run.
Interrupted runs never write state and never preserve partial output
directories.

## Container Execution

Workflow packs can run with `execution: {"mode": "container"}` when a
container runtime is available. The worker resolves `docker` then `podman`
(override with `LINXIRA_BIO_CONTAINER_RUNTIME`) and requires
`LINXIRA_BIO_CONTAINER_IMAGE` to name an image that already contains the
pack's runtime and dependencies. The workflow root, the request directory, and
every input file's parent directory are mounted read-only; the output parent
is mounted read-write, and the entrypoint runs inside the container with the
interpreter from `LINXIRA_BIO_CONTAINER_INTERPRETER` (default `Rscript` for R
packs, `python3` otherwise). Result artifact paths are remapped back to host
locations and validated exactly like local runs, and the recorded provenance
carries `execution_mode: container`. Container execution is only accepted for
workflow-pack capabilities; requesting it without an available runtime is a
structured error, never a silent fallback.
