//! Deterministic dose-response and enzyme-kinetics curve fitting
//! (`curve.fit.v1`).
//!
//! Fits a two-column CSV/TSV table (x = concentration or substrate, y =
//! response or rate) with one of three models:
//!
//! - four-parameter logistic (`4pl`) for IC50 and ELISA standard curves,
//! - Michaelis-Menten (`michaelis-menten`) for substrate-saturation kinetics,
//! - Lineweaver-Burk (`lineweaver-burk`) as a closed-form double-reciprocal
//!   linearization.
//!
//! The nonlinear models use a fixed-iteration Gauss-Newton solver with step
//! damping; no external fitting dependency is involved and the output is fully
//! deterministic for a given input.

use csv::{ReaderBuilder, Trim};
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

const CURVE_FIT_MIN_NONLINEAR_POINTS: usize = 4;
const CURVE_FIT_MIN_LINEAR_POINTS: usize = 2;
const CURVE_FIT_MAX_DAMPING_HALVINGS: usize = 40;
const CURVE_FIT_MAX_TABLE_ROWS: usize = 1_000_000;
const CURVE_FIT_PIVOT_FLOOR: f64 = 1e-12;

const FOUR_PARAMETER_LOGISTIC_PARAMETER_NAMES: &[&str] = &["a", "b", "c_ic50", "d"];
const MICHAELIS_MENTEN_PARAMETER_NAMES: &[&str] = &["vmax", "km"];
const LINEWEAVER_BURK_PARAMETER_NAMES: &[&str] = &["slope", "intercept", "vmax", "km"];

/// Estimated parameter values with their approximate standard errors.
struct CurveFitEstimates {
    values: Vec<f64>,
    standard_errors: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CurveFitModel {
    FourParameterLogistic,
    MichaelisMenten,
    LineweaverBurk,
}

impl CurveFitModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FourParameterLogistic => "four-parameter-logistic",
            Self::MichaelisMenten => "michaelis-menten",
            Self::LineweaverBurk => "lineweaver-burk",
        }
    }
}

/// Parses CLI model names for `linxira-bio curve fit --model`.
pub fn parse_curve_fit_model(value: &str) -> Result<CurveFitModel, CurveFitError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "4pl" | "four-parameter-logistic" | "four_parameter_logistic" => {
            Ok(CurveFitModel::FourParameterLogistic)
        }
        "michaelis-menten" | "michaelis_menten" | "mm" => Ok(CurveFitModel::MichaelisMenten),
        "lineweaver-burk" | "lineweaver_burk" | "lb" => Ok(CurveFitModel::LineweaverBurk),
        _ => Err(CurveFitError::InvalidOptions(format!(
            "unsupported curve fit model {value:?}; expected 4pl, michaelis-menten, or lineweaver-burk"
        ))),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CurveFitOptions {
    pub max_iterations: usize,
    pub tolerance: f64,
}

impl Default for CurveFitOptions {
    fn default() -> Self {
        Self {
            max_iterations: 200,
            tolerance: 1e-10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurveFitParameter {
    pub name: &'static str,
    pub value: f64,
    pub standard_error: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurveFitPoint {
    pub x: f64,
    pub y: f64,
    pub fit: f64,
    pub residual: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CurveFitResult {
    pub model: CurveFitModel,
    pub point_count: u64,
    pub parameters: Vec<CurveFitParameter>,
    pub r_squared: f64,
    pub rmse: f64,
    pub residuals: Vec<CurveFitPoint>,
    pub iterations: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum CurveFitError {
    Io(io::Error),
    Csv(csv::Error),
    InvalidHeader(String),
    InvalidRecord {
        record: u64,
        message: String,
    },
    InvalidOptions(String),
    InvalidData(String),
    NotEnoughPoints {
        model: &'static str,
        required: usize,
        available: usize,
    },
    Fit(String),
}

impl Display for CurveFitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "curve fit I/O failed: {error}"),
            Self::Csv(error) => write!(formatter, "invalid delimited curve table: {error}"),
            Self::InvalidHeader(message) => {
                write!(formatter, "invalid curve table header: {message}")
            }
            Self::InvalidRecord { record, message } => {
                write!(formatter, "invalid curve table record {record}: {message}")
            }
            Self::InvalidOptions(message) => {
                write!(formatter, "invalid curve fit options: {message}")
            }
            Self::InvalidData(message) => write!(formatter, "invalid curve fit input: {message}"),
            Self::NotEnoughPoints {
                model,
                required,
                available,
            } => write!(
                formatter,
                "curve fit ({model}) requires at least {required} valid points, found {available}"
            ),
            Self::Fit(message) => write!(formatter, "curve fit failed: {message}"),
        }
    }
}

impl Error for CurveFitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            Self::InvalidHeader(_)
            | Self::InvalidRecord { .. }
            | Self::InvalidOptions(_)
            | Self::InvalidData(_)
            | Self::NotEnoughPoints { .. }
            | Self::Fit(_) => None,
        }
    }
}

impl From<io::Error> for CurveFitError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for CurveFitError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

/// Fits a two-column CSV/TSV table (optionally gzipped) with the requested
/// model and returns the parameter estimates with uncertainty and residuals.
pub fn curve_fit_path(
    input_path: impl AsRef<Path>,
    model: CurveFitModel,
    options: &CurveFitOptions,
) -> Result<CurveFitResult, CurveFitError> {
    validate_curve_fit_options(options)?;
    let input = open_curve_input(input_path.as_ref())?;
    let points = read_curve_table(BufReader::new(input))?;
    curve_fit_points(&points, model, options)
}

fn validate_curve_fit_options(options: &CurveFitOptions) -> Result<(), CurveFitError> {
    if options.max_iterations == 0 {
        return Err(CurveFitError::InvalidOptions(
            "max_iterations must be at least 1".to_owned(),
        ));
    }
    if !options.tolerance.is_finite() || options.tolerance <= 0.0 {
        return Err(CurveFitError::InvalidOptions(
            "tolerance must be a positive finite number".to_owned(),
        ));
    }
    Ok(())
}

fn curve_fit_points(
    points: &[(f64, f64)],
    model: CurveFitModel,
    options: &CurveFitOptions,
) -> Result<CurveFitResult, CurveFitError> {
    let warnings = Vec::new();
    match model {
        CurveFitModel::FourParameterLogistic => {
            fit_four_parameter_logistic(points, options, warnings)
        }
        CurveFitModel::MichaelisMenten => fit_michaelis_menten(points, options, warnings),
        CurveFitModel::LineweaverBurk => fit_lineweaver_burk(points, warnings),
    }
}

fn fit_four_parameter_logistic(
    points: &[(f64, f64)],
    options: &CurveFitOptions,
    mut warnings: Vec<String>,
) -> Result<CurveFitResult, CurveFitError> {
    if points.len() < CURVE_FIT_MIN_NONLINEAR_POINTS {
        return Err(CurveFitError::NotEnoughPoints {
            model: CurveFitModel::FourParameterLogistic.as_str(),
            required: CURVE_FIT_MIN_NONLINEAR_POINTS,
            available: points.len(),
        });
    }
    for (index, &(x, _)) in points.iter().enumerate() {
        if x <= 0.0 {
            return Err(CurveFitError::InvalidData(format!(
                "four-parameter logistic requires positive concentrations; point {} has x = {x}",
                index + 1
            )));
        }
    }

    let mut minimum_y = f64::INFINITY;
    let mut maximum_y = f64::NEG_INFINITY;
    let mut log_concentration_sum = 0.0;
    for &(x, y) in points {
        minimum_y = minimum_y.min(y);
        maximum_y = maximum_y.max(y);
        log_concentration_sum += x.ln();
    }
    let geometric_mean_concentration = (log_concentration_sum / points.len() as f64).exp();
    // a = response at zero concentration, b = slope factor, c = IC50,
    // d = response at saturating concentration.
    let initial = vec![minimum_y, 1.0, geometric_mean_concentration, maximum_y];

    let is_valid = |parameters: &[f64]| {
        parameters.iter().all(|value| value.is_finite()) && parameters[2] > 0.0
    };
    let (parameters, iterations, _converged, solver_warnings) = run_gauss_newton(
        points,
        &initial,
        options,
        four_parameter_logistic_value,
        four_parameter_logistic_jacobian,
        is_valid,
    );
    warnings.extend(solver_warnings);
    let residual_sum_of_squares =
        sum_of_squared_errors(points, &parameters, four_parameter_logistic_value);
    let standard_errors = parameter_standard_errors(
        points,
        &parameters,
        residual_sum_of_squares,
        four_parameter_logistic_jacobian,
        &mut warnings,
    );
    let estimates = CurveFitEstimates {
        values: parameters,
        standard_errors,
    };
    Ok(assemble_curve_fit_result(
        CurveFitModel::FourParameterLogistic,
        points,
        FOUR_PARAMETER_LOGISTIC_PARAMETER_NAMES,
        estimates,
        iterations,
        warnings,
        four_parameter_logistic_value,
    ))
}

/// y = d + (a - d) / (1 + (x / c)^b), evaluated through the numerically
/// stable logistic fraction q = 1 / (1 + exp(b * ln(x / c))).
fn four_parameter_logistic_value(x: f64, parameters: &[f64]) -> f64 {
    let a = parameters[0];
    let b = parameters[1];
    let c = parameters[2];
    let d = parameters[3];
    let lower_fraction = 1.0 / (1.0 + (b * (x / c).ln()).exp());
    d + (a - d) * lower_fraction
}

fn four_parameter_logistic_jacobian(x: f64, parameters: &[f64]) -> Vec<f64> {
    let a = parameters[0];
    let b = parameters[1];
    let c = parameters[2];
    let d = parameters[3];
    let ratio_log = (x / c).ln();
    let lower_fraction = 1.0 / (1.0 + (b * ratio_log).exp());
    let band = lower_fraction * (1.0 - lower_fraction);
    vec![
        lower_fraction,
        -(a - d) * band * ratio_log,
        (a - d) * band * b / c,
        1.0 - lower_fraction,
    ]
}

fn fit_michaelis_menten(
    points: &[(f64, f64)],
    options: &CurveFitOptions,
    mut warnings: Vec<String>,
) -> Result<CurveFitResult, CurveFitError> {
    if points.len() < CURVE_FIT_MIN_NONLINEAR_POINTS {
        return Err(CurveFitError::NotEnoughPoints {
            model: CurveFitModel::MichaelisMenten.as_str(),
            required: CURVE_FIT_MIN_NONLINEAR_POINTS,
            available: points.len(),
        });
    }
    for (index, &(x, _)) in points.iter().enumerate() {
        if x <= 0.0 {
            return Err(CurveFitError::InvalidData(format!(
                "michaelis-menten requires positive substrate concentrations; point {} has x = {x}",
                index + 1
            )));
        }
    }

    let mut maximum_rate = f64::NEG_INFINITY;
    for &(_, y) in points {
        maximum_rate = maximum_rate.max(y);
    }
    if maximum_rate <= 0.0 {
        return Err(CurveFitError::InvalidData(
            "michaelis-menten requires at least one positive rate".to_owned(),
        ));
    }
    let half_maximum = maximum_rate / 2.0;
    let mut half_maximum_substrate = points[0].0;
    let mut best_distance = f64::INFINITY;
    for &(x, y) in points {
        let distance = (y - half_maximum).abs();
        if distance < best_distance {
            best_distance = distance;
            half_maximum_substrate = x;
        }
    }
    let initial = vec![maximum_rate, half_maximum_substrate];

    let is_valid = |parameters: &[f64]| {
        parameters.iter().all(|value| value.is_finite())
            && parameters[0] > 0.0
            && parameters[1] > 0.0
    };
    let (parameters, iterations, _converged, solver_warnings) = run_gauss_newton(
        points,
        &initial,
        options,
        michaelis_menten_value,
        michaelis_menten_jacobian,
        is_valid,
    );
    warnings.extend(solver_warnings);
    let residual_sum_of_squares =
        sum_of_squared_errors(points, &parameters, michaelis_menten_value);
    let standard_errors = parameter_standard_errors(
        points,
        &parameters,
        residual_sum_of_squares,
        michaelis_menten_jacobian,
        &mut warnings,
    );
    let estimates = CurveFitEstimates {
        values: parameters,
        standard_errors,
    };
    Ok(assemble_curve_fit_result(
        CurveFitModel::MichaelisMenten,
        points,
        MICHAELIS_MENTEN_PARAMETER_NAMES,
        estimates,
        iterations,
        warnings,
        michaelis_menten_value,
    ))
}

/// v = Vmax * S / (Km + S).
fn michaelis_menten_value(x: f64, parameters: &[f64]) -> f64 {
    let vmax = parameters[0];
    let km = parameters[1];
    vmax * x / (km + x)
}

fn michaelis_menten_jacobian(x: f64, parameters: &[f64]) -> Vec<f64> {
    let vmax = parameters[0];
    let km = parameters[1];
    let denominator = km + x;
    vec![x / denominator, -vmax * x / (denominator * denominator)]
}

fn fit_lineweaver_burk(
    points: &[(f64, f64)],
    mut warnings: Vec<String>,
) -> Result<CurveFitResult, CurveFitError> {
    let mut used = Vec::with_capacity(points.len());
    for (index, &(x, y)) in points.iter().enumerate() {
        if x == 0.0 || y == 0.0 {
            warnings.push(format!(
                "point {}: x = {x} or y = {y} is zero and was excluded from the lineweaver-burk fit",
                index + 1
            ));
            continue;
        }
        used.push((x, y));
    }
    if used.len() < CURVE_FIT_MIN_LINEAR_POINTS {
        return Err(CurveFitError::NotEnoughPoints {
            model: CurveFitModel::LineweaverBurk.as_str(),
            required: CURVE_FIT_MIN_LINEAR_POINTS,
            available: used.len(),
        });
    }

    let reciprocal_x: Vec<f64> = used.iter().map(|(x, _)| 1.0 / x).collect();
    let reciprocal_y: Vec<f64> = used.iter().map(|(_, y)| 1.0 / y).collect();
    let count = used.len() as f64;
    let mean_reciprocal_x = reciprocal_x.iter().sum::<f64>() / count;
    let mean_reciprocal_y = reciprocal_y.iter().sum::<f64>() / count;
    let mut centered_square_x = 0.0;
    let mut centered_product = 0.0;
    let mut centered_square_y = 0.0;
    for (u, w) in reciprocal_x.iter().zip(reciprocal_y.iter()) {
        centered_square_x += (u - mean_reciprocal_x) * (u - mean_reciprocal_x);
        centered_product += (u - mean_reciprocal_x) * (w - mean_reciprocal_y);
        centered_square_y += (w - mean_reciprocal_y) * (w - mean_reciprocal_y);
    }
    if centered_square_x == 0.0 {
        return Err(CurveFitError::Fit(
            "lineweaver-burk requires at least two distinct substrate concentrations".to_owned(),
        ));
    }
    let slope = centered_product / centered_square_x;
    let intercept = mean_reciprocal_y - slope * mean_reciprocal_x;
    if !slope.is_finite() || !intercept.is_finite() || intercept == 0.0 {
        return Err(CurveFitError::Fit(
            "lineweaver-burk intercept is zero; vmax is undefined".to_owned(),
        ));
    }
    let vmax = 1.0 / intercept;
    let km = slope / intercept;
    if !vmax.is_finite() || !km.is_finite() {
        return Err(CurveFitError::Fit(
            "lineweaver-burk produced non-finite kinetic parameters".to_owned(),
        ));
    }

    let mut residual_sum_of_squares = 0.0;
    for (u, w) in reciprocal_x.iter().zip(reciprocal_y.iter()) {
        let residual = w - (slope * u + intercept);
        residual_sum_of_squares += residual * residual;
    }
    let degrees_of_freedom = used.len().saturating_sub(2);
    let (slope_error, intercept_error) = if degrees_of_freedom > 0 {
        let variance = residual_sum_of_squares / degrees_of_freedom as f64;
        (
            Some((variance / centered_square_x).sqrt()),
            Some(
                (variance
                    * (1.0 / count + mean_reciprocal_x * mean_reciprocal_x / centered_square_x))
                    .sqrt(),
            ),
        )
    } else {
        warnings.push("lineweaver-burk standard errors require at least three points".to_owned());
        (None, None)
    };
    // Delta-method standard errors for the derived kinetic parameters.
    let vmax_error = intercept_error.map(|error| error / (intercept * intercept));
    let km_error = match (slope_error, intercept_error) {
        (Some(slope_error), Some(intercept_error)) => Some(
            ((slope_error / intercept) * (slope_error / intercept)
                + (slope * intercept_error / (intercept * intercept))
                    * (slope * intercept_error / (intercept * intercept)))
                .sqrt(),
        ),
        _ => None,
    };

    let linear_r_squared = if centered_square_y > 0.0 {
        1.0 - residual_sum_of_squares / centered_square_y
    } else {
        1.0
    };
    let mut result = assemble_curve_fit_result(
        CurveFitModel::LineweaverBurk,
        &used,
        LINEWEAVER_BURK_PARAMETER_NAMES,
        CurveFitEstimates {
            values: vec![slope, intercept, vmax, km],
            standard_errors: vec![slope_error, intercept_error, vmax_error, km_error],
        },
        0,
        warnings,
        |x, _| 1.0 / (slope / x + intercept),
    );
    // r_squared is reported for the linearized (1/S, 1/v) regression, which is
    // the conventional Lineweaver-Burk linearity diagnostic.
    result.r_squared = linear_r_squared;
    Ok(result)
}

fn sum_of_squared_errors<F>(points: &[(f64, f64)], parameters: &[f64], evaluate: F) -> f64
where
    F: Fn(f64, &[f64]) -> f64,
{
    let mut sum = 0.0;
    for &(x, y) in points {
        let residual = y - evaluate(x, parameters);
        sum += residual * residual;
    }
    sum
}

/// Fixed-iteration Gauss-Newton with step damping: each update is halved
/// until the sum of squared errors strictly improves and the candidate stays
/// in the valid parameter region.
fn run_gauss_newton<F, G, H>(
    points: &[(f64, f64)],
    initial: &[f64],
    options: &CurveFitOptions,
    evaluate: F,
    jacobian_row: G,
    is_valid: H,
) -> (Vec<f64>, u64, bool, Vec<String>)
where
    F: Fn(f64, &[f64]) -> f64,
    G: Fn(f64, &[f64]) -> Vec<f64>,
    H: Fn(&[f64]) -> bool,
{
    let parameter_count = initial.len();
    let mut parameters = initial.to_vec();
    let mut iterations = 0_u64;
    let mut converged = false;
    let mut warnings = Vec::new();
    let mut current_sse = sum_of_squared_errors(points, &parameters, &evaluate);

    for _ in 0..options.max_iterations {
        let mut normal = vec![vec![0.0; parameter_count]; parameter_count];
        let mut gradient = vec![0.0; parameter_count];
        for &(x, y) in points {
            let row = jacobian_row(x, &parameters);
            let residual = y - evaluate(x, &parameters);
            for i in 0..parameter_count {
                gradient[i] += row[i] * residual;
                for j in 0..parameter_count {
                    normal[i][j] += row[i] * row[j];
                }
            }
        }
        let step = match solve_linear_system(normal, gradient) {
            Some(step) => step,
            None => {
                warnings.push(
                    "gauss-newton normal equations became singular; stopping at the current parameters"
                        .to_owned(),
                );
                break;
            }
        };
        let raw_relative_step = step
            .iter()
            .zip(parameters.iter())
            .map(|(delta, current)| delta.abs() / current.abs().max(1.0))
            .fold(0.0_f64, f64::max);
        if raw_relative_step < options.tolerance {
            converged = true;
            break;
        }

        let mut lambda = 1.0;
        let mut accepted = false;
        for _ in 0..CURVE_FIT_MAX_DAMPING_HALVINGS {
            let candidate: Vec<f64> = parameters
                .iter()
                .zip(step.iter())
                .map(|(current, delta)| current + lambda * delta)
                .collect();
            if is_valid(&candidate) {
                let candidate_sse = sum_of_squared_errors(points, &candidate, &evaluate);
                if candidate_sse < current_sse {
                    let relative_step = step
                        .iter()
                        .zip(parameters.iter())
                        .map(|(delta, current)| (lambda * delta).abs() / current.abs().max(1.0))
                        .fold(0.0_f64, f64::max);
                    parameters = candidate;
                    current_sse = candidate_sse;
                    iterations += 1;
                    accepted = true;
                    if relative_step < options.tolerance {
                        converged = true;
                    }
                    break;
                }
            }
            lambda *= 0.5;
        }
        if !accepted {
            warnings.push(
                "damping reached its floor without improving the fit; stopping at the current parameters"
                    .to_owned(),
            );
            break;
        }
        if converged {
            break;
        }
    }
    if !converged && iterations == options.max_iterations as u64 {
        warnings.push(format!(
            "iteration limit {} reached before the {} convergence tolerance",
            options.max_iterations, options.tolerance
        ));
    }
    (parameters, iterations, converged, warnings)
}

/// Approximate parameter standard errors from the diagonal of the inverse
/// normal matrix scaled by the residual variance (Seber & Wild, 2003).
fn parameter_standard_errors<G>(
    points: &[(f64, f64)],
    parameters: &[f64],
    residual_sum_of_squares: f64,
    jacobian_row: G,
    warnings: &mut Vec<String>,
) -> Vec<Option<f64>>
where
    G: Fn(f64, &[f64]) -> Vec<f64>,
{
    let parameter_count = parameters.len();
    let degrees_of_freedom = points.len().saturating_sub(parameter_count);
    if degrees_of_freedom == 0 {
        warnings.push(
            "parameter standard errors are undefined without residual degrees of freedom"
                .to_owned(),
        );
        return vec![None; parameter_count];
    }
    let mut normal = vec![vec![0.0; parameter_count]; parameter_count];
    for &(x, _) in points {
        let row = jacobian_row(x, parameters);
        for i in 0..parameter_count {
            for j in 0..parameter_count {
                normal[i][j] += row[i] * row[j];
            }
        }
    }
    let Some(inverse) = invert_matrix(&normal) else {
        warnings.push(
            "parameter standard errors are unavailable because the normal matrix is singular"
                .to_owned(),
        );
        return vec![None; parameter_count];
    };
    let residual_variance = residual_sum_of_squares / degrees_of_freedom as f64;
    inverse
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let variance = row[i] * residual_variance;
            (variance >= 0.0).then(|| variance.sqrt())
        })
        .collect()
}

/// Subtracts `factor * rows[source]` from `rows[target]` without holding two
/// simultaneous borrows of the same row matrix.
fn subtract_scaled_row(rows: &mut [Vec<f64>], target: usize, source: usize, factor: f64) {
    debug_assert_ne!(target, source);
    if target < source {
        let (lower, upper) = rows.split_at_mut(source);
        for (target_cell, source_cell) in lower[target].iter_mut().zip(upper[0].iter()) {
            *target_cell -= factor * source_cell;
        }
    } else {
        let (lower, upper) = rows.split_at_mut(target);
        for (target_cell, source_cell) in upper[0].iter_mut().zip(lower[source].iter()) {
            *target_cell -= factor * source_cell;
        }
    }
}

/// Gaussian elimination with partial pivoting for small dense systems.
fn solve_linear_system(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Option<Vec<f64>> {
    let n = rhs.len();
    for col in 0..n {
        let mut pivot = col;
        for row in (col + 1)..n {
            if matrix[row][col].abs() > matrix[pivot][col].abs() {
                pivot = row;
            }
        }
        if matrix[pivot][col].abs() < CURVE_FIT_PIVOT_FLOOR {
            return None;
        }
        matrix.swap(col, pivot);
        rhs.swap(col, pivot);
        for row in (col + 1)..n {
            let factor = matrix[row][col] / matrix[col][col];
            if factor == 0.0 {
                continue;
            }
            subtract_scaled_row(&mut matrix, row, col, factor);
            rhs[row] -= factor * rhs[col];
        }
    }
    let mut solution = vec![0.0; n];
    for row in (0..n).rev() {
        let mut accumulator = rhs[row];
        for k in (row + 1)..n {
            accumulator -= matrix[row][k] * solution[k];
        }
        solution[row] = accumulator / matrix[row][row];
    }
    Some(solution)
}

/// Gauss-Jordan inversion with partial pivoting for small dense matrices.
fn invert_matrix(matrix: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = matrix.len();
    let mut augmented: Vec<Vec<f64>> = matrix
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut augmented_row = row.clone();
            augmented_row.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            augmented_row
        })
        .collect();
    for col in 0..n {
        let mut pivot = col;
        for row in (col + 1)..n {
            if augmented[row][col].abs() > augmented[pivot][col].abs() {
                pivot = row;
            }
        }
        if augmented[pivot][col].abs() < CURVE_FIT_PIVOT_FLOOR {
            return None;
        }
        augmented.swap(col, pivot);
        let divisor = augmented[col][col];
        for value in &mut augmented[col] {
            *value /= divisor;
        }
        for row in 0..n {
            if row == col {
                continue;
            }
            let factor = augmented[row][col];
            if factor == 0.0 {
                continue;
            }
            subtract_scaled_row(&mut augmented, row, col, factor);
        }
    }
    Some(augmented.into_iter().map(|row| row[n..].to_vec()).collect())
}

fn assemble_curve_fit_result<F>(
    model: CurveFitModel,
    points: &[(f64, f64)],
    parameter_names: &[&'static str],
    estimates: CurveFitEstimates,
    iterations: u64,
    mut warnings: Vec<String>,
    evaluate: F,
) -> CurveFitResult
where
    F: Fn(f64, &[f64]) -> f64,
{
    let CurveFitEstimates {
        values: parameters,
        standard_errors,
    } = estimates;
    let mut y_mean = 0.0;
    for &(_, y) in points {
        y_mean += y;
    }
    y_mean /= points.len().max(1) as f64;
    let mut residual_sum_of_squares = 0.0;
    let mut total_sum_of_squares = 0.0;
    let mut residuals = Vec::with_capacity(points.len());
    for &(x, y) in points {
        let fit = evaluate(x, &parameters);
        let residual = y - fit;
        residual_sum_of_squares += residual * residual;
        total_sum_of_squares += (y - y_mean) * (y - y_mean);
        residuals.push(CurveFitPoint {
            x,
            y,
            fit,
            residual,
        });
    }
    let point_count = points.len();
    let rmse = (residual_sum_of_squares / point_count.max(1) as f64).sqrt();
    let r_squared = if total_sum_of_squares > 0.0 {
        1.0 - residual_sum_of_squares / total_sum_of_squares
    } else {
        warnings.push(
            "y values have zero variance; r_squared is reported for a perfect constant fit"
                .to_owned(),
        );
        1.0
    };
    CurveFitResult {
        model,
        point_count: point_count as u64,
        parameters: parameter_names
            .iter()
            .zip(parameters.into_iter().zip(standard_errors))
            .map(|(name, (value, standard_error))| CurveFitParameter {
                name,
                value,
                standard_error,
            })
            .collect(),
        r_squared,
        rmse,
        residuals,
        iterations,
        warnings,
    }
}

fn open_curve_input(path: &Path) -> Result<Box<dyn Read>, CurveFitError> {
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Ok(Box::new(MultiGzDecoder::new(File::open(path)?)))
    } else {
        Ok(Box::new(File::open(path)?))
    }
}

fn read_curve_table(mut input: impl BufRead) -> Result<Vec<(f64, f64)>, CurveFitError> {
    let delimiter = infer_curve_delimiter(input.fill_buf()?)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .flexible(false)
        .trim(Trim::All)
        .from_reader(input);
    let headers = reader.headers()?.clone();
    if headers.len() < 2 {
        return Err(CurveFitError::InvalidHeader(
            "expected a header with at least two columns (x and y)".to_owned(),
        ));
    }
    let x_column = headers[0].to_owned();
    let y_column = headers[1].to_owned();
    let mut points = Vec::new();
    for (record_index, record) in reader.records().enumerate() {
        let record = record?;
        if record_index + 1 > CURVE_FIT_MAX_TABLE_ROWS {
            return Err(CurveFitError::InvalidData(format!(
                "curve table exceeds the local analysis limit of {CURVE_FIT_MAX_TABLE_ROWS} rows"
            )));
        }
        let row = record_index as u64 + 1;
        let x = parse_curve_cell(record.get(0).unwrap_or_default(), &x_column, row)?;
        let y = parse_curve_cell(record.get(1).unwrap_or_default(), &y_column, row)?;
        points.push((x, y));
    }
    Ok(points)
}

fn parse_curve_cell(value: &str, column: &str, row: u64) -> Result<f64, CurveFitError> {
    if is_curve_missing(value) {
        return Err(CurveFitError::InvalidRecord {
            record: row,
            message: format!(
                "column {column:?} contains a missing value; remove or impute the row before curve fitting"
            ),
        });
    }
    let parsed = value
        .parse::<f64>()
        .map_err(|_| CurveFitError::InvalidRecord {
            record: row,
            message: format!("column {column:?} contains non-numeric value {value:?}"),
        })?;
    if !parsed.is_finite() {
        return Err(CurveFitError::InvalidRecord {
            record: row,
            message: format!("column {column:?} contains a non-finite value"),
        });
    }
    Ok(parsed)
}

fn infer_curve_delimiter(buffer: &[u8]) -> Result<u8, CurveFitError> {
    let tab = probe_curve_delimiter(buffer, b'\t');
    let comma = probe_curve_delimiter(buffer, b',');
    match (tab, comma) {
        (None, None) => Err(CurveFitError::InvalidHeader(
            "could not detect CSV or TSV delimiter".to_owned(),
        )),
        (Some(_), None) => Ok(b'\t'),
        (None, Some(_)) => Ok(b','),
        (Some(tab), Some(comma)) if tab.is_better_than(comma) => Ok(b'\t'),
        (Some(_), Some(_)) => Ok(b','),
    }
}

#[derive(Debug, Clone, Copy)]
struct CurveDelimiterProbe {
    inconsistent_record_count: usize,
    consistent_record_count: usize,
    foreign_delimiter_field_count: usize,
    header_field_count: usize,
}

impl CurveDelimiterProbe {
    fn is_better_than(self, other: Self) -> bool {
        (
            self.inconsistent_record_count,
            usize::MAX - self.consistent_record_count,
            self.foreign_delimiter_field_count,
            usize::MAX - self.header_field_count,
        ) < (
            other.inconsistent_record_count,
            usize::MAX - other.consistent_record_count,
            other.foreign_delimiter_field_count,
            usize::MAX - other.header_field_count,
        )
    }
}

fn probe_curve_delimiter(buffer: &[u8], delimiter: u8) -> Option<CurveDelimiterProbe> {
    const PROBE_RECORD_LIMIT: usize = 8;

    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(buffer);
    let mut records = reader.byte_records();
    let header = records.next()?.ok()?;
    if header.len() < 2 {
        return None;
    }

    let foreign_delimiter = if delimiter == b'\t' { b',' } else { b'\t' };
    let mut probe = CurveDelimiterProbe {
        inconsistent_record_count: 0,
        consistent_record_count: 0,
        foreign_delimiter_field_count: header
            .iter()
            .filter(|field| field.contains(&foreign_delimiter))
            .count(),
        header_field_count: header.len(),
    };
    for record in records.take(PROBE_RECORD_LIMIT) {
        let record = record.ok()?;
        if record.len() == probe.header_field_count {
            probe.consistent_record_count += 1;
        } else {
            probe.inconsistent_record_count += 1;
        }
        probe.foreign_delimiter_field_count += record
            .iter()
            .filter(|field| field.contains(&foreign_delimiter))
            .count();
    }
    Some(probe)
}

fn is_curve_missing(value: &str) -> bool {
    value.is_empty()
        || value == "."
        || value.eq_ignore_ascii_case("na")
        || value.eq_ignore_ascii_case("nan")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "value {actual} is not within {tolerance} of {expected}"
        );
    }

    fn parameter_value(result: &CurveFitResult, name: &str) -> f64 {
        result
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap_or_else(|| panic!("missing parameter {name}"))
            .value
    }

    fn standard_error(result: &CurveFitResult, name: &str) -> Option<f64> {
        result
            .parameters
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap_or_else(|| panic!("missing parameter {name}"))
            .standard_error
    }

    fn write_temp_table(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "linxira-bio-curve-fit-{name}-{}.tmp",
            std::process::id()
        ));
        std::fs::write(&path, content).expect("write temporary curve table");
        path
    }

    #[test]
    fn four_parameter_logistic_recovers_known_parameters() {
        let concentrations = [0.1, 0.3, 1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0];
        let truth = [0.15, 1.35, 12.0, 2.8];
        let points: Vec<(f64, f64)> = concentrations
            .iter()
            .enumerate()
            .map(|(index, &x)| {
                let y = four_parameter_logistic_value(x, &truth);
                (x, y * (1.0 + 1e-9 * (index as f64 - 4.0)))
            })
            .collect();
        let result = curve_fit_points(
            &points,
            CurveFitModel::FourParameterLogistic,
            &CurveFitOptions::default(),
        )
        .expect("four-parameter logistic fit succeeds");
        assert_close(parameter_value(&result, "a"), 0.15, 1e-3);
        assert_close(parameter_value(&result, "b"), 1.35, 1e-3);
        assert_close(parameter_value(&result, "c_ic50"), 12.0, 1e-3);
        assert_close(parameter_value(&result, "d"), 2.8, 1e-3);
        assert!(result.r_squared > 0.999);
        assert_close(result.rmse, 0.0, 1e-3);
        assert!(standard_error(&result, "c_ic50").is_some());
        assert_eq!(result.point_count, 9);
        assert_eq!(result.residuals.len(), 9);
        assert!(result.iterations <= 200);
    }

    #[test]
    fn michaelis_menten_recovers_known_parameters() {
        let substrates = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
        let truth = [4.2, 1.7];
        let points: Vec<(f64, f64)> = substrates
            .iter()
            .enumerate()
            .map(|(index, &x)| {
                let y = michaelis_menten_value(x, &truth);
                (x, y * (1.0 + 1e-9 * (index as f64 - 3.0)))
            })
            .collect();
        let result = curve_fit_points(
            &points,
            CurveFitModel::MichaelisMenten,
            &CurveFitOptions::default(),
        )
        .expect("michaelis-menten fit succeeds");
        assert_close(parameter_value(&result, "vmax"), 4.2, 1e-3);
        assert_close(parameter_value(&result, "km"), 1.7, 1e-3);
        assert!(result.r_squared > 0.9999);
        assert!(standard_error(&result, "vmax").is_some());
        assert!(standard_error(&result, "km").is_some());
        assert_eq!(result.residuals.len(), 8);
    }

    #[test]
    fn lineweaver_burk_agrees_with_michaelis_menten_on_clean_data() {
        let substrates = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
        let truth = [4.2, 1.7];
        let points: Vec<(f64, f64)> = substrates
            .iter()
            .map(|&x| (x, michaelis_menten_value(x, &truth)))
            .collect();
        let michaelis_menten = curve_fit_points(
            &points,
            CurveFitModel::MichaelisMenten,
            &CurveFitOptions::default(),
        )
        .expect("michaelis-menten fit succeeds");
        let lineweaver_burk = curve_fit_points(
            &points,
            CurveFitModel::LineweaverBurk,
            &CurveFitOptions::default(),
        )
        .expect("lineweaver-burk fit succeeds");
        assert_close(parameter_value(&lineweaver_burk, "slope"), 1.7 / 4.2, 1e-6);
        assert_close(
            parameter_value(&lineweaver_burk, "intercept"),
            1.0 / 4.2,
            1e-6,
        );
        assert_close(parameter_value(&lineweaver_burk, "vmax"), 4.2, 1e-6);
        assert_close(parameter_value(&lineweaver_burk, "km"), 1.7, 1e-6);
        assert_close(
            parameter_value(&lineweaver_burk, "vmax"),
            parameter_value(&michaelis_menten, "vmax"),
            1e-5,
        );
        assert_close(
            parameter_value(&lineweaver_burk, "km"),
            parameter_value(&michaelis_menten, "km"),
            1e-5,
        );
        assert!(lineweaver_burk.r_squared > 0.9999);
        assert_eq!(lineweaver_burk.iterations, 0);
        assert!(standard_error(&lineweaver_burk, "vmax").is_some());
    }

    #[test]
    fn lineweaver_burk_excludes_zero_points_with_warning() {
        let substrates = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0];
        let truth = [4.2, 1.7];
        let mut points: Vec<(f64, f64)> = substrates
            .iter()
            .map(|&x| (x, michaelis_menten_value(x, &truth)))
            .collect();
        points.push((0.0, 5.0));
        points.push((8.0, 0.0));
        let result = curve_fit_points(
            &points,
            CurveFitModel::LineweaverBurk,
            &CurveFitOptions::default(),
        )
        .expect("lineweaver-burk fit succeeds");
        assert_eq!(result.point_count, 8);
        assert_eq!(result.residuals.len(), 8);
        assert_close(parameter_value(&result, "vmax"), 4.2, 1e-6);
        assert_close(parameter_value(&result, "km"), 1.7, 1e-6);
        assert!(
            result
                .warnings
                .iter()
                .filter(|warning| warning.contains("excluded"))
                .count()
                >= 2
        );
    }

    #[test]
    fn rejects_non_positive_concentrations() {
        let mut points: Vec<(f64, f64)> = (1..=4).map(|i| (i as f64, i as f64)).collect();
        points.push((0.0, 2.5));
        let error = curve_fit_points(
            &points,
            CurveFitModel::FourParameterLogistic,
            &CurveFitOptions::default(),
        )
        .expect_err("zero concentration must fail");
        assert!(matches!(error, CurveFitError::InvalidData(_)));

        let negative = vec![(-1.0, 1.0), (1.0, 2.0), (2.0, 3.0), (3.0, 4.0)];
        let error = curve_fit_points(
            &negative,
            CurveFitModel::MichaelisMenten,
            &CurveFitOptions::default(),
        )
        .expect_err("negative substrate must fail");
        assert!(matches!(error, CurveFitError::InvalidData(_)));
    }

    #[test]
    fn rejects_insufficient_points() {
        let points = vec![(1.0, 1.0), (2.0, 2.0), (4.0, 3.0)];
        for model in [
            CurveFitModel::FourParameterLogistic,
            CurveFitModel::MichaelisMenten,
        ] {
            match curve_fit_points(&points, model, &CurveFitOptions::default()) {
                Err(CurveFitError::NotEnoughPoints {
                    required: 4,
                    available: 3,
                    ..
                }) => {}
                other => panic!("expected NotEnoughPoints, got {other:?}"),
            }
        }
    }

    #[test]
    fn rejects_empty_table() {
        let path = write_temp_table("empty", "concentration,response\n");
        let error = curve_fit_path(
            &path,
            CurveFitModel::FourParameterLogistic,
            &CurveFitOptions::default(),
        )
        .expect_err("empty table must fail");
        assert!(matches!(
            error,
            CurveFitError::NotEnoughPoints {
                available: 0,
                required: 4,
                ..
            }
        ));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reads_tsv_with_arbitrary_headers() {
        let concentrations = [0.1, 0.3, 1.0, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0];
        let truth = [0.5, 1.1, 8.0, 3.2];
        let mut table = String::from("conc\tod\n");
        for &x in &concentrations {
            table.push_str(&format!(
                "{}\t{:.12}\n",
                x,
                four_parameter_logistic_value(x, &truth)
            ));
        }
        let path = write_temp_table("tsv", &table);
        let result = curve_fit_path(
            &path,
            CurveFitModel::FourParameterLogistic,
            &CurveFitOptions::default(),
        )
        .expect("tsv curve fit succeeds");
        assert_close(parameter_value(&result, "a"), 0.5, 1e-3);
        assert_close(parameter_value(&result, "c_ic50"), 8.0, 1e-3);
        assert_eq!(result.point_count, 9);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_non_finite_values() {
        let path = write_temp_table("nonfinite", "concentration,response\n1,2\n3,nan\n");
        let error = curve_fit_path(
            &path,
            CurveFitModel::FourParameterLogistic,
            &CurveFitOptions::default(),
        )
        .expect_err("non-finite value must fail");
        assert!(matches!(error, CurveFitError::InvalidRecord { .. }));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn parses_model_names() {
        assert_eq!(
            parse_curve_fit_model("4pl").unwrap(),
            CurveFitModel::FourParameterLogistic
        );
        assert_eq!(
            parse_curve_fit_model("michaelis-menten").unwrap(),
            CurveFitModel::MichaelisMenten
        );
        assert_eq!(
            parse_curve_fit_model("lineweaver-burk").unwrap(),
            CurveFitModel::LineweaverBurk
        );
        assert!(parse_curve_fit_model("5pl").is_err());
    }
}
