# Bulk Differential Expression

## Purpose

Fit a local two-condition DESeq2 model to raw bulk RNA-seq counts and produce auditable differential-expression and normalized-count tables.

## Inputs

Provide one CSV or TSV raw integer count matrix and one CSV or TSV sample-metadata table. Feature identifiers must be unique, sample identifiers must match the count columns exactly, and each of the two condition levels requires at least two biological samples.

## Parameters

Required fields are `output_directory`, `feature_id_column`, `sample_id_column`, `condition_column`, `reference_level`, and `contrast_level`. Optional `alpha` defaults to `0.05`; optional `min_total_count` defaults to `10`.

## Outputs

The atomic output directory contains `differential-expression.csv`, `normalized-counts.csv`, and `result.json`. The result records hashes, effective parameters, R and package versions, filtering counts, and the dependency-lock hash.

## Examples

```text
linxira-bio workflow run org.linxira.bulk-expression-deseq2 request.json output/result.json
```

Use `expression.differential.v1` in the schema-v2 request.

## Interpretation

Interpret log2 fold change together with adjusted p-value, expression strength, replicate quality, and the declared contrast. Multiple-testing significance does not establish biological importance or causality.

## Caveats

This version supports exactly two conditions with the design `~ condition`. It does not accept TPM, FPKM, percentages, normalized counts, batch terms, paired designs, interactions, or covariates.

## Runtime Dependencies

Requires a tested stable R 4.6.x interpreter and an existing project-isolated package library containing compatible DESeq2, jsonlite, digest, and their resolved dependencies. Select them with `LINXIRA_BIO_WORKFLOW_R` and `LINXIRA_BIO_WORKFLOW_R_LIBRARY`; the workflow does not install packages or modify global libraries.

## Citations

Cite Love MI, Huber W, Anders S. Moderated estimation of fold change and dispersion for RNA-seq data with DESeq2. Genome Biology 15, 550 (2014), plus the exact package version.

## Troubleshooting

Run the environment audit, confirm every declared package resolves from the project library, verify integer counts and matching sample identifiers, and ensure the requested output directory does not already exist.
