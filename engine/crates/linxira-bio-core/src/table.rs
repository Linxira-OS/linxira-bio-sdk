use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableDelimiter {
    Csv,
    Tsv,
}

impl TableDelimiter {
    pub fn byte(self) -> u8 {
        match self {
            Self::Csv => b',',
            Self::Tsv => b'\t',
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
        }
    }

    pub fn infer_from_path(path: &Path) -> Option<Self> {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)?;
        let extension = match extension.as_str() {
            "gz" | "bgz" | "bgzip" => path
                .file_stem()
                .and_then(|stem| Path::new(stem).extension())
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or(extension),
            _ => extension,
        };
        match extension.as_str() {
            "csv" => Some(Self::Csv),
            "tsv" | "tab" => Some(Self::Tsv),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableFilter {
    Equals { column: String, value: String },
    Contains { column: String, value: String },
    NonEmpty { column: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableManipulateOptions {
    pub input_delimiter: Option<TableDelimiter>,
    pub output_delimiter: Option<TableDelimiter>,
    pub select_columns: Vec<String>,
    pub drop_columns: Vec<String>,
    pub filter: Option<TableFilter>,
    pub skip_rows: usize,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct TableManipulateSummary {
    pub input_rows: u64,
    pub output_rows: u64,
    pub skipped_rows: u64,
    pub filtered_rows: u64,
    pub input_columns: usize,
    pub output_columns: usize,
    pub input_delimiter: String,
    pub output_delimiter: String,
    pub selected_columns: Vec<String>,
    pub dropped_columns: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum TableManipulateError {
    Io(io::Error),
    Csv(csv::Error),
    OutputAlreadyExists(PathBuf),
    InvalidOption(String),
    EmptyTable,
    DuplicateColumn(String),
    MissingColumn(String),
}

impl Display for TableManipulateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to process table: {error}"),
            Self::Csv(error) => write!(formatter, "malformed delimited table: {error}"),
            Self::OutputAlreadyExists(path) => {
                write!(
                    formatter,
                    "refusing to overwrite existing output: {}",
                    path.display()
                )
            }
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::EmptyTable => formatter.write_str("table contains no header row"),
            Self::DuplicateColumn(column) => write!(formatter, "duplicate column name: {column}"),
            Self::MissingColumn(column) => write!(formatter, "column not found: {column}"),
        }
    }
}

impl Error for TableManipulateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for TableManipulateError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for TableManipulateError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

pub fn manipulate_table_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &TableManipulateOptions,
) -> Result<TableManipulateSummary, TableManipulateError> {
    let input = input.as_ref();
    let output = output.as_ref();
    if output.exists() {
        return Err(TableManipulateError::OutputAlreadyExists(output.to_owned()));
    }
    validate_options(options)?;
    let input_delimiter = match options.input_delimiter {
        Some(delimiter) => delimiter,
        None => infer_delimiter(input)?,
    };
    let output_delimiter = options
        .output_delimiter
        .or_else(|| TableDelimiter::infer_from_path(output))
        .unwrap_or(input_delimiter);
    let temporary = temporary_output_path(output);
    let result = (|| {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(input_delimiter.byte())
            .flexible(false)
            .from_reader(open_table(input)?);
        let headers = reader.headers().map_err(TableManipulateError::Csv)?.clone();
        if headers.is_empty() {
            return Err(TableManipulateError::EmptyTable);
        }
        let header_names = headers.iter().map(str::to_owned).collect::<Vec<_>>();
        let index = column_index(&header_names)?;
        let selected_indices = selected_indices(&header_names, &index, options)?;
        let filter_index = filter_index(options.filter.as_ref(), &index)?;
        let output_headers = selected_indices
            .iter()
            .map(|&position| header_names[position].clone())
            .collect::<Vec<_>>();
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let mut writer = csv::WriterBuilder::new()
            .delimiter(output_delimiter.byte())
            .from_writer(BufWriter::new(file));
        writer.write_record(&output_headers)?;
        let mut summary = TableManipulateSummary {
            input_columns: header_names.len(),
            output_columns: output_headers.len(),
            input_delimiter: input_delimiter.name().to_owned(),
            output_delimiter: output_delimiter.name().to_owned(),
            selected_columns: output_headers,
            dropped_columns: dropped_columns(&header_names, &selected_indices),
            ..TableManipulateSummary::default()
        };
        for record in reader.records() {
            let record = record?;
            summary.input_rows += 1;
            if summary.input_rows <= options.skip_rows as u64 {
                summary.skipped_rows += 1;
                continue;
            }
            if !passes_filter(&record, options.filter.as_ref(), filter_index) {
                summary.filtered_rows += 1;
                continue;
            }
            if options
                .limit
                .is_some_and(|limit| summary.output_rows >= limit as u64)
            {
                continue;
            }
            writer.write_record(selected_indices.iter().map(|&position| &record[position]))?;
            summary.output_rows += 1;
        }
        if summary.output_rows == 0 {
            summary
                .warnings
                .push("no data rows were written".to_owned());
        }
        writer.flush()?;
        drop(writer);
        fs::rename(&temporary, output)?;
        Ok(summary)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn open_table(path: &Path) -> Result<Box<dyn io::Read>, TableManipulateError> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let is_gzip = reader.fill_buf()?.starts_with(&[0x1f, 0x8b]);
    if is_gzip {
        Ok(Box::new(MultiGzDecoder::new(reader)))
    } else {
        Ok(Box::new(reader))
    }
}

fn infer_delimiter(path: &Path) -> Result<TableDelimiter, TableManipulateError> {
    if let Some(delimiter) = TableDelimiter::infer_from_path(path) {
        return Ok(delimiter);
    }
    let mut reader = BufReader::new(File::open(path)?);
    let mut sample = Vec::new();
    reader.read_until(b'\n', &mut sample)?;
    let comma = sample.iter().filter(|&&byte| byte == b',').count();
    let tab = sample.iter().filter(|&&byte| byte == b'\t').count();
    match (comma, tab) {
        (0, 0) => Err(TableManipulateError::InvalidOption(
            "could not infer CSV or TSV delimiter; pass --delimiter".to_owned(),
        )),
        (comma, tab) if tab > comma => Ok(TableDelimiter::Tsv),
        _ => Ok(TableDelimiter::Csv),
    }
}

fn validate_options(options: &TableManipulateOptions) -> Result<(), TableManipulateError> {
    if !options.select_columns.is_empty() && !options.drop_columns.is_empty() {
        return Err(TableManipulateError::InvalidOption(
            "select_columns and drop_columns cannot both be used".to_owned(),
        ));
    }
    if options.limit == Some(0) {
        return Err(TableManipulateError::InvalidOption(
            "limit must be at least 1 when provided".to_owned(),
        ));
    }
    Ok(())
}

fn column_index(columns: &[String]) -> Result<HashMap<String, usize>, TableManipulateError> {
    let mut index = HashMap::new();
    for (position, column) in columns.iter().enumerate() {
        if index.insert(column.clone(), position).is_some() {
            return Err(TableManipulateError::DuplicateColumn(column.clone()));
        }
    }
    Ok(index)
}

fn selected_indices(
    headers: &[String],
    index: &HashMap<String, usize>,
    options: &TableManipulateOptions,
) -> Result<Vec<usize>, TableManipulateError> {
    if !options.select_columns.is_empty() {
        return options
            .select_columns
            .iter()
            .map(|column| {
                index
                    .get(column)
                    .copied()
                    .ok_or_else(|| TableManipulateError::MissingColumn(column.clone()))
            })
            .collect();
    }
    let mut dropped = HashSet::new();
    for column in &options.drop_columns {
        let position = index
            .get(column)
            .copied()
            .ok_or_else(|| TableManipulateError::MissingColumn(column.clone()))?;
        dropped.insert(position);
    }
    Ok((0..headers.len())
        .filter(|position| !dropped.contains(position))
        .collect())
}

fn filter_index(
    filter: Option<&TableFilter>,
    index: &HashMap<String, usize>,
) -> Result<Option<usize>, TableManipulateError> {
    match filter {
        Some(TableFilter::Equals { column, .. })
        | Some(TableFilter::Contains { column, .. })
        | Some(TableFilter::NonEmpty { column }) => index
            .get(column)
            .copied()
            .map(Some)
            .ok_or_else(|| TableManipulateError::MissingColumn(column.clone())),
        None => Ok(None),
    }
}

fn passes_filter(
    record: &csv::StringRecord,
    filter: Option<&TableFilter>,
    filter_index: Option<usize>,
) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let value = filter_index
        .and_then(|position| record.get(position))
        .unwrap_or("");
    match filter {
        TableFilter::Equals {
            value: expected, ..
        } => value == expected,
        TableFilter::Contains { value: needle, .. } => value.contains(needle),
        TableFilter::NonEmpty { .. } => !value.trim().is_empty(),
    }
}

fn dropped_columns(headers: &[String], selected_indices: &[usize]) -> Vec<String> {
    let selected = selected_indices.iter().copied().collect::<HashSet<_>>();
    headers
        .iter()
        .enumerate()
        .filter(|(index, _)| !selected.contains(index))
        .map(|(_, column)| column.clone())
        .collect()
}

fn temporary_output_path(output: &Path) -> PathBuf {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("table-output");
    let mut temporary = output.to_owned();
    temporary.set_file_name(format!(".{file_name}.linxira-tmp-{}", std::process::id()));
    temporary
}

#[cfg(test)]
mod tests {
    use super::{TableDelimiter, TableFilter, TableManipulateOptions, manipulate_table_path};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn selects_filters_skips_and_limits_rows() {
        let input = temporary_path("input.tsv");
        let output = temporary_path("output.csv");
        fs::write(
            &input,
            "gene\tsample\tvalue\tcomment\nA\ts1\t1\tkeep\nB\ts1\t2\tdrop\nC\ts2\t3\tkeep\nD\ts1\t4\tkeep\n",
        )
        .expect("write table");
        let summary = manipulate_table_path(
            &input,
            &output,
            &TableManipulateOptions {
                output_delimiter: Some(TableDelimiter::Csv),
                select_columns: vec!["gene".to_owned(), "value".to_owned()],
                filter: Some(TableFilter::Equals {
                    column: "sample".to_owned(),
                    value: "s1".to_owned(),
                }),
                skip_rows: 1,
                limit: Some(2),
                ..TableManipulateOptions::default()
            },
        )
        .expect("manipulate table");
        assert_eq!(summary.input_rows, 4);
        assert_eq!(summary.output_rows, 2);
        assert_eq!(summary.skipped_rows, 1);
        assert_eq!(summary.filtered_rows, 1);
        assert_eq!(summary.output_columns, 2);
        assert_eq!(
            fs::read_to_string(&output).expect("output table"),
            "gene,value\nB,2\nD,4\n"
        );
        cleanup(&[input, output]);
    }

    #[test]
    fn drops_columns_and_reads_gzip_by_magic_bytes() {
        let input = temporary_path("input.data");
        let output = temporary_path("output.tsv");
        let mut gzip = GzEncoder::new(
            fs::File::create(&input).expect("create gzip table"),
            Compression::default(),
        );
        gzip.write_all(b"id,name,note\n1,Alice,alpha\n2,Bob,beta\n")
            .expect("write gzip table");
        gzip.finish().expect("finish gzip table");
        let summary = manipulate_table_path(
            &input,
            &output,
            &TableManipulateOptions {
                input_delimiter: Some(TableDelimiter::Csv),
                output_delimiter: Some(TableDelimiter::Tsv),
                drop_columns: vec!["note".to_owned()],
                filter: Some(TableFilter::Contains {
                    column: "name".to_owned(),
                    value: "o".to_owned(),
                }),
                ..TableManipulateOptions::default()
            },
        )
        .expect("drop column");
        assert_eq!(summary.output_rows, 1);
        assert_eq!(summary.dropped_columns, vec!["note"]);
        assert_eq!(
            fs::read_to_string(&output).expect("output table"),
            "id\tname\n2\tBob\n"
        );
        cleanup(&[input, output]);
    }

    #[test]
    fn refuses_to_overwrite_outputs() {
        let input = temporary_path("overwrite-input.csv");
        let output = temporary_path("overwrite-output.csv");
        fs::write(&input, "id\n1\n").expect("write input");
        fs::write(&output, "protected\n").expect("write output");
        let error = manipulate_table_path(&input, &output, &TableManipulateOptions::default())
            .expect_err("existing output must fail");
        assert!(error.to_string().contains("refusing to overwrite"));
        assert_eq!(
            fs::read_to_string(&output).expect("protected"),
            "protected\n"
        );
        cleanup(&[input, output]);
    }

    #[test]
    fn infers_nested_gzip_table_extensions() {
        assert_eq!(
            TableDelimiter::infer_from_path(&PathBuf::from("counts.csv.gz")),
            Some(TableDelimiter::Csv)
        );
        assert_eq!(
            TableDelimiter::infer_from_path(&PathBuf::from("counts.tsv.bgz")),
            Some(TableDelimiter::Tsv)
        );
        assert_eq!(
            TableDelimiter::infer_from_path(&PathBuf::from("counts.tab.bgzip")),
            Some(TableDelimiter::Tsv)
        );
    }

    fn temporary_path(suffix: &str) -> PathBuf {
        let count = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-table-manipulate-{}-{count}-{suffix}",
            std::process::id()
        ))
    }

    fn cleanup(paths: &[PathBuf]) {
        for path in paths {
            let _ = fs::remove_file(path);
        }
    }
}
