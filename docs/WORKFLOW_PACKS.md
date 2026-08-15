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
