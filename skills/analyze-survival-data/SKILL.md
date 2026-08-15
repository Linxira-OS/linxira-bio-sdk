---
name: analyze-survival-data
description: Fit research-only Cox proportional-hazards models on cohort tables with hazard ratios, confidence intervals, p-values, and Kaplan-Meier summaries per group.
---

# Analyze Survival Data

Inspect imported files before execution. Use the R workflow pack; do not
reimplement survival models in Python or Rust.

## Choose a capability

- Use `medical.survival.v1` with a CSV/TSV cohort table (time, event, group
  columns) to fit a Cox PH model and produce hazard-ratio and Kaplan-Meier
  summary tables. Research-use-only.

## Execute

```bash
linxira-bio medical survival COHORT.csv RESULTS/ --time-column time --event-column event --group-column treatment --reference-level control --json
```

## Interpret

Report the hazard ratio with its 95% confidence interval and p-value for each
model term, and per-group median survival when estimable. Check proportional
hazards assumptions before drawing conclusions. This is research-use-only;
never present results as clinical decision support. Keep clinical cohort data
local.

## Caveats

Requires the project-isolated R library (survival/jsonlite/digest). Only a
single group covariate is modeled; ties use the efron approximation.
