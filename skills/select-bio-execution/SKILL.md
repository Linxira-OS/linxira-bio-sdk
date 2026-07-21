---
name: select-bio-execution
description: Select and govern local CPU, local GPU, HPC, cloud, or authenticated-browser execution for bioinformatics work. Use when an analysis may exceed local resources, requires large reference databases or accelerators, would upload data, could incur cost, or depends on a browser-only service such as AlphaFold Server.
---

# Select Bio Execution

Choose the lowest-risk execution mode that satisfies the measured resource and
scientific requirements.

## Selection Order

1. Use local CPU for parsing, QC, filtering, sequence operations, interval
   analysis, ordinary statistics, and result validation.
2. Use local GPU when the installed model and hardware meet the documented
   memory and runtime requirements.
3. Use an approved institutional workstation or scheduler when local memory,
   runtime, database size, scratch storage, or concurrency is insufficient.
4. Use cloud compute only after the user approves provider, account, region,
   resources, budget, data transfer, retention, outputs, and shutdown behavior.
5. Use an authenticated browser service only when no suitable programmatic or
   locally deployable route exists and the service permits the operation.

Record why the selected mode was necessary. Keep planning, upload, provisioning,
execution, download, and teardown as separate actions.

## AlphaFold Routing

- Prefer a compatible local AlphaFold 2 pipeline on an adequate local GPU.
- Move AlphaFold 2 to approved HPC or cloud GPU only when local hardware or
  databases are inadequate.
- Treat AlphaFold Server or another AlphaFold 3 browser service as a gated
  connector. Require the user to authenticate and approve sequence upload.
- Do not type or store passwords, MFA values, CAPTCHA responses, recovery codes,
  or acceptance of service terms.
- Resume browser automation only after the user provides an authenticated
  session and the intended use complies with the service policy.

## Browser Tool Boundary

CUA refers to a computer-using agent. MCP is a tool protocol; Playwright MCP and
Chrome DevTools MCP are possible browser control surfaces. Availability of a
browser tool is not authorization to use an account, transfer data, or incur
cost.

## Failure Behavior

Stop before mutation when resource estimates, data sensitivity, authorization,
cost limits, or output ownership are unclear. Return a local plan and the exact
missing decision instead of selecting a remote route implicitly.
