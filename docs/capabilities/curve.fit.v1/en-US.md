# curve.fit.v1

Fit dose-response and enzyme-kinetics curves from a two-column CSV/TSV table
(x = concentration or substrate, y = response or rate) with one deterministic
local model: the four-parameter logistic for IC50 and ELISA standard curves,
Michaelis-Menten for substrate-saturation kinetics, or the Lineweaver-Burk
double-reciprocal linearization. The solver is a fixed-iteration Gauss-Newton
loop with step damping; there is no external fitting dependency and the output
is reproducible for a given input.

## Purpose

Turn plate-reader and kinetics tables into reportable curve parameters —
IC50/EC50 with the 4PL asymptotes and slope factor, Vmax and Km from direct
Michaelis-Menten fitting, and the Lineweaver-Burk cross-check — with per-point
residuals and approximate parameter standard errors for quality control.

## Inputs

- One CSV or TSV table (optionally gzipped) with a header row and at least
  two columns; the first two columns are used as x and y regardless of their
  header names.
- Missing cells (`NA`, `nan`, `.`) and non-finite values are rejected; remove
  or impute such rows before fitting.

## Parameters

- `--model 4pl|michaelis-menten|lineweaver-burk` (default `4pl`).
- `--max-iterations N` (default 200): Gauss-Newton iteration cap for the
  nonlinear models.
- `--tolerance F` (default 1e-10): relative-step convergence threshold.
- `--json`: emit the standard result envelope.

## Outputs

- `model`: the fitted model name.
- `point_count`: number of points used in the fit.
- `parameters`: named estimates with approximate standard errors —
  `a`/`b`/`c_ic50`/`d` for 4PL (`c_ic50` is the inflection/IC50), `vmax`/`km`
  for Michaelis-Menten, and `slope` (= Km/Vmax)/`intercept` (= 1/Vmax) plus
  derived `vmax`/`km` for Lineweaver-Burk.
- `r_squared`, `rmse`: goodness of fit; for Lineweaver-Burk, `r_squared`
  describes the linearized (1/S, 1/v) regression while `rmse` is in original
  (S, v) units.
- `residuals`: per point `x`, `y`, `fit`, and `residual` (y - fit).
- `iterations`, `warnings`: solver iterations and data-quality notes.

## Examples

```bash
linxira-bio curve fit dose-response.csv --model 4pl --json
linxira-bio curve fit kinetics.tsv --model michaelis-menten --json
linxira-bio curve fit kinetics.tsv --model lineweaver-burk --json
```

## Interpretation

- 4PL: `a` is the response at zero concentration and `d` the saturated
  response; `c_ic50` is the concentration at the curve midpoint and should be
  bracketed by the measured concentration range; `b` is the slope factor
  (Hill-like steepness).
- Michaelis-Menten: `km` is reliable only when the substrate range spans
  below and above Km and approaches saturation; otherwise Vmax and Km are
  strongly correlated and their standard errors inflate.
- Lineweaver-Burk is a linearization cross-check: derived `vmax`/`km` should
  agree with the direct Michaelis-Menten fit on clean data; systematic
  disagreement indicates error structure that the reciprocal transform
  amplifies.
- Parameter standard errors come from the inverse (J^T J) normal matrix
  scaled by the residual variance — a local, asymptotic approximation.

## Caveats

- 4PL and Michaelis-Menten require positive x (concentration/substrate) and
  at least 4 valid points; Lineweaver-Burk requires at least 2 points with
  non-zero x and y, and points with a zero value are excluded with a warning.
- A single `y` column is expected; replicate wells should be pre-averaged or
  provided as additional rows (each row is one fit point).
- The Gauss-Newton solver uses fixed initial values (4PL: a = min y, d = max
  y, c = geometric mean x, b = 1; MM: Vmax = max v, Km = the substrate nearest
  half-maximum) and stops at the iteration cap with a warning rather than
  switching algorithms.
- Research use only; do not use for clinical dose decisions.

## Runtime Dependencies

- None beyond the Rust engine; no Python, R, or native tools are involved.

## Citations

- Seber, G. A. F. & Wild, C. J. (2003). Nonlinear Regression. Wiley.
- Michaelis, L. & Menten, M. L. (1913). Die Kinetik der Invertinwirkung.
  Biochemisches Zeitschrift, 49, 333–369.
- Lineweaver, H. & Burk, D. (1934). The determination of enzyme dissociation
  constants. Journal of the American Chemical Society, 56, 658–666.

## Troubleshooting

- "requires positive concentrations": zero or negative x values in a 4PL or
  Michaelis-Menten table; remove dilution rows at 0 or switch models.
- "requires at least 4 valid points": add dilution points or use
  `--model lineweaver-burk` (minimum 2 non-zero points).
- "lineweaver-burk intercept is zero": rates never approach saturation;
  fit `--model michaelis-menten` directly instead.
- A warning about the iteration limit or singular normal equations means the
  parameters are at a flat or degenerate region of the objective; widen the
  concentration range or check the data for outliers.
