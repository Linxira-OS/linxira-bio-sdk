# Survival Analysis

## Purpose

Fit a Cox proportional-hazards model on a cohort table with survival time and event columns plus a group column, and report hazard ratios with confidence intervals, p-values, and a Kaplan-Meier summary per group. Research-use-only.

## Inputs

A CSV or TSV cohort table with columns for survival time (numeric), event indicator (0/1), and the grouping variable.

## Parameters

- `--time-column <column>` (required): survival time column.
- `--event-column <column>` (required): event indicator column (0/1).
- `--group-column <column>` (required): grouping/covariate column.
- `--reference-level <level>` (required): reference group level for the hazard ratio.

## Outputs

`cox-results.csv` (term, coefficient, hazard ratio, standard error, statistic, p-value, 95% CI) and `km-summary.csv` (per-group n, events, median survival). JSON output reports the model terms, per-term rows, and per-group Kaplan-Meier summaries.

## Examples

```bash
linxira-bio medical survival cohort.csv results/ --time-column time --event-column event --group-column treatment --reference-level control --json
```

## Interpretation

The hazard ratio is the exponentiated coefficient relative to the reference level; a ratio above 1 indicates higher event hazard. The p-value tests the coefficient against zero. Median survival is the time at which half the group has experienced the event, when estimable.

## Caveats

Research-use-only; not clinical decision support. The model assumes proportional hazards; covariates beyond the single group column are not supported. No censoring-time adjustments beyond the standard Cox framework. Ties use the default efron approximation.

## Runtime Dependencies

R with the `survival`, `jsonlite`, and `digest` packages in the project-isolated library (see `dependencies.lock.json`; `Rscript scripts/bootstrap-survival-lib.R <library-dir>`).

## Citations

Therneau, T.M., & Grambsch, P.M. (2000). Modeling Survival Data: Extending the Cox Model. Springer.

## Troubleshooting

If the model fails to converge, check for a constant group column, a single event level, or non-numeric time values. Confirm the reference level appears in the group column.
