# Medical Survival (R)

Research-use-only survival analysis: fits a Cox proportional-hazards model
(`survival::coxph`) on a cohort table with time and event columns plus a group
column, and reports hazard ratios with confidence intervals, p-values, and a
Kaplan-Meier summary per group. Outputs `cox-results.csv` and `km-summary.csv`
plus a versioned result envelope.

Requires the project-isolated R library from `dependencies.lock.json`
(`Rscript scripts/bootstrap-survival-lib.R <library-dir>`). Invoked through
the Linxira Bio worker; see `docs/capabilities/medical.survival.v1` for the
capability documentation.
