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
