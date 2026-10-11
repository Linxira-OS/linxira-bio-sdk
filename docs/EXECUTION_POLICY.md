# Execution Policy

## Selection Order

Choose the first execution mode that satisfies the scientific and resource
requirements:

1. Local CPU
2. Local GPU
3. Institutional workstation or scheduler
4. Approved cloud compute
5. Authenticated browser service

Do not move work off the local machine merely because a remote option exists.
Record the reason whenever a workflow leaves the local execution boundary.

## Platform And Validation Priority

Linux is the first-class platform. The project baseline targets our Arch-based
Linux distribution; Debian is the compatibility provider. Windows is a
**development machine only**: it is fine for editing, desktop preview, and
quick checks, but it is not a source of official benchmark data.

- Run release gates (`cargo fmt/clippy/test`, repository validation, packaging
  checks) on Linux whenever possible — a WSL distribution counts as Linux for
  this purpose, as does a container or remote Linux host.
- Treat Linux output as authoritative when a gate behaves differently per
  platform, and record the platform with every measurement.
- Run the gates from a Linux-native filesystem: a WSL distribution's own
  filesystem, a container, or a real Linux host. Repository tests that assert
  paths under `target/` fail when the tree is accessed through a Windows mount
  (`/mnt/c`), and platform-toolchain quirks differ there too.
- Keep build caches bounded: after large build or benchmark sessions, report
  and clean the Rust target directory (and any staged release trees) so the
  working drives do not fill up. On WSL the build cache lives inside the
  distribution's virtual disk, which also consumes the host drive holding it.

## Local Default

Keep format parsing, QC, filtering, sequence manipulation, interval operations,
small and medium matrix analysis, structure inspection, and result validation
local whenever resources permit. Stream large files and expose explicit thread,
memory, temporary-directory, and output controls.

## HPC And Cloud

Use HPC or cloud when profiling shows that local runtime, memory, GPU memory,
database size, scratch storage, or batch concurrency is inadequate. Before any
remote mutation, require the provider or scheduler, account or allocation,
region or partition, resource class, cost boundary, input location, output
location, retention policy, and shutdown behavior.

Uploading data and provisioning resources are separate approval gates. A cloud
connector must support planning and dry-run behavior before execution.

## Protein Structure Prediction

- Run AlphaFold 2 or compatible local pipelines on a local GPU or approved HPC
  environment when the model, databases, licenses, and hardware permit it.
- Treat ColabFold and hosted inference endpoints as remote connectors with data
  transfer and service-policy checks.
- Treat AlphaFold Server and other browser-only services as interactive
  connectors, not unattended compute backends.

## Browser And Authentication

Use browser automation only after the user explicitly selects the service and
approves the data transfer. Hand authentication to the user. Do not type or
store passwords, recovery codes, MFA values, CAPTCHA responses, or acceptance
of legal terms. Resume automation only after the authenticated session is
available and the service permits the intended operation.

Computer-use systems are commonly described as CUA (computer-using agents).
MCP is the tool protocol; Playwright MCP and Chrome DevTools MCP are examples of
browser control surfaces. The connector must not assume that any browser MCP
grants permission to use an account or upload biological data.
