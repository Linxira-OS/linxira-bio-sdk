use crate::table::TableDelimiter;
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

const MAX_COHORT_ROWS: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CohortColumnQc {
    pub column: String,
    pub non_missing_count: u64,
    pub missing_count: u64,
    pub missing_percent: f64,
    pub distinct_value_count: u64,
    pub numeric_value_count: u64,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CohortTableQc {
    pub delimiter: String,
    pub row_count: u64,
    pub column_count: u64,
    pub duplicate_row_count: u64,
    pub columns: Vec<CohortColumnQc>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum CohortQcError {
    Io(io::Error),
    Csv(csv::Error),
    InvalidHeader(String),
    TooManyRows,
}

impl Display for CohortQcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "cohort table I/O failed: {error}"),
            Self::Csv(error) => write!(f, "invalid cohort table: {error}"),
            Self::InvalidHeader(message) => write!(f, "invalid cohort table header: {message}"),
            Self::TooManyRows => write!(
                f,
                "cohort table exceeds the {MAX_COHORT_ROWS} row safety limit"
            ),
        }
    }
}

impl Error for CohortQcError {}
impl From<io::Error> for CohortQcError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<csv::Error> for CohortQcError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

pub fn cohort_table_qc_path(path: impl AsRef<Path>) -> Result<CohortTableQc, CohortQcError> {
    let path = path.as_ref();
    let mut raw = BufReader::new(File::open(path)?);
    let gzip = raw.fill_buf()?.starts_with(&[0x1f, 0x8b]);
    let mut input: Box<dyn Read> = if gzip {
        Box::new(MultiGzDecoder::new(raw))
    } else {
        Box::new(raw)
    };
    let mut prefix = [0_u8; 4096];
    let read = input.read(&mut prefix)?;
    let delimiter = TableDelimiter::infer_from_path(path).unwrap_or_else(|| {
        if prefix[..read].iter().filter(|&&byte| byte == b'\t').count()
            > prefix[..read].iter().filter(|&&byte| byte == b',').count()
        {
            TableDelimiter::Tsv
        } else {
            TableDelimiter::Csv
        }
    });
    let chained = io::Cursor::new(prefix[..read].to_vec()).chain(input);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter.byte())
        .flexible(false)
        .trim(csv::Trim::All)
        .from_reader(chained);
    let headers = reader.headers()?.clone();
    if headers.is_empty() {
        return Err(CohortQcError::InvalidHeader(
            "expected at least one column".to_owned(),
        ));
    }
    let mut names = HashSet::new();
    for name in &headers {
        if name.is_empty() || !names.insert(name.to_owned()) {
            return Err(CohortQcError::InvalidHeader(
                "column names must be non-empty and unique".to_owned(),
            ));
        }
    }
    let mut missing = vec![0_u64; headers.len()];
    let mut present = vec![0_u64; headers.len()];
    let mut distinct = (0..headers.len())
        .map(|_| HashSet::new())
        .collect::<Vec<_>>();
    let mut numeric = vec![0_u64; headers.len()];
    let mut min = vec![None; headers.len()];
    let mut max = vec![None; headers.len()];
    let mut rows = HashSet::new();
    let mut row_count = 0_u64;
    let mut duplicate_rows = 0_u64;
    for record in reader.records() {
        let record = record?;
        row_count += 1;
        if row_count > MAX_COHORT_ROWS {
            return Err(CohortQcError::TooManyRows);
        }
        if !rows.insert(record.iter().collect::<Vec<_>>().join("\u{1f}")) {
            duplicate_rows += 1;
        }
        for (index, value) in record.iter().enumerate() {
            if matches!(
                value.to_ascii_lowercase().as_str(),
                "" | "na" | "nan" | "null" | "."
            ) {
                missing[index] += 1;
                continue;
            }
            present[index] += 1;
            distinct[index].insert(value.to_owned());
            if let Ok(value) = value.parse::<f64>()
                && value.is_finite()
            {
                numeric[index] += 1;
                min[index] = Some(min[index].map_or(value, |old: f64| old.min(value)));
                max[index] = Some(max[index].map_or(value, |old: f64| old.max(value)));
            }
        }
    }
    let columns = headers
        .iter()
        .enumerate()
        .map(|(i, name)| CohortColumnQc {
            column: name.to_owned(),
            non_missing_count: present[i],
            missing_count: missing[i],
            missing_percent: if row_count == 0 {
                0.0
            } else {
                missing[i] as f64 / row_count as f64 * 100.0
            },
            distinct_value_count: distinct[i].len() as u64,
            numeric_value_count: numeric[i],
            minimum: min[i],
            maximum: max[i],
        })
        .collect();
    let mut warnings = Vec::new();
    if row_count == 0 {
        warnings.push("cohort table contains no data rows".to_owned());
    }
    if duplicate_rows != 0 {
        warnings.push(format!(
            "cohort table contains {duplicate_rows} duplicate rows"
        ));
    }
    Ok(CohortTableQc {
        delimiter: delimiter.name().to_owned(),
        row_count,
        column_count: headers.len() as u64,
        duplicate_row_count: duplicate_rows,
        columns,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::cohort_table_qc_path;
    use std::path::PathBuf;

    #[test]
    fn reports_missing_values_numeric_ranges_and_duplicate_rows() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/cohort/participants.tsv");
        let result = cohort_table_qc_path(path).expect("cohort QC");
        assert_eq!(result.row_count, 4);
        assert_eq!(result.duplicate_row_count, 1);
        assert_eq!(result.columns[1].column, "age");
        assert_eq!(result.columns[1].missing_count, 1);
        assert_eq!(result.columns[1].minimum, Some(30.0));
        assert_eq!(result.columns[1].maximum, Some(42.0));
    }
}
