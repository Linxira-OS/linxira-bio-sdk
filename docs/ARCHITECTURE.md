# Architecture

## Product Definition

Linxira Bio SDK is a Chinese-first, offline-documented, AI-guided,
agent-callable, reproducible local bioinformatics workbench. It is one product
with several clients, not a collection of unrelated scripts.

```text
Native GUI       CLI       Python SDK       MCP clients
     \            |            |                 /
          versioned capability and job contract
                         |
                 local Rust worker
                  /             \
       built-in Rust code     pinned external tools
```

The GUI, CLI, SDK, and agents use the same versioned job and result contracts.
Legacy v1 remains supported; artifact-aware v2 adds stable file identity,
integrity checks, structured diagnostics, and output artifacts. A feature is
not complete when it exists only as a GUI button or an agent prompt.

## Repository Boundaries

- `skills/` contains concise selection, validation, and interpretation
  procedures for agents.
- `engine/` contains deterministic shared execution code.
- `capabilities/` is the public feature registry.
- `schemas/` defines machine contracts.
- `runtimes/` defines managed runtime providers. It does not contain downloaded
  runtimes.
- `docs/capabilities/` is the canonical offline user documentation consumed by
  the GUI, CLI, AI retrieval, and future website.
- `.linxira/` is ignored local state for development. Released applications use
  the operating system's user data directory.

`linxira-skills` remains the broader cross-discipline skill collection. This
repository owns executable bioinformatics implementations and publishes a
skill pack for that collection to import.

## Desktop Layout

The native Rust and egui desktop application has two density modes over the
same capabilities:

- Guided mode exposes file selection, safe defaults, interpretation, and the
  minimum required parameters. Environment setup separates workload selection
  from use-existing, managed-user, project-isolated, and system-missing-only
  transaction previews.
- Expert mode exposes full parameters, command previews, batch inputs, runtime
  locks, and structured outputs.

The stable layout is a project and capability navigator on the left, an
analysis workspace in the center, an optional AI/document panel on the right,
and a persistent job queue. Planned capabilities are visually separated and
cannot be executed.

## Compatibility

Windows GNU is the primary desktop build. Debian Bookworm and current Arch are
release gates. The project does not require Visual Studio, MSVC, Qt, Java UI,
or a WebView. Java is supported only as a managed analysis runtime.
