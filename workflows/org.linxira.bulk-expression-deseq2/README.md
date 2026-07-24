# DESeq2 bulk-expression workflow pack

This first-party pack performs a two-level bulk RNA-seq differential-expression
comparison with DESeq2. It accepts a raw integer count matrix plus sample
metadata as CSV or TSV, validates identifiers and replication, applies a
minimum total-count filter, fits `~ condition`, and writes:

- `differential-expression.csv`;
- `normalized-counts.csv`;
- artifact-aware `result.json` with every loaded R namespace version,
  input/output SHA-256, parameters, and summary counts.

All three files are built in a private sibling staging directory and activated
by a same-filesystem directory rename. The requested output directory must not
already exist, and `--result` must name `<output_directory>/result.json`.

Run inside an environment that satisfies the direct runtime constraints in
`dependencies.lock.json`:

```text
Rscript src/run_deseq2.R --request request.json --result output/result.json
```

The current dependency file is deliberately marked
`direct-dependencies-only`. It pins and checks R plus the packages invoked by
the wrapper, but it is not a complete transitive Bioconductor lock and cannot
be used to install the pack. Successful results record every loaded namespace
version for audit. A signed resolver must materialize and checksum the full
dependency graph before the catalog can promote this pack to installable.

This is a raw-count workflow, not an analysis for TPM, FPKM, percentages, or
already normalized values. It requires at least two biological samples in each
condition. Batch effects, paired designs, interactions, shrinkage, independent
filter customization, and covariates are intentionally outside version 0.1.0.

The source is `cataloged`, not installable or application-dispatchable. No data
is uploaded and execution requires no network access.
