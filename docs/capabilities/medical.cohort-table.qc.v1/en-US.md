# Research Cohort Table QC

## Purpose

Summarize local research cohort-table structure, missingness, duplicate rows,
distinct values, and finite numeric ranges. This is research use only and does
not produce a diagnosis, risk score, treatment recommendation, or clinical decision.

## Inputs

One CSV or TSV cohort table with non-empty, unique column names.

## Parameters

The input path is required. Delimiter detection is automatic.

## Outputs

Row and column counts plus per-column missingness, distinct-value counts, and
numeric ranges.

## Interpretation

Missingness and duplicated records identify data-quality questions for review;
they do not establish participant status or outcome.

## Caveats

This capability does not validate clinical coding systems, infer diagnoses, or
replace data-governance review.

## Examples

```text
linxira-bio medical cohort-qc participants.tsv --json
```

## Runtime Dependencies

Local Rust only. Input data stays on the local machine.

## Citations

Cite the cohort, protocol, and data dictionary used for the analysis.

## Troubleshooting

Resolve duplicate headers and malformed CSV/TSV rows before running QC.
