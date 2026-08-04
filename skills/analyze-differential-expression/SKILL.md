---
name: analyze-differential-expression
description: Validate and run the implemented local bulk RNA-seq differential-expression workflow from raw integer count matrices and sample metadata. Use for two-condition DESeq2 comparisons through expression.differential.v1 or for explicitly research-use-only medical-omics analysis through medical.bulk-rnaseq.v1.
---

# Analyze Differential Expression

Use the bundled, versioned R workflow; do not rewrite the statistical method.

## Prepare

1. Inspect the raw count matrix and sample metadata locally.
2. Confirm feature identifiers and sample identifiers are unique and match exactly.
3. Require non-negative integer counts, exactly two condition levels, and at least two biological samples per level.
4. Select `expression.differential.v1` for general biology or `medical.bulk-rnaseq.v1` for research-use-only medical omics.
5. Run `linxira-bio environment audit --json`. Use the selected stable R interpreter and an existing project library through `LINXIRA_BIO_WORKFLOW_R` and `LINXIRA_BIO_WORKFLOW_R_LIBRARY`; never mutate the global R library.

## Run

Create a schema-v2 request with `counts` and `sample_metadata` input roles and these required parameters: `output_directory`, `feature_id_column`, `sample_id_column`, `condition_column`, `reference_level`, and `contrast_level`. Optional parameters are `alpha` and `min_total_count`.

```text
linxira-bio workflow run org.linxira.bulk-expression-deseq2 request.json output/result.json
```

Preserve the result envelope, both CSV artifacts, input hashes, R and package versions, dependency-lock hash, contrast, and effective parameters.

## Validate And Interpret

- Check filtering counts, normalized counts, effect direction, adjusted p-values, and missing estimates.
- Report the contrast as `contrast_level` relative to `reference_level`.
- Treat multiple-testing-adjusted significance and effect size as separate evidence.
- Do not use TPM, FPKM, percentages, or already normalized values as count-model input.
- Do not infer causality, diagnosis, prognosis, or treatment from this workflow. The medical capability is research use only.
- Cite DESeq2 and record the exact package version.
