# Runtime Management

## Default Policy

Application-managed Python, R, Java, and analysis environments are user-scoped
and isolated from global development environments. Linxira Bio must not change
global `PATH`, `JAVA_HOME`, the default Python, or the default R installation
unless a separate advanced operation is explicitly requested and approved.

The provider registry is `runtimes/catalog.json`. `runtime.catalog.v1` exposes
that registry read-only. A provider marked `cataloged` is a design commitment,
not an implemented installer.

Environment planning has four explicit modes:

| Mode | Target | Existing tools | Privileged actions |
| --- | --- | --- | --- |
| `use-existing` | None | Reuse | Never proposed |
| `managed-user` | User data root | Preserve | Excluded |
| `project-isolated` | `<project>/.linxira-bio` | Preserve | Excluded |
| `system-missing-only` | System data root | Preserve | Missing items only |

`audit-only` remains the separate `environment audit` operation. Project mode
requires an explicit root. A system plan is not authority to execute it.

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
Unix-native genomics tools run through WSL Arch or WSL Debian on Windows. Arch
is the default current-platform provider; Debian remains the compatibility
provider for older or Debian-only components. An existing compatible provider
is reused before another distribution is proposed. Debian uses `apt` and Arch
uses `pacman` only after showing the exact privileged plan. Windows execution
readiness requires WSL Arch, WSL Debian, or Docker. Linux never probes WSL and
instead reports Docker and Podman separately, accepting either container backend.

Bioconda does not publish native Windows packages. A Windows Miniforge channel
configuration may be audited for compatibility, but Bioconda environments run
inside WSL Arch, WSL Debian, or another supported Linux backend. The UI must
distinguish a configured channel from native platform support.

Linxira WSL follows the Arch provider path but is not installable yet. Publishing
it requires a versioned minimal rootfs, checksums and provenance, an upgrade and
rollback contract, and explicit integration with `environment.apply.v1`.

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

`environment.plan.v1` exposes these stages as a dry-run transaction preview,
including target, shared cache, lock path, administrator requirement, system
mutation flag, and blockers. `apply_available` and `ready_to_apply` remain
false until the apply capability satisfies every stage above.

## Job Reproducibility

Every completed analysis records input identities, parameters, capability
version, resolved tool and runtime versions, command or backend invocation,
execution mode, warnings, citations, and result artifacts. Global tools found
during audit may be used only when the job manifest records them explicitly.
