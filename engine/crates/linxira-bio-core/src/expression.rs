use csv::{ReaderBuilder, Trim};
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

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

#[derive(Debug)]
pub enum ExpressionMatrixError {
    Io(io::Error),
    Csv(csv::Error),
    InvalidHeader(String),
    InvalidRecord { record: u64, message: String },
}

impl Display for ExpressionMatrixError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read expression matrix: {error}"),
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
        }
    }
}

impl Error for ExpressionMatrixError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            Self::InvalidHeader(_) | Self::InvalidRecord { .. } => None,
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
    let path = path.as_ref();
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
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
    use super::{ExpressionMatrixError, expression_matrix_qc};
    use std::io::Cursor;

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
}
