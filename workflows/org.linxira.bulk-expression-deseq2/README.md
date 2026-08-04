# DESeq2 bulk-expression workflow pack

This first-party pack performs a two-level bulk RNA-seq differential-expression
comparison with DESeq2. It accepts a raw integer count matrix plus sample
metadata as CSV or TSV, validates identifiers and replication, applies a
minimum total-count filter, fits `~ condition`, and writes:

- `differential-expression.csv`;
- `normalized-counts.csv`;
- artifact-aware `result.json` with every loaded R namespace version,
  input/output SHA-256, parameters, and summary counts.

The public capability identifiers are `expression.differential.v1` and
`medical.bulk-rnaseq.v1`; `expression.deseq2.v1` remains a compatibility
alias. The result always repeats the exact requested capability. Every result
is marked research-use-only and non-clinical. The medical entry additionally
emits a `research_use_only` warning: it does not diagnose, recommend treatment,
or provide clinical interpretation.

All three files are built in a private sibling staging directory and activated
by a same-filesystem directory rename. The requested output directory must not
already exist, and `--result` must name `<output_directory>/result.json`.

Run with the selected R interpreter and its matching project package library.
R 4.6.1 is the current preferred stable runtime and this pack accepts tested
R 4.6.x patch releases. Different projects and R versions can coexist because
the interpreter and library are selected independently:

```text
LINXIRA_BIO_WORKFLOW_R=<Rscript-for-this-environment>
LINXIRA_BIO_WORKFLOW_R_LIBRARY=<project-library-for-that-R-version>
Rscript src/run_deseq2.R --request request.json --result output/result.json
```

A managed project should use a versioned location such as
`<project>/.linxira-bio/r/4.6.1/library`; selecting another interpreter selects
its corresponding library and resolved environment lock rather than replacing
the first environment.

The runner must invoke the executable selected by
`LINXIRA_BIO_WORKFLOW_R`; the script activates only the existing directory in
`LINXIRA_BIO_WORKFLOW_R_LIBRARY`. All declared analysis packages must resolve
from that project library. Every loaded non-base/non-recommended namespace is
checked again before results are committed, so a transitive dependency cannot
silently fall back to a global site library. The script never installs a
package, changes the global R library, or changes the system `PATH`.

`dependencies.lock.json` is the immutable pack compatibility and resolution
policy. It records the preferred stable runtime, tested version ranges,
repositories, direct requirements, and the fields required in a resolved
environment lock. It deliberately does not claim that a platform-specific
transitive graph has already been resolved. Before activation, the future
managed resolver must write an exact direct-and-transitive environment lock
with canonical sources, SHA-256 values, licenses, and the selected R runtime.
Successful results record every loaded namespace version for audit.

This is a raw-count workflow, not an analysis for TPM, FPKM, percentages, or
already normalized values. It requires at least two biological samples in each
condition. Batch effects, paired designs, interactions, shrinkage, independent
filter customization, and covariates are intentionally outside version 0.1.0.

The source is `cataloged`, not installable. A prepared and audited local
environment can execute it without network access; no data is uploaded.
