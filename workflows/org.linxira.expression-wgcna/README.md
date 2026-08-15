# org.linxira.expression-wgcna

Weighted gene co-expression network analysis (WGCNA) workflow pack serving
`expression.wgcna.v1`.

## Capability

`expression.wgcna.v1` infers signed or unsigned co-expression modules from a
rectangular expression matrix (genes x samples), selects a soft-thresholding
power from scale-free topology fit, and emits module assignments, module
eigengenes, a per-module summary, and the scale-free fit table.

Research-use-only: results describe correlations and module structure and are
not clinical interpretation.

## Runtime

- R `>=4.6.1,<4.7.0` (4.6.1 preferred) with Bioconductor 3.23
- Direct requirements: `WGCNA`, `jsonlite`, `digest` — installed separately
  into a project-isolated library identified by
  `LINXIRA_BIO_WORKFLOW_R_LIBRARY`; the interpreter is selected by
  `LINXIRA_BIO_WORKFLOW_R`.
- See `dependencies.lock.json` for the resolved transitive lock and
  `NOTICE.md` for source and license signals.

## Input

One expression artifact (CSV or TSV, genes x samples, first column the gene
identifier) plus parameters:

| Parameter | Default | Meaning |
| --- | --- | --- |
| `output_directory` | required | Where artifacts are written (must not exist) |
| `min_expression` | 1 | Minimum mean expression for a gene to be retained |
| `min_samples` | 3 | Minimum samples a gene must be expressed in |
| `min_module_size` | 30 | Minimum genes per module |
| `merge_cut_height` | 0.25 | Module merge cut height |
| `network_type` | `signed` | `unsigned`, `signed`, or `signed hybrid` |
| `power` | 0 | Soft threshold; 0 selects from scale-free fit |
| `log_transform` | `true` | Log2-transform the matrix before network construction |
| `threads` | 1 | Parallel worker threads |

## Output

Four CSV tables: `module-assignments.csv`, `module-eigengenes.csv`,
`module-summary.csv`, and `scale-free-fit.csv`, plus a result envelope with
module counts and the effective parameters.

## Validation

`Rscript tests/test_validation.R` exercises the request layer without needing
WGCNA installed. Real execution requires the project-isolated library above.
