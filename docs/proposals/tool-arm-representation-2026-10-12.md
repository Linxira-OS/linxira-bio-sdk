# Tool arms: how the second arm class gets represented

2026-10-12. Status: proposal, needs a decision on the three points at the end. This
closes the mechanism gap left by the ROADMAP rule ("parity and tool arms must be labelled
per row and never mixed") and unblocks the tool-arm round.

## Problem

The rule exists; nothing enforces it and nothing produces the second class.

- `schemas/benchmark-report.schema.json` has exactly these top-level fields:
  `schema_version`, `capability`, `dataset_class`, `backends`, `speedup`,
  `memory_saving`, `consistency`, `findings`, `environment`, `sampled_at`. There is no
  field that says which arm class a report belongs to, so a parity arm and a tool arm
  produce structurally identical artifacts. A table can mix them and no validator, no
  figure verifier and no reader can tell.
- `benchmark.run.v1` drives *our* capability packs by capability id: the harness asks a
  backend to execute a capability, so it can only measure implementations we ship. An
  ecosystem tool (seqkit, fastp, pyfastx, scikit-learn, R `prcomp`) is not a backend of
  ours; it has its own CLI, its own input expectations and its own idea of what a result
  is.
- The analysis scripts that build the published tables (`statistics_rows`,
  `comparison_rows`, `consistency_rows`, `trimmed_rows`) read reports and emit TSV rows
  without any notion of class, so the "never mixed" rule is currently a promise in a
  document.

## Options

**A. Arm class in the report, tool arms produced by a separate runner (recommended).**
Add an optional `arm_class` (`"parity"` default, `"tool"` for ecosystem tooling) plus a
`tool` block (`name`, `version`, `invocation`) to the report schema. Add a producer under
`scripts/benchmark/run/tool-arms/` that runs an ecosystem tool under the same protocol we
use for our own arms (GNU `time -v`, warmup, N ≥ 10 timed repeats, same fixtures, same RSS
and wall capture, same environment block) and writes a schema-valid report with
`arm_class: "tool"`. Make the table builders and the figure verifier reject any table or
figure that would contain both classes, and have the article tables print the class in the
caption.

- Backward compatible: every existing report keeps its meaning with `arm_class` defaulting
  to `"parity"`; nothing already published changes.
- The class travels with the number, from the measurement to the figure, which is the
  property the rule actually needs.
- Cost: schema change, one new script, class checks in two analysis scripts. No capability
  catalog entries, no user-facing surface change.

**B. A capability id per tool arm** (e.g. `expression.pca.sklearn.v1`). Rejected: the
capability catalog is a product contract that tells users what they can run, not a
benchmark ledger. It would also make "which implementation answers `expression.pca.v1`?"
ambiguous, which is exactly the confusion the two-class rule exists to prevent.

**C. Measure tool arms by hand and report them in prose.** Rejected: nothing in the
archive, no verifier, no reproducibility, and it breaks the project rule that every
published number is traceable to an archived artifact.

## What A looks like concretely

Schema (report level):

```json
"arm_class": { "enum": ["parity", "tool"], "default": "parity" },
"tool": {
  "type": "object",
  "properties": {
    "name": { "type": "string" },
    "version": { "type": "string" },
    "invocation": { "type": "string" }
  }
}
```

Producer protocol, identical to the parity arms so numbers stay comparable in *kind*:
GNU `time -v` around the tool; one discarded warmup run; N ≥ 10 timed repeats; the same
fixture file, with its sha256 recorded; tool version captured by asking the tool
(`seqkit version`, `fastp --version`, `python -c 'import sklearn'`, `Rscript -e
packageVersion()`); report written with `dataset_class` and `environment` filled the same
way.

Enforcement, in order of the pipeline:

1. report schema (`arm_class` required to be one of two values);
2. `statistics-tables.py`: refuse to write a table whose rows disagree on class;
3. `figures/verify-figures.py`: refuse a figure panel that would plot two classes together;
4. article convention: the caption states the class, and a tool-arm table never sits in the
   same table as a parity-arm table.

## What a tool arm is, and is not

It is: "what a user running the ecosystem-standard tool on the same machine, same input
and same protocol would get". It is not an algorithmic equivalence claim, not a statement
that the tool is well used (a badly configured fastp run would be our fault, not fastp's,
so the invocation is recorded verbatim in the report), and not a substitute for the parity
arms, which answer the language-runtime question.

## Acceptance criteria

- a synthetic tool-arm report validates against the report schema, and a report without
  `arm_class` still validates (default applied);
- a table builder given one parity and one tool report exits non-zero with a message naming
  both, and the same for the figure path;
- one real tool arm is measured end to end on the pinned host and its medians reproduce
  within its own bootstrap interval on a rerun;
- nothing in the already-published archive or figures changes.

## Decisions needed

1. **Option A, B or C** (recommendation: A).
2. **Field naming**: `arm_class` + `"parity" | "tool"`, or Chinese-facing wording such as
   `对照组类型` / different English terms. The rule in ROADMAP §7.1 uses the English terms,
   so A keeps the vocabulary identical to the rule.
3. **First tool arms**: suggested `seqkit stats` (FASTA/FASTQ stats), `pyfastx` (FASTQ read
   throughput), `fastp` (FASTQ QC/trim), `prcomp` (PCA) — i.e. one per capability family
   where an ecosystem standard exists, and *none* where it does not (we will not invent a
   tool to fill a row).
