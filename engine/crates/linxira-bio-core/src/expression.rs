use csv::{ReaderBuilder, Trim, WriterBuilder};
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

const MAX_NUMERIC_EXPRESSION_CELLS: usize = 10_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionSampleQc {
    pub sample: String,
    pub numeric_value_count: u64,
    pub missing_value_count: u64,
    pub detected_feature_count: u64,
    pub total: f64,
    pub mean: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionMatrixQc {
    pub delimiter: String,
    pub feature_id_column: String,
    pub feature_count: u64,
    pub sample_count: u64,
    pub total_value_count: u64,
    pub numeric_value_count: u64,
    pub missing_value_count: u64,
    pub zero_value_count: u64,
    pub negative_value_count: u64,
    pub zero_percent: Option<f64>,
    pub duplicate_feature_id_count: u64,
    pub samples: Vec<ExpressionSampleQc>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExpressionNormalizationMethod {
    Cpm,
    Log2Cpm,
    MedianRatio,
}

impl ExpressionNormalizationMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpm => "cpm",
            Self::Log2Cpm => "log2-cpm",
            Self::MedianRatio => "median-ratio",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionNormalizeOptions {
    pub method: ExpressionNormalizationMethod,
    pub pseudocount: f64,
}

impl Default for ExpressionNormalizeOptions {
    fn default() -> Self {
        Self {
            method: ExpressionNormalizationMethod::Cpm,
            pseudocount: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionNormalizationSample {
    pub sample: String,
    pub input_total: f64,
    pub scale_factor: f64,
    pub output_total: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionNormalizationSummary {
    pub method: String,
    pub feature_count: u64,
    pub sample_count: u64,
    pub pseudocount: Option<f64>,
    pub samples: Vec<ExpressionNormalizationSample>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionPcaOptions {
    pub components: usize,
    pub scale_features: bool,
}

impl Default for ExpressionPcaOptions {
    fn default() -> Self {
        Self {
            components: 2,
            scale_features: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionPcaLoading {
    pub feature: String,
    pub loading: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionPcaComponent {
    pub component: usize,
    pub eigenvalue: f64,
    pub explained_variance_percent: f64,
    pub top_positive_loadings: Vec<ExpressionPcaLoading>,
    pub top_negative_loadings: Vec<ExpressionPcaLoading>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionPcaSample {
    pub sample: String,
    pub scores: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionPcaResult {
    pub feature_count: u64,
    pub sample_count: u64,
    pub scaled_features: bool,
    pub total_variance: f64,
    pub components: Vec<ExpressionPcaComponent>,
    pub samples: Vec<ExpressionPcaSample>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionClusterOptions {
    pub sample_clusters: usize,
    pub feature_clusters: usize,
    pub max_iterations: usize,
    pub scale_features: bool,
}

impl Default for ExpressionClusterOptions {
    fn default() -> Self {
        Self {
            sample_clusters: 2,
            feature_clusters: 4,
            max_iterations: 100,
            scale_features: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionClusterAssignment {
    pub label: String,
    pub cluster: usize,
    pub distance_to_centroid: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionClusterAxisResult {
    pub requested_clusters: usize,
    pub populated_clusters: usize,
    pub iterations: usize,
    pub converged: bool,
    pub within_cluster_sum_squares: f64,
    pub cluster_sizes: Vec<u64>,
    pub assignments: Vec<ExpressionClusterAssignment>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionClusterResult {
    pub feature_count: u64,
    pub sample_count: u64,
    pub scaled_features: bool,
    pub samples: ExpressionClusterAxisResult,
    pub features: ExpressionClusterAxisResult,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExpressionHeatmapOptions {
    pub top_variable_features: usize,
    pub scale_rows: bool,
}

impl Default for ExpressionHeatmapOptions {
    fn default() -> Self {
        Self {
            top_variable_features: 50,
            scale_rows: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionHeatmapResult {
    pub input_feature_count: u64,
    pub selected_feature_count: u64,
    pub sample_count: u64,
    pub scaled_rows: bool,
    pub minimum_value: f64,
    pub maximum_value: f64,
    pub row_labels: Vec<String>,
    pub column_labels: Vec<String>,
    pub values: Vec<Vec<f64>>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum ExpressionMatrixError {
    Io(io::Error),
    Csv(csv::Error),
    InvalidHeader(String),
    InvalidRecord { record: u64, message: String },
    InvalidOptions(String),
    Analysis(String),
}

impl Display for ExpressionMatrixError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "expression matrix I/O failed: {error}"),
            Self::Csv(error) => write!(formatter, "invalid delimited expression matrix: {error}"),
            Self::InvalidHeader(message) => {
                write!(formatter, "invalid expression matrix header: {message}")
            }
            Self::InvalidRecord { record, message } => {
                write!(
                    formatter,
                    "invalid expression matrix record {record}: {message}"
                )
            }
            Self::InvalidOptions(message) => {
                write!(formatter, "invalid expression analysis options: {message}")
            }
            Self::Analysis(message) => write!(formatter, "expression analysis failed: {message}"),
        }
    }
}

impl Error for ExpressionMatrixError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            Self::InvalidHeader(_)
            | Self::InvalidRecord { .. }
            | Self::InvalidOptions(_)
            | Self::Analysis(_) => None,
        }
    }
}

impl From<io::Error> for ExpressionMatrixError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for ExpressionMatrixError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

pub fn expression_matrix_qc_path(
    path: impl AsRef<Path>,
) -> Result<ExpressionMatrixQc, ExpressionMatrixError> {
    let input = open_expression_input(path.as_ref())?;
    expression_matrix_qc(BufReader::new(input))
}

fn expression_matrix_qc(
    mut input: impl BufRead,
) -> Result<ExpressionMatrixQc, ExpressionMatrixError> {
    let delimiter = infer_delimiter(input.fill_buf()?)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .flexible(false)
        .trim(Trim::All)
        .from_reader(input);
    let headers = reader.headers()?.clone();
    if headers.len() < 2 {
        return Err(ExpressionMatrixError::InvalidHeader(
            "expected a feature identifier column and at least one sample".to_owned(),
        ));
    }
    if headers[0].is_empty() {
        return Err(ExpressionMatrixError::InvalidHeader(
            "feature identifier column name is empty".to_owned(),
        ));
    }

    let mut sample_names = BTreeSet::new();
    let mut samples = Vec::with_capacity(headers.len() - 1);
    for sample in headers.iter().skip(1) {
        if sample.is_empty() {
            return Err(ExpressionMatrixError::InvalidHeader(
                "sample names must not be empty".to_owned(),
            ));
        }
        if !sample_names.insert(sample.to_owned()) {
            return Err(ExpressionMatrixError::InvalidHeader(format!(
                "duplicate sample name {sample:?}"
            )));
        }
        samples.push(ExpressionSampleQc {
            sample: sample.to_owned(),
            numeric_value_count: 0,
            missing_value_count: 0,
            detected_feature_count: 0,
            total: 0.0,
            mean: None,
        });
    }

    let mut feature_ids = BTreeSet::new();
    let mut feature_count = 0_u64;
    let mut numeric_value_count = 0_u64;
    let mut missing_value_count = 0_u64;
    let mut zero_value_count = 0_u64;
    let mut negative_value_count = 0_u64;
    let mut duplicate_feature_id_count = 0_u64;

    for record in reader.records() {
        let record = record?;
        feature_count += 1;
        let feature_id = &record[0];
        if feature_id.is_empty() {
            return Err(ExpressionMatrixError::InvalidRecord {
                record: feature_count,
                message: "feature identifier is empty".to_owned(),
            });
        }
        if !feature_ids.insert(feature_id.to_owned()) {
            duplicate_feature_id_count += 1;
        }

        for (sample_index, value) in record.iter().skip(1).enumerate() {
            let sample = &mut samples[sample_index];
            if is_missing(value) {
                missing_value_count += 1;
                sample.missing_value_count += 1;
                continue;
            }
            let parsed =
                value
                    .parse::<f64>()
                    .map_err(|_| ExpressionMatrixError::InvalidRecord {
                        record: feature_count,
                        message: format!(
                            "sample {:?} contains non-numeric value {value:?}",
                            sample.sample
                        ),
                    })?;
            if !parsed.is_finite() {
                return Err(ExpressionMatrixError::InvalidRecord {
                    record: feature_count,
                    message: format!("sample {:?} contains non-finite value", sample.sample),
                });
            }
            sample.total += parsed;
            if !sample.total.is_finite() {
                return Err(ExpressionMatrixError::InvalidRecord {
                    record: feature_count,
                    message: format!("sample {:?} total exceeds supported range", sample.sample),
                });
            }
            sample.numeric_value_count += 1;
            numeric_value_count += 1;
            if parsed == 0.0 {
                zero_value_count += 1;
            } else {
                sample.detected_feature_count += 1;
            }
            if parsed < 0.0 {
                negative_value_count += 1;
            }
        }
    }

    for sample in &mut samples {
        sample.mean = (sample.numeric_value_count != 0)
            .then_some(sample.total / sample.numeric_value_count as f64);
    }
    let sample_count = u64::try_from(samples.len()).expect("sample count fits in u64");
    let total_value_count = feature_count
        .checked_mul(sample_count)
        .ok_or_else(|| ExpressionMatrixError::InvalidHeader("matrix is too large".to_owned()))?;
    let mut warnings = Vec::new();
    if feature_count == 0 {
        warnings.push("expression matrix contains no feature rows".to_owned());
    }
    if duplicate_feature_id_count != 0 {
        warnings.push(format!(
            "expression matrix contains {duplicate_feature_id_count} duplicate feature identifiers"
        ));
    }
    if negative_value_count != 0 {
        warnings.push(format!(
            "expression matrix contains {negative_value_count} negative values; verify that the matrix is transformed rather than raw counts"
        ));
    }

    Ok(ExpressionMatrixQc {
        delimiter: if delimiter == b'\t' { "tab" } else { "comma" }.to_owned(),
        feature_id_column: headers[0].to_owned(),
        feature_count,
        sample_count,
        total_value_count,
        numeric_value_count,
        missing_value_count,
        zero_value_count,
        negative_value_count,
        zero_percent: (numeric_value_count != 0)
            .then_some(zero_value_count as f64 / numeric_value_count as f64 * 100.0),
        duplicate_feature_id_count,
        samples,
        warnings,
    })
}

#[derive(Debug, Clone, PartialEq)]
struct NumericExpressionMatrix {
    feature_id_column: String,
    feature_ids: Vec<String>,
    sample_names: Vec<String>,
    values: Vec<Vec<f64>>,
}

pub fn parse_expression_normalization_method(
    value: &str,
) -> Result<ExpressionNormalizationMethod, ExpressionMatrixError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cpm" => Ok(ExpressionNormalizationMethod::Cpm),
        "log2-cpm" | "log2cpm" => Ok(ExpressionNormalizationMethod::Log2Cpm),
        "median-ratio" | "median_ratio" => Ok(ExpressionNormalizationMethod::MedianRatio),
        _ => Err(ExpressionMatrixError::InvalidOptions(format!(
            "unsupported normalization method {value:?}; expected cpm, log2-cpm, or median-ratio"
        ))),
    }
}

pub fn normalize_expression_matrix_path(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    options: &ExpressionNormalizeOptions,
) -> Result<ExpressionNormalizationSummary, ExpressionMatrixError> {
    if !options.pseudocount.is_finite() || options.pseudocount < 0.0 {
        return Err(ExpressionMatrixError::InvalidOptions(
            "pseudocount must be finite and non-negative".to_owned(),
        ));
    }
    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();
    if input_path == output_path {
        return Err(ExpressionMatrixError::InvalidOptions(
            "input and output paths must differ".to_owned(),
        ));
    }
    let matrix = read_numeric_expression_matrix_path(input_path)?;
    validate_nonnegative_matrix(&matrix)?;
    let feature_count = matrix.feature_ids.len();
    let sample_count = matrix.sample_names.len();
    let input_totals = column_totals(&matrix.values, sample_count);

    let scale_factors = match options.method {
        ExpressionNormalizationMethod::Cpm | ExpressionNormalizationMethod::Log2Cpm => input_totals
            .iter()
            .enumerate()
            .map(|(index, total)| {
                if *total <= 0.0 {
                    Err(ExpressionMatrixError::Analysis(format!(
                        "sample {:?} has zero library size",
                        matrix.sample_names[index]
                    )))
                } else {
                    Ok(1_000_000.0 / total)
                }
            })
            .collect::<Result<Vec<_>, _>>()?,
        ExpressionNormalizationMethod::MedianRatio => median_ratio_scale_factors(&matrix)?,
    };

    let mut writer = WriterBuilder::new()
        .delimiter(b'\t')
        .from_path(output_path)?;
    let mut header = Vec::with_capacity(sample_count + 1);
    header.push(matrix.feature_id_column.as_str());
    header.extend(matrix.sample_names.iter().map(String::as_str));
    writer.write_record(header)?;
    let mut output_totals = vec![0.0; sample_count];
    for (feature_id, row) in matrix.feature_ids.iter().zip(&matrix.values) {
        let mut record = Vec::with_capacity(sample_count + 1);
        record.push(feature_id.clone());
        for (sample_index, value) in row.iter().enumerate() {
            let scaled = value * scale_factors[sample_index];
            let normalized = if options.method == ExpressionNormalizationMethod::Log2Cpm {
                (scaled + options.pseudocount).log2()
            } else {
                scaled
            };
            output_totals[sample_index] += normalized;
            record.push(normalized.to_string());
        }
        writer.write_record(record)?;
    }
    writer.flush()?;
    let samples = matrix
        .sample_names
        .iter()
        .enumerate()
        .map(|(index, sample)| ExpressionNormalizationSample {
            sample: sample.clone(),
            input_total: input_totals[index],
            scale_factor: scale_factors[index],
            output_total: output_totals[index],
        })
        .collect();
    let mut warnings = Vec::new();
    if options.method == ExpressionNormalizationMethod::Cpm {
        warnings
            .push("CPM adjusts library size only and does not model composition bias".to_owned());
    } else if options.method == ExpressionNormalizationMethod::Log2Cpm {
        warnings.push(
            "log2-CPM is intended for exploration; retain raw counts for count-based models"
                .to_owned(),
        );
    } else {
        warnings.push(
            "median-ratio normalization requires enough features with positive counts in every sample"
                .to_owned(),
        );
    }

    Ok(ExpressionNormalizationSummary {
        method: options.method.as_str().to_owned(),
        feature_count: feature_count as u64,
        sample_count: sample_count as u64,
        pseudocount: (options.method == ExpressionNormalizationMethod::Log2Cpm)
            .then_some(options.pseudocount),
        samples,
        warnings,
    })
}

pub fn expression_pca_path(
    path: impl AsRef<Path>,
    options: &ExpressionPcaOptions,
) -> Result<ExpressionPcaResult, ExpressionMatrixError> {
    if options.components == 0 {
        return Err(ExpressionMatrixError::InvalidOptions(
            "PCA component count must be positive".to_owned(),
        ));
    }
    let matrix = read_numeric_expression_matrix_path(path.as_ref())?;
    if matrix.sample_names.len() < 2 {
        return Err(ExpressionMatrixError::Analysis(
            "PCA requires at least two samples".to_owned(),
        ));
    }
    let (rows, constant_features) = centered_rows(&matrix.values, options.scale_features);
    let denominator = matrix.sample_names.len() as f64 - 1.0;
    let total_variance = rows
        .iter()
        .flat_map(|row| row.iter())
        .map(|value| value * value)
        .sum::<f64>()
        / denominator;
    if total_variance <= f64::EPSILON {
        return Err(ExpressionMatrixError::Analysis(
            "PCA requires at least one feature with non-zero variance".to_owned(),
        ));
    }

    let component_limit = options
        .components
        .min(matrix.sample_names.len() - 1)
        .min(matrix.feature_ids.len());
    let eigenpairs = leading_sample_eigenpairs(&rows, component_limit, denominator);
    if eigenpairs.is_empty() {
        return Err(ExpressionMatrixError::Analysis(
            "PCA could not resolve a non-zero component".to_owned(),
        ));
    }

    let mut sample_scores = matrix
        .sample_names
        .iter()
        .map(|sample| ExpressionPcaSample {
            sample: sample.clone(),
            scores: Vec::with_capacity(eigenpairs.len()),
        })
        .collect::<Vec<_>>();
    let mut components = Vec::with_capacity(eigenpairs.len());
    for (component_index, (eigenvalue, vector)) in eigenpairs.iter().enumerate() {
        let singular_value = (eigenvalue * denominator).sqrt();
        for (sample, coordinate) in sample_scores.iter_mut().zip(vector) {
            sample.scores.push(coordinate * singular_value);
        }
        let mut loadings = matrix
            .feature_ids
            .iter()
            .zip(&rows)
            .map(|(feature, row)| ExpressionPcaLoading {
                feature: feature.clone(),
                loading: dot(row, vector) / singular_value,
            })
            .collect::<Vec<_>>();
        loadings.sort_by(|left, right| {
            right
                .loading
                .total_cmp(&left.loading)
                .then_with(|| left.feature.cmp(&right.feature))
        });
        let top_positive_loadings = loadings
            .iter()
            .filter(|loading| loading.loading > 0.0)
            .take(10)
            .cloned()
            .collect();
        let top_negative_loadings = loadings
            .iter()
            .rev()
            .filter(|loading| loading.loading < 0.0)
            .take(10)
            .cloned()
            .collect();
        components.push(ExpressionPcaComponent {
            component: component_index + 1,
            eigenvalue: *eigenvalue,
            explained_variance_percent: (eigenvalue / total_variance * 100.0).clamp(0.0, 100.0),
            top_positive_loadings,
            top_negative_loadings,
        });
    }

    let mut warnings = Vec::new();
    if constant_features != 0 {
        warnings.push(format!(
            "{constant_features} constant features contributed no PCA variance"
        ));
    }
    if component_limit < options.components {
        warnings.push(format!(
            "requested {} components but matrix rank permits at most {component_limit}",
            options.components
        ));
    }

    Ok(ExpressionPcaResult {
        feature_count: matrix.feature_ids.len() as u64,
        sample_count: matrix.sample_names.len() as u64,
        scaled_features: options.scale_features,
        total_variance,
        components,
        samples: sample_scores,
        warnings,
    })
}

pub fn expression_cluster_path(
    path: impl AsRef<Path>,
    options: &ExpressionClusterOptions,
) -> Result<ExpressionClusterResult, ExpressionMatrixError> {
    if options.sample_clusters == 0 || options.feature_clusters == 0 {
        return Err(ExpressionMatrixError::InvalidOptions(
            "sample and feature cluster counts must be positive".to_owned(),
        ));
    }
    if options.max_iterations == 0 || options.max_iterations > 10_000 {
        return Err(ExpressionMatrixError::InvalidOptions(
            "max_iterations must be between 1 and 10000".to_owned(),
        ));
    }
    let matrix = read_numeric_expression_matrix_path(path.as_ref())?;
    let feature_vectors = if options.scale_features {
        centered_rows(&matrix.values, true).0
    } else {
        matrix.values.clone()
    };
    let sample_vectors = transpose(&feature_vectors, matrix.sample_names.len());
    let sample_clusters = options.sample_clusters.min(matrix.sample_names.len());
    let feature_clusters = options.feature_clusters.min(matrix.feature_ids.len());
    let samples = deterministic_kmeans(
        &matrix.sample_names,
        &sample_vectors,
        sample_clusters,
        options.max_iterations,
    );
    let features = deterministic_kmeans(
        &matrix.feature_ids,
        &feature_vectors,
        feature_clusters,
        options.max_iterations,
    );
    let mut warnings = Vec::new();
    if sample_clusters < options.sample_clusters {
        warnings.push(format!(
            "sample cluster count was reduced from {} to {sample_clusters}",
            options.sample_clusters
        ));
    }
    if feature_clusters < options.feature_clusters {
        warnings.push(format!(
            "feature cluster count was reduced from {} to {feature_clusters}",
            options.feature_clusters
        ));
    }
    if !samples.converged || !features.converged {
        warnings.push("at least one k-means axis reached the iteration limit".to_owned());
    }

    Ok(ExpressionClusterResult {
        feature_count: matrix.feature_ids.len() as u64,
        sample_count: matrix.sample_names.len() as u64,
        scaled_features: options.scale_features,
        samples,
        features,
        warnings,
    })
}

pub fn expression_heatmap_path(
    path: impl AsRef<Path>,
    options: &ExpressionHeatmapOptions,
) -> Result<ExpressionHeatmapResult, ExpressionMatrixError> {
    if options.top_variable_features == 0 || options.top_variable_features > 200 {
        return Err(ExpressionMatrixError::InvalidOptions(
            "top_variable_features must be between 1 and 200".to_owned(),
        ));
    }
    let matrix = read_numeric_expression_matrix_path(path.as_ref())?;
    let mut ranked = matrix
        .values
        .iter()
        .enumerate()
        .map(|(index, row)| (index, variance(row)))
        .collect::<Vec<_>>();
    ranked.sort_by(
        |(left_index, left_variance), (right_index, right_variance)| {
            right_variance.total_cmp(left_variance).then_with(|| {
                matrix.feature_ids[*left_index].cmp(&matrix.feature_ids[*right_index])
            })
        },
    );
    ranked.truncate(options.top_variable_features.min(ranked.len()));
    let selected_indices = ranked.iter().map(|(index, _)| *index).collect::<Vec<_>>();
    let selected_labels = selected_indices
        .iter()
        .map(|index| matrix.feature_ids[*index].clone())
        .collect::<Vec<_>>();
    let selected_raw = selected_indices
        .iter()
        .map(|index| matrix.values[*index].clone())
        .collect::<Vec<_>>();
    let selected_values = if options.scale_rows {
        centered_rows(&selected_raw, true).0
    } else {
        selected_raw
    };

    let row_order = average_linkage_order(&selected_values);
    let mut warnings = Vec::new();
    let column_vectors = transpose(&selected_values, matrix.sample_names.len());
    let column_order = if column_vectors.len() <= 200 {
        average_linkage_order(&column_vectors)
    } else {
        warnings.push(
            "more than 200 samples were ordered by the first centered projection instead of hierarchical clustering"
                .to_owned(),
        );
        order_vectors_by_projection(&column_vectors)
    };
    let row_labels = row_order
        .iter()
        .map(|index| selected_labels[*index].clone())
        .collect::<Vec<_>>();
    let column_labels = column_order
        .iter()
        .map(|index| matrix.sample_names[*index].clone())
        .collect::<Vec<_>>();
    let values = row_order
        .iter()
        .map(|row_index| {
            column_order
                .iter()
                .map(|column_index| selected_values[*row_index][*column_index])
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let (minimum_value, maximum_value) = finite_range(&values).ok_or_else(|| {
        ExpressionMatrixError::Analysis("heatmap contains no finite values".to_owned())
    })?;
    if selected_indices.len() < matrix.feature_ids.len() {
        warnings.push(format!(
            "selected the {} most variable features from {} input features",
            selected_indices.len(),
            matrix.feature_ids.len()
        ));
    }

    Ok(ExpressionHeatmapResult {
        input_feature_count: matrix.feature_ids.len() as u64,
        selected_feature_count: selected_indices.len() as u64,
        sample_count: matrix.sample_names.len() as u64,
        scaled_rows: options.scale_rows,
        minimum_value,
        maximum_value,
        row_labels,
        column_labels,
        values,
        warnings,
    })
}

fn open_expression_input(path: &Path) -> Result<Box<dyn Read>, ExpressionMatrixError> {
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Ok(Box::new(MultiGzDecoder::new(File::open(path)?)))
    } else {
        Ok(Box::new(File::open(path)?))
    }
}

fn read_numeric_expression_matrix_path(
    path: &Path,
) -> Result<NumericExpressionMatrix, ExpressionMatrixError> {
    let input = open_expression_input(path)?;
    read_numeric_expression_matrix(BufReader::new(input))
}

fn read_numeric_expression_matrix(
    mut input: impl BufRead,
) -> Result<NumericExpressionMatrix, ExpressionMatrixError> {
    let delimiter = infer_delimiter(input.fill_buf()?)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(true)
        .flexible(false)
        .trim(Trim::All)
        .from_reader(input);
    let headers = reader.headers()?.clone();
    if headers.len() < 2 || headers[0].is_empty() {
        return Err(ExpressionMatrixError::InvalidHeader(
            "expected a named feature identifier column and at least one sample".to_owned(),
        ));
    }
    let mut unique_samples = BTreeSet::new();
    let mut sample_names = Vec::with_capacity(headers.len() - 1);
    for sample in headers.iter().skip(1) {
        if sample.is_empty() || !unique_samples.insert(sample.to_owned()) {
            return Err(ExpressionMatrixError::InvalidHeader(format!(
                "sample name {sample:?} is empty or duplicated"
            )));
        }
        sample_names.push(sample.to_owned());
    }

    let mut unique_features = BTreeSet::new();
    let mut feature_ids = Vec::new();
    let mut values = Vec::new();
    for (record_index, record) in reader.records().enumerate() {
        let record = record?;
        let cell_count = (record_index + 1)
            .checked_mul(sample_names.len())
            .ok_or_else(|| {
                ExpressionMatrixError::Analysis(
                    "expression matrix dimensions exceed the supported range".to_owned(),
                )
            })?;
        if cell_count > MAX_NUMERIC_EXPRESSION_CELLS {
            return Err(ExpressionMatrixError::Analysis(format!(
                "expression matrix exceeds the local analysis limit of {MAX_NUMERIC_EXPRESSION_CELLS} numeric cells"
            )));
        }
        let feature_id = record[0].trim();
        if feature_id.is_empty() || !unique_features.insert(feature_id.to_owned()) {
            return Err(ExpressionMatrixError::InvalidRecord {
                record: record_index as u64 + 1,
                message: format!("feature identifier {feature_id:?} is empty or duplicated"),
            });
        }
        let mut row = Vec::with_capacity(sample_names.len());
        for (sample_index, value) in record.iter().skip(1).enumerate() {
            if is_missing(value) {
                return Err(ExpressionMatrixError::InvalidRecord {
                    record: record_index as u64 + 1,
                    message: format!(
                        "sample {:?} contains a missing value; impute or filter it before this analysis",
                        sample_names[sample_index]
                    ),
                });
            }
            let parsed =
                value
                    .parse::<f64>()
                    .map_err(|_| ExpressionMatrixError::InvalidRecord {
                        record: record_index as u64 + 1,
                        message: format!(
                            "sample {:?} contains non-numeric value {value:?}",
                            sample_names[sample_index]
                        ),
                    })?;
            if !parsed.is_finite() {
                return Err(ExpressionMatrixError::InvalidRecord {
                    record: record_index as u64 + 1,
                    message: format!(
                        "sample {:?} contains a non-finite value",
                        sample_names[sample_index]
                    ),
                });
            }
            row.push(parsed);
        }
        feature_ids.push(feature_id.to_owned());
        values.push(row);
    }
    if values.is_empty() {
        return Err(ExpressionMatrixError::Analysis(
            "expression matrix contains no feature rows".to_owned(),
        ));
    }
    Ok(NumericExpressionMatrix {
        feature_id_column: headers[0].to_owned(),
        feature_ids,
        sample_names,
        values,
    })
}

fn validate_nonnegative_matrix(
    matrix: &NumericExpressionMatrix,
) -> Result<(), ExpressionMatrixError> {
    if let Some((row_index, column_index, value)) =
        matrix
            .values
            .iter()
            .enumerate()
            .find_map(|(row_index, row)| {
                row.iter()
                    .enumerate()
                    .find(|(_, value)| **value < 0.0)
                    .map(|(column_index, value)| (row_index, column_index, *value))
            })
    {
        return Err(ExpressionMatrixError::Analysis(format!(
            "normalization requires non-negative values; feature {:?}, sample {:?} contains {value}",
            matrix.feature_ids[row_index], matrix.sample_names[column_index]
        )));
    }
    Ok(())
}

fn column_totals(values: &[Vec<f64>], column_count: usize) -> Vec<f64> {
    let mut totals = vec![0.0; column_count];
    for row in values {
        for (total, value) in totals.iter_mut().zip(row) {
            *total += value;
        }
    }
    totals
}

fn median_ratio_scale_factors(
    matrix: &NumericExpressionMatrix,
) -> Result<Vec<f64>, ExpressionMatrixError> {
    let mut ratios = vec![Vec::new(); matrix.sample_names.len()];
    for row in &matrix.values {
        if row.iter().all(|value| *value > 0.0) {
            let geometric_mean =
                (row.iter().map(|value| value.ln()).sum::<f64>() / row.len() as f64).exp();
            for (sample_index, value) in row.iter().enumerate() {
                ratios[sample_index].push(value / geometric_mean);
            }
        }
    }
    ratios
        .into_iter()
        .enumerate()
        .map(|(sample_index, mut values)| {
            if values.is_empty() {
                return Err(ExpressionMatrixError::Analysis(
                    "median-ratio normalization found no feature with positive values in every sample"
                        .to_owned(),
                ));
            }
            values.sort_by(f64::total_cmp);
            let size_factor = median_sorted(&values);
            if !size_factor.is_finite() || size_factor <= 0.0 {
                return Err(ExpressionMatrixError::Analysis(format!(
                    "sample {:?} has an invalid median-ratio size factor",
                    matrix.sample_names[sample_index]
                )));
            }
            Ok(1.0 / size_factor)
        })
        .collect()
}

fn median_sorted(values: &[f64]) -> f64 {
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn centered_rows(values: &[Vec<f64>], scale: bool) -> (Vec<Vec<f64>>, usize) {
    let mut constant_count = 0;
    let rows = values
        .iter()
        .map(|row| {
            let mean = row.iter().sum::<f64>() / row.len() as f64;
            let mut centered = row.iter().map(|value| value - mean).collect::<Vec<_>>();
            let sum_squares = centered.iter().map(|value| value * value).sum::<f64>();
            if sum_squares <= f64::EPSILON {
                constant_count += 1;
                centered.fill(0.0);
            } else if scale {
                let denominator = (row.len().saturating_sub(1)).max(1) as f64;
                let standard_deviation = (sum_squares / denominator).sqrt();
                for value in &mut centered {
                    *value /= standard_deviation;
                }
            }
            centered
        })
        .collect();
    (rows, constant_count)
}

fn leading_sample_eigenpairs(
    rows: &[Vec<f64>],
    component_count: usize,
    denominator: f64,
) -> Vec<(f64, Vec<f64>)> {
    let sample_count = rows[0].len();
    let mut eigenvectors: Vec<Vec<f64>> = Vec::new();
    let mut eigenpairs = Vec::new();
    for component in 0..component_count {
        let mut vector = (0..sample_count)
            .map(|index| {
                let phase = ((index + 1) * (component + 2)) as f64;
                phase.sin() + (phase * 0.37).cos()
            })
            .collect::<Vec<_>>();
        orthogonalize(&mut vector, &eigenvectors);
        if normalize_vector(&mut vector) <= f64::EPSILON {
            let mut found = false;
            for basis in 0..sample_count {
                vector.fill(0.0);
                vector[basis] = 1.0;
                orthogonalize(&mut vector, &eigenvectors);
                if normalize_vector(&mut vector) > f64::EPSILON {
                    found = true;
                    break;
                }
            }
            if !found {
                break;
            }
        }

        for _ in 0..500 {
            let mut next = multiply_sample_gram(rows, &vector, denominator);
            orthogonalize(&mut next, &eigenvectors);
            if normalize_vector(&mut next) <= 1e-14 {
                break;
            }
            let alignment = dot(&vector, &next).abs();
            vector = next;
            if 1.0 - alignment < 1e-11 {
                break;
            }
        }
        let projected = multiply_sample_gram(rows, &vector, denominator);
        let eigenvalue = dot(&vector, &projected);
        if !eigenvalue.is_finite() || eigenvalue <= 1e-12 {
            break;
        }
        if vector
            .iter()
            .max_by(|left, right| left.abs().total_cmp(&right.abs()))
            .is_some_and(|value| *value < 0.0)
        {
            for value in &mut vector {
                *value = -*value;
            }
        }
        eigenvectors.push(vector.clone());
        eigenpairs.push((eigenvalue, vector));
    }
    eigenpairs
}

fn multiply_sample_gram(rows: &[Vec<f64>], vector: &[f64], denominator: f64) -> Vec<f64> {
    let mut result = vec![0.0; vector.len()];
    for row in rows {
        let projection = dot(row, vector) / denominator;
        for (result_value, row_value) in result.iter_mut().zip(row) {
            *result_value += row_value * projection;
        }
    }
    result
}

fn orthogonalize(vector: &mut [f64], basis: &[Vec<f64>]) {
    for direction in basis {
        let projection = dot(vector, direction);
        for (value, direction_value) in vector.iter_mut().zip(direction) {
            *value -= projection * direction_value;
        }
    }
}

fn normalize_vector(vector: &mut [f64]) -> f64 {
    let norm = dot(vector, vector).sqrt();
    if norm > f64::EPSILON {
        for value in vector {
            *value /= norm;
        }
    }
    norm
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn transpose(rows: &[Vec<f64>], column_count: usize) -> Vec<Vec<f64>> {
    let mut columns = vec![Vec::with_capacity(rows.len()); column_count];
    for row in rows {
        for (column, value) in columns.iter_mut().zip(row) {
            column.push(*value);
        }
    }
    columns
}

fn deterministic_kmeans(
    labels: &[String],
    vectors: &[Vec<f64>],
    cluster_count: usize,
    max_iterations: usize,
) -> ExpressionClusterAxisResult {
    let mut centroids = Vec::with_capacity(cluster_count);
    let first = vectors
        .iter()
        .enumerate()
        .max_by(|(left_index, left), (right_index, right)| {
            dot(left, left)
                .total_cmp(&dot(right, right))
                .then_with(|| right_index.cmp(left_index))
        })
        .map(|(index, _)| index)
        .unwrap_or(0);
    centroids.push(vectors[first].clone());
    while centroids.len() < cluster_count {
        let next = vectors
            .iter()
            .enumerate()
            .filter(|(_, vector)| !centroids.iter().any(|centroid| *centroid == **vector))
            .max_by(|(left_index, left), (right_index, right)| {
                minimum_squared_distance(left, &centroids)
                    .total_cmp(&minimum_squared_distance(right, &centroids))
                    .then_with(|| right_index.cmp(left_index))
            })
            .map(|(index, _)| index)
            .unwrap_or(centroids.len() % vectors.len());
        centroids.push(vectors[next].clone());
    }

    let mut assignments = vec![usize::MAX; vectors.len()];
    let mut converged = false;
    let mut iterations = 0;
    for iteration in 0..max_iterations {
        iterations = iteration + 1;
        let mut changed = false;
        for (assignment, vector) in assignments.iter_mut().zip(vectors) {
            let nearest = nearest_centroid(vector, &centroids);
            changed |= *assignment != nearest;
            *assignment = nearest;
        }
        let mut counts = vec![0_usize; cluster_count];
        for assignment in &assignments {
            counts[*assignment] += 1;
        }
        for empty_cluster in 0..cluster_count {
            if counts[empty_cluster] != 0 {
                continue;
            }
            if let Some((index, old_cluster)) = assignments
                .iter()
                .enumerate()
                .filter(|(_, cluster)| counts[**cluster] > 1)
                .max_by(|(left_index, left_cluster), (right_index, right_cluster)| {
                    squared_distance(&vectors[*left_index], &centroids[**left_cluster])
                        .total_cmp(&squared_distance(
                            &vectors[*right_index],
                            &centroids[**right_cluster],
                        ))
                        .then_with(|| right_index.cmp(left_index))
                })
                .map(|(index, cluster)| (index, *cluster))
            {
                assignments[index] = empty_cluster;
                counts[old_cluster] -= 1;
                counts[empty_cluster] += 1;
                changed = true;
            }
        }

        let dimension = vectors[0].len();
        let mut next_centroids = vec![vec![0.0; dimension]; cluster_count];
        for (vector, assignment) in vectors.iter().zip(&assignments) {
            for (sum, value) in next_centroids[*assignment].iter_mut().zip(vector) {
                *sum += value;
            }
        }
        for (cluster, centroid) in next_centroids.iter_mut().enumerate() {
            if counts[cluster] == 0 {
                *centroid = centroids[cluster].clone();
            } else {
                for value in centroid {
                    *value /= counts[cluster] as f64;
                }
            }
        }
        centroids = next_centroids;
        if !changed {
            converged = true;
            break;
        }
    }

    let mut cluster_sizes = vec![0_u64; cluster_count];
    let mut within_cluster_sum_squares = 0.0;
    let result_assignments = labels
        .iter()
        .zip(vectors)
        .zip(assignments)
        .map(|((label, vector), cluster)| {
            let squared = squared_distance(vector, &centroids[cluster]);
            cluster_sizes[cluster] += 1;
            within_cluster_sum_squares += squared;
            ExpressionClusterAssignment {
                label: label.clone(),
                cluster: cluster + 1,
                distance_to_centroid: squared.sqrt(),
            }
        })
        .collect::<Vec<_>>();
    let populated_clusters = cluster_sizes.iter().filter(|size| **size != 0).count();
    ExpressionClusterAxisResult {
        requested_clusters: cluster_count,
        populated_clusters,
        iterations,
        converged,
        within_cluster_sum_squares,
        cluster_sizes,
        assignments: result_assignments,
    }
}

fn nearest_centroid(vector: &[f64], centroids: &[Vec<f64>]) -> usize {
    centroids
        .iter()
        .enumerate()
        .min_by(|(left_index, left), (right_index, right)| {
            squared_distance(vector, left)
                .total_cmp(&squared_distance(vector, right))
                .then_with(|| left_index.cmp(right_index))
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn minimum_squared_distance(vector: &[f64], centroids: &[Vec<f64>]) -> f64 {
    centroids
        .iter()
        .map(|centroid| squared_distance(vector, centroid))
        .min_by(f64::total_cmp)
        .unwrap_or(0.0)
}

fn squared_distance(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| {
            let difference = left - right;
            difference * difference
        })
        .sum()
}

fn variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values
        .iter()
        .map(|value| {
            let difference = value - mean;
            difference * difference
        })
        .sum::<f64>()
        / (values.len() as f64 - 1.0)
}

fn correlation_distance(left: &[f64], right: &[f64]) -> f64 {
    let left_mean = left.iter().sum::<f64>() / left.len() as f64;
    let right_mean = right.iter().sum::<f64>() / right.len() as f64;
    let mut covariance = 0.0;
    let mut left_sum_squares = 0.0;
    let mut right_sum_squares = 0.0;
    for (left, right) in left.iter().zip(right) {
        let left_centered = left - left_mean;
        let right_centered = right - right_mean;
        covariance += left_centered * right_centered;
        left_sum_squares += left_centered * left_centered;
        right_sum_squares += right_centered * right_centered;
    }
    let denominator = (left_sum_squares * right_sum_squares).sqrt();
    if denominator <= f64::EPSILON {
        squared_distance(left, right).sqrt()
    } else {
        (1.0 - covariance / denominator).clamp(0.0, 2.0)
    }
}

#[derive(Clone)]
struct HeatmapCluster {
    members: Vec<usize>,
    order: Vec<usize>,
}

fn average_linkage_order(vectors: &[Vec<f64>]) -> Vec<usize> {
    if vectors.len() <= 1 {
        return (0..vectors.len()).collect();
    }
    let mut distances = vec![vec![0.0; vectors.len()]; vectors.len()];
    for left in 0..vectors.len() {
        for right in (left + 1)..vectors.len() {
            let distance = correlation_distance(&vectors[left], &vectors[right]);
            distances[left][right] = distance;
            distances[right][left] = distance;
        }
    }
    let mut clusters = (0..vectors.len())
        .map(|index| HeatmapCluster {
            members: vec![index],
            order: vec![index],
        })
        .collect::<Vec<_>>();
    while clusters.len() > 1 {
        let mut best = (0_usize, 1_usize, f64::INFINITY);
        for left in 0..clusters.len() {
            for right in (left + 1)..clusters.len() {
                let distance = average_cluster_distance(
                    &clusters[left].members,
                    &clusters[right].members,
                    &distances,
                );
                if distance < best.2 - 1e-12
                    || ((distance - best.2).abs() <= 1e-12 && (left, right) < (best.0, best.1))
                {
                    best = (left, right, distance);
                }
            }
        }
        let right = clusters.remove(best.1);
        let left = clusters.remove(best.0);
        let order = orient_and_join_orders(&left.order, &right.order, &distances);
        let mut members = left.members;
        members.extend(right.members);
        clusters.push(HeatmapCluster { members, order });
    }
    clusters
        .pop()
        .map(|cluster| cluster.order)
        .unwrap_or_default()
}

fn average_cluster_distance(left: &[usize], right: &[usize], distances: &[Vec<f64>]) -> f64 {
    let mut total = 0.0;
    for left_index in left {
        for right_index in right {
            total += distances[*left_index][*right_index];
        }
    }
    total / (left.len() * right.len()) as f64
}

fn orient_and_join_orders(left: &[usize], right: &[usize], distances: &[Vec<f64>]) -> Vec<usize> {
    let mut candidates = Vec::with_capacity(4);
    for reverse_left in [false, true] {
        for reverse_right in [false, true] {
            let mut left_order = left.to_vec();
            let mut right_order = right.to_vec();
            if reverse_left {
                left_order.reverse();
            }
            if reverse_right {
                right_order.reverse();
            }
            let boundary =
                distances[*left_order.last().expect("non-empty cluster")][right_order[0]];
            left_order.extend(right_order);
            candidates.push((boundary, left_order));
        }
    }
    candidates
        .into_iter()
        .min_by(
            |(left_distance, left_order), (right_distance, right_order)| {
                left_distance
                    .total_cmp(right_distance)
                    .then_with(|| left_order.cmp(right_order))
            },
        )
        .map(|(_, order)| order)
        .unwrap_or_default()
}

fn order_vectors_by_projection(vectors: &[Vec<f64>]) -> Vec<usize> {
    let direction = vectors
        .iter()
        .enumerate()
        .max_by(|(left_index, left), (right_index, right)| {
            variance(left)
                .total_cmp(&variance(right))
                .then_with(|| right_index.cmp(left_index))
        })
        .map(|(_, vector)| vector.clone())
        .unwrap_or_default();
    let mut order = (0..vectors.len()).collect::<Vec<_>>();
    order.sort_by(|left, right| {
        dot(&vectors[*left], &direction)
            .total_cmp(&dot(&vectors[*right], &direction))
            .then_with(|| left.cmp(right))
    });
    order
}

fn finite_range(values: &[Vec<f64>]) -> Option<(f64, f64)> {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for value in values.iter().flatten().filter(|value| value.is_finite()) {
        minimum = minimum.min(*value);
        maximum = maximum.max(*value);
    }
    minimum.is_finite().then_some((minimum, maximum))
}

fn infer_delimiter(buffer: &[u8]) -> Result<u8, ExpressionMatrixError> {
    let tab = probe_delimiter(buffer, b'\t');
    let comma = probe_delimiter(buffer, b',');
    match (tab, comma) {
        (None, None) => Err(ExpressionMatrixError::InvalidHeader(
            "could not detect CSV or TSV delimiter".to_owned(),
        )),
        (Some(_), None) => Ok(b'\t'),
        (None, Some(_)) => Ok(b','),
        (Some(tab), Some(comma)) if tab.is_better_than(comma) => Ok(b'\t'),
        (Some(_), Some(_)) => Ok(b','),
    }
}

#[derive(Debug, Clone, Copy)]
struct DelimiterProbe {
    inconsistent_record_count: usize,
    consistent_record_count: usize,
    foreign_delimiter_field_count: usize,
    header_field_count: usize,
}

impl DelimiterProbe {
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

fn probe_delimiter(buffer: &[u8], delimiter: u8) -> Option<DelimiterProbe> {
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
    let mut probe = DelimiterProbe {
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

fn is_missing(value: &str) -> bool {
    value.is_empty()
        || value == "."
        || value.eq_ignore_ascii_case("na")
        || value.eq_ignore_ascii_case("nan")
}

#[cfg(test)]
mod tests {
    use super::{
        ExpressionClusterOptions, ExpressionHeatmapOptions, ExpressionMatrixError,
        ExpressionNormalizationMethod, ExpressionNormalizeOptions, ExpressionPcaOptions,
        expression_cluster_path, expression_heatmap_path, expression_matrix_qc,
        expression_pca_path, normalize_expression_matrix_path,
        parse_expression_normalization_method,
    };
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "linxira-expression-{}-{}-{suffix}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn write_matrix(contents: &str) -> PathBuf {
        let path = temporary_path("matrix.tsv");
        fs::write(&path, contents).expect("write expression fixture");
        path
    }

    #[test]
    fn summarizes_tsv_expression_matrix() {
        let input = b"gene\ts1\ts2\nA\t1\t0\nB\t2.5\tNA\nA\t-1\t4\n";
        let qc = expression_matrix_qc(Cursor::new(input)).expect("valid matrix");

        assert_eq!(qc.delimiter, "tab");
        assert_eq!(qc.feature_count, 3);
        assert_eq!(qc.sample_count, 2);
        assert_eq!(qc.numeric_value_count, 5);
        assert_eq!(qc.missing_value_count, 1);
        assert_eq!(qc.zero_value_count, 1);
        assert_eq!(qc.negative_value_count, 1);
        assert_eq!(qc.duplicate_feature_id_count, 1);
        assert_eq!(qc.samples[0].total, 2.5);
        assert_eq!(qc.samples[1].detected_feature_count, 1);
        assert_eq!(qc.warnings.len(), 2);
    }

    #[test]
    fn rejects_duplicate_sample_names() {
        let error = expression_matrix_qc(Cursor::new(b"gene,s1,s1\nA,1,2\n"))
            .expect_err("duplicate samples");
        assert!(matches!(error, ExpressionMatrixError::InvalidHeader(_)));
    }

    #[test]
    fn rejects_non_numeric_values() {
        let error = expression_matrix_qc(Cursor::new(b"gene,s1\nA,nope\n"))
            .expect_err("invalid numeric value");
        assert!(matches!(
            error,
            ExpressionMatrixError::InvalidRecord { record: 1, .. }
        ));
    }

    #[test]
    fn detects_tsv_when_quoted_headers_contain_commas() {
        let input = b"gene\t\"sample,with,many,commas\"\ts2\nA\t1\t2\nB\t3\t4\n";
        let qc = expression_matrix_qc(Cursor::new(input)).expect("valid quoted TSV");

        assert_eq!(qc.delimiter, "tab");
        assert_eq!(qc.sample_count, 2);
        assert_eq!(qc.samples[0].sample, "sample,with,many,commas");
        assert_eq!(qc.samples[0].total, 4.0);
    }

    #[test]
    fn normalizes_nonnegative_counts_to_cpm() {
        let input = write_matrix("gene\ts1\ts2\nA\t10\t30\nB\t10\t10\n");
        let output = temporary_path("normalized.tsv");
        let summary = normalize_expression_matrix_path(
            &input,
            &output,
            &ExpressionNormalizeOptions::default(),
        )
        .expect("normalize counts");

        assert_eq!(summary.method, "cpm");
        assert_eq!(summary.samples[0].output_total, 1_000_000.0);
        assert_eq!(summary.samples[1].output_total, 1_000_000.0);
        let written = fs::read_to_string(&output).expect("read normalized matrix");
        assert!(written.contains("A\t500000\t750000"));
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn resolves_expression_pca_scores_and_loadings() {
        let input =
            write_matrix("gene\ts1\ts2\ts3\ts4\nA\t1\t2\t9\t10\nB\t2\t3\t8\t9\nC\t4\t4\t4\t4\n");
        let result = expression_pca_path(
            &input,
            &ExpressionPcaOptions {
                components: 2,
                scale_features: false,
            },
        )
        .expect("run PCA");

        assert_eq!(result.samples.len(), 4);
        assert!(!result.components.is_empty());
        assert!(result.components[0].explained_variance_percent > 95.0);
        assert!(result.samples[0].scores[0] * result.samples[3].scores[0] < 0.0);
        assert_eq!(result.warnings.len(), 1);
        let _ = fs::remove_file(input);
    }

    #[test]
    fn clusters_samples_and_features_deterministically() {
        let input = write_matrix(
            "gene\ts1\ts2\ts3\ts4\nA\t1\t1\t9\t9\nB\t2\t2\t8\t8\nC\t9\t9\t1\t1\nD\t8\t8\t2\t2\n",
        );
        let result = expression_cluster_path(
            &input,
            &ExpressionClusterOptions {
                sample_clusters: 2,
                feature_clusters: 2,
                max_iterations: 100,
                scale_features: true,
            },
        )
        .expect("cluster matrix");

        assert_eq!(result.samples.populated_clusters, 2);
        assert_eq!(result.features.populated_clusters, 2);
        assert_eq!(
            result.samples.assignments[0].cluster,
            result.samples.assignments[1].cluster
        );
        assert_ne!(
            result.samples.assignments[0].cluster,
            result.samples.assignments[2].cluster
        );
        let _ = fs::remove_file(input);
    }

    #[test]
    fn builds_a_clustered_heatmap_from_top_variable_features() {
        let input = write_matrix(
            "gene\ts1\ts2\ts3\nconstant\t4\t4\t4\nvariable1\t1\t5\t9\nvariable2\t9\t5\t1\n",
        );
        let result = expression_heatmap_path(
            &input,
            &ExpressionHeatmapOptions {
                top_variable_features: 2,
                scale_rows: true,
            },
        )
        .expect("build heatmap");

        assert_eq!(result.selected_feature_count, 2);
        assert!(!result.row_labels.contains(&"constant".to_owned()));
        assert_eq!(result.values.len(), 2);
        assert_eq!(result.values[0].len(), 3);
        assert!(result.minimum_value < 0.0);
        assert!(result.maximum_value > 0.0);
        let _ = fs::remove_file(input);
    }

    #[test]
    fn parses_normalization_methods_strictly() {
        assert_eq!(
            parse_expression_normalization_method("median-ratio").expect("method"),
            ExpressionNormalizationMethod::MedianRatio
        );
        assert!(parse_expression_normalization_method("quantile").is_err());
    }
}
