# AI, SDK, And Agent Integration

## One Capability Surface

Automation is built on the versioned capability contract, not on GUI control
automation. New SDK integrations should use the artifact-aware v2 request and
result envelopes; v1 remains a compatibility surface. The implementation order is:

1. stabilize JSON job and result contracts;
2. add JSON-RPC over standard input/output;
3. publish a typed Python wrapper around JSON-RPC;
4. expose the same methods through `linxira-bio mcp serve`;
5. let the native GUI use the same worker and documentation sources.

The first Python SDK remains a subprocess client. This avoids tying the Python
ABI and release schedule to PyO3 before the capability model is stable.

## Runtime Boundary: No RustPython

Python and R are workflow backends reached as separate processes, not embedded
interpreters. Replacing CPython with RustPython was evaluated and rejected:

- RustPython's CPython C-API layer is still described as minimal, so the
  packages the bioinformatics packs rely on (NumPy, SciPy, Biopython, RDKit)
  cannot be imported. Pure-Python-only execution covers no pack we ship.
- Delegated work is accepted only when its output is byte-identical to the
  reference implementation; a different interpreter invalidates that standard.
- R has the same shape as CPython: the official runtime is written in C,
  Fortran, and R, and the statistical cores behind DESeq2, WGCNA, and survival
  are compiled C/Fortran. Nothing can be swapped without discarding the packages
  that define the method.

A second interpreter would move computation out of the Rust engine rather than
into it. Strengthening the Rust core means pulling computation into Rust as
scheduled in `RUST_NATIVE_ROADMAP.md`, not changing the interpreter.

One case stays open: running ephemeral, model-generated pure-Python snippets in
a sandbox that deliberately has no package ecosystem. That is a sandbox
interpreter role, not a replacement for the workflow runtimes.

## Built-In Assistant

The assistant follows a constrained workflow:

1. Understand the biological question without inventing missing sample facts.
2. Recommend an available capability and explain why it fits.
3. Validate inputs, runtime, data sensitivity, and execution envelope.
4. Produce a read-only execution plan.
5. Ask for approval before installation, filesystem mutation outside the
   project, cloud cost, remote upload, or authenticated browser use.
6. Invoke a registered capability and interpret its structured result.

It does not turn arbitrary model-generated source code into an implicit
capability. Unsupported work may be drafted in a sandbox, but it remains
unreleased until it receives contracts, tests, provenance, documentation, and
platform validation.

## Providers And Privacy

Supported AI provider boundaries are an optional local llama.cpp-compatible
runtime, a user-configured OpenAI-compatible endpoint, and external agents such
as Codex or OpenCode through MCP. No provider is mandatory for local analysis.

Before remote inference, the application displays the destination, fields or
files transmitted, retention uncertainty, expected cost class, and a local
alternative when one exists. Credentials are stored through the operating
system credential service and never written to job manifests or prompts.

Authenticated browser connectors remain human controlled. The project does not
automate passwords, MFA, CAPTCHA, or acceptance of service terms.
