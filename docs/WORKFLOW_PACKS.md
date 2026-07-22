# Workflow Packs

Workflow packs adapt maintained Python, R, Java, and native ecosystems to the
same versioned job and result contracts as built-in Rust capabilities. They are
not arbitrary scripts copied into the application directory.

Every installable pack must provide a manifest conforming to
`schemas/workflow-pack-manifest.schema.json`, an immutable dependency lock,
file checksums, SPDX-compatible license metadata, input and output schemas,
platform declarations, resource requirements, and an explicit network policy.
The pack is installed into an application-owned user directory and never
changes global Python, R, Java, `PATH`, or package libraries by default.

## Trust Levels

- Official packs are reviewed in this repository, signed, fixture-tested, and
  gated on Windows GNU, Debian, and Arch when their platform list includes
  those targets.
- Community packs come from a separate signed index that is disabled by
  default. Installation always displays publisher, license, source, checksum,
  dependencies, requested network access, and the exact entry point.
- Trust affects presentation and approval requirements, not filesystem or
  network isolation. Both levels are verified before activation.

Pack installation and execution remain unavailable until
`environment.apply.v1` implements download verification, staging, health
checks, atomic activation, runtime locks, and rollback. Entries marked
`planned` in `workflows/catalog.json` are product commitments, not runnable
workflows.

The first planned official adapters are Biopython sequence conversion and an R
DESeq2 bulk-expression workflow. Their implementation must be scientifically
validated before either status changes from `planned`.
