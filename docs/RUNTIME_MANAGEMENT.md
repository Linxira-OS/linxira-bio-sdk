# Runtime Management

## Default Policy

Application-managed Python, R, Java, and analysis environments are user-scoped
and isolated from global development environments. Linxira Bio must not change
global `PATH`, `JAVA_HOME`, the default Python, or the default R installation
unless a separate advanced operation is explicitly requested and approved.

The provider registry is `runtimes/catalog.json`. `runtime.catalog.v1` exposes
that registry read-only. A provider marked `cataloged` is a design commitment,
not an implemented installer.

## Providers

| Runtime need | Default provider | Purpose |
| --- | --- | --- |
| Python | uv-managed CPython 3.12 | Fast isolated Python applications and environments |
| Mixed Python/R/Bioconda | Pixi | Reproducible cross-language analysis environments |
| Existing Conda workflows | Miniforge | Compatibility with Conda/Bioconda packages |
| R installations | rig | Versioned R runtime management |
| Java | Eclipse Temurin 21 LTS | Default Java analysis runtime |
| Java compatibility | Eclipse Temurin 17 LTS | Tools that do not yet support Java 21 |

Windows native archives are preferred when maintained and verifiable.
Unix-native genomics tools run through managed WSL Debian on Windows. Debian
uses `apt` and Arch uses `pacman` only after showing the exact privileged plan.
Windows execution readiness requires WSL or Docker. Linux never probes WSL and
instead reports Docker and Podman separately, accepting either container backend.

Bioconda does not publish native Windows packages. A Windows Miniforge channel
configuration may be audited for compatibility, but Bioconda environments run
inside WSL Debian or another supported Linux backend. The UI must distinguish a
configured channel from native platform support.

## Transaction Model

`environment.apply.v1` must implement these stages before it can be marked
available:

1. Resolve an immutable version and canonical source.
2. Download into a content-addressed cache.
3. Verify SHA-256, expected publisher/source, and license metadata.
4. Extract into a staging directory.
5. Run provider and runtime health checks.
6. Write a runtime lock matching `schemas/runtime-lock.schema.json`.
7. Atomically activate the staged runtime.
8. Keep the previous lock and activation target for rollback.

Cancel, failure, or a failed health check must leave the previous runtime
usable. Repair replays the locked transaction. Remove deletes only application
owned paths after displaying them and receiving confirmation.

## Job Reproducibility

Every completed analysis records input identities, parameters, capability
version, resolved tool and runtime versions, command or backend invocation,
execution mode, warnings, citations, and result artifacts. Global tools found
during audit may be used only when the job manifest records them explicitly.
