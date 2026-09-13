---
name: fit-dose-response-curves
description: Fit dose-response and enzyme-kinetics curves from a two-column CSV/TSV table with the local deterministic curve.fit.v1 capability (four-parameter logistic for IC50/ELISA standard curves, Michaelis-Menten, and Lineweaver-Burk), returning parameters with approximate standard errors, R-squared, RMSE, and per-point residuals. Use when an agent needs IC50/EC50 estimates, ELISA standard-curve fitting, or Vmax/Km kinetics without external fitting tools. Not for five-parameter logistic, weighted regression, or clinical dose decisions.
---

# Fit Dose-Response Curves

## Steps

1. Confirm the input is a two-column CSV/TSV table (first column x =
   concentration/substrate, second column y = response/rate; header names are
   arbitrary). Missing and non-finite cells must be removed or imputed first.
2. Choose the model and run one command:

```bash
linxira-bio curve fit dose-response.csv --model 4pl --json
linxira-bio curve fit kinetics.tsv --model michaelis-menten --json
linxira-bio curve fit kinetics.tsv --model lineweaver-burk --json
```

`--model 4pl` needs positive concentrations and >= 4 points; output parameters
are `a`, `b`, `c_ic50`, `d`. `--model michaelis-menten` needs positive
substrate and >= 4 points; output `vmax`/`km`. `--model lineweaver-burk`
excludes zero x or y points with warnings and reports `slope` (= Km/Vmax),
`intercept` (= 1/Vmax), plus derived `vmax`/`km`.
3. Check quality before reporting: `r_squared` and `rmse`, parameter standard
   errors, residual patterns in `residuals`, and any `warnings` (iteration
   limit, damping floor, excluded points). For Lineweaver-Burk, `r_squared`
   describes the linearized (1/S, 1/v) regression.

## Contract Notes

- Solver is a fixed-iteration (default 200) Gauss-Newton loop with step
  damping and a 1e-10 relative tolerance; `--max-iterations` and `--tolerance`
  override the defaults. Results are deterministic for a given input.
- Parameter standard errors are the local (J^T J) inverse approximation
  scaled by residual variance; treat them as asymptotic, not exact.
- Cross-check enzyme kinetics by fitting both `michaelis-menten` and
  `lineweaver-burk` on the same table; derived Vmax/Km should agree on clean
  data.
