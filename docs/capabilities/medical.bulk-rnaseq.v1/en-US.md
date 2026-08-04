# Bulk RNA-seq Research Workflow

## Purpose

Run the validated local two-condition bulk RNA-seq differential-expression workflow for medical-omics research use only.

## Inputs

Provide a raw integer count matrix and sample metadata with exactly two research groups. Identifiers must be unique and consistent, and each group requires at least two biological samples. Do not include direct identifiers or unnecessary protected data.

## Parameters

Declare the output directory, feature and sample ID columns, condition column, reference level, and contrast level. Optional `alpha` and `min_total_count` control reporting and pre-fit filtering.

## Outputs

Returns differential-expression and normalized-count CSV tables plus a result envelope marked `research-use-only` and `clinical_use: false`. The medical entrypoint always emits a research-use-only diagnostic.

## Examples

```text
linxira-bio workflow run org.linxira.bulk-expression-deseq2 request.json output/result.json
```

Use `medical.bulk-rnaseq.v1` in the schema-v2 request.

## Interpretation

Interpret the statistical contrast as exploratory research evidence. Review cohort construction, confounding, replicate quality, effect size, adjusted p-value, and biological plausibility.

## Caveats

This capability does not diagnose disease, assign prognosis, recommend treatment, validate a biomarker, or provide clinical interpretation. It is not a medical device and must not be used for patient-level decisions.

## Runtime Dependencies

Uses the same stable R 4.6.x, project-isolated DESeq2 workflow as `expression.differential.v1`. It performs no package installation, global library mutation, network upload, or cloud execution.

## Citations

Cite DESeq2, the exact package version, the study design, and applicable cohort or assay sources.

## Troubleshooting

Run the environment audit, resolve the full R dependency graph into the selected project library, remove direct identifiers from inputs, and validate the study contrast before execution.
