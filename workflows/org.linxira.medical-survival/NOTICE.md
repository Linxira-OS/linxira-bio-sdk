# Linxira Bio Medical Survival Pack

Runtime dependencies are installed separately and are not vendored. The
declared R packages are pinned by `dependencies.lock.json` with source URLs
and SHA-256 hashes:

- survival 3.8-9 (CRAN)
- jsonlite 2.0.0 (CRAN)
- digest 0.6.39 (CRAN)

Distribution: AGPL-3.0-or-later, copyright Linxira OS.

This pack executes `src/run_survival.R` with the interpreter selected by the
worker (`LINXIRA_BIO_WORKFLOW_R`) against the project-isolated R library
(`LINXIRA_BIO_WORKFLOW_R_LIBRARY`); it never mutates the global R library.
The script fits a Cox proportional-hazards model for a research-use-only
cohort table and writes cox-results.csv and km-summary.csv plus a versioned
result envelope.
