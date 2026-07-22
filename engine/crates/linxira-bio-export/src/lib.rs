#![forbid(unsafe_code)]

//! Shared table export used by the desktop, CLI, SDK, and workflow adapters.

use rust_xlsxwriter::{Format, Workbook};
use serde::Serialize;
use serde_json::{Map, Number, Value};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Seek, Write};
use std::path::Path;
use tempfile::NamedTempFile;

pub type ExportResult<T> = Result<T, ExportError>;

#[derive(Debug)]
pub enum ExportError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Csv(csv::Error),
    Xlsx(rust_xlsxwriter::XlsxError),
    UnsupportedExtension(String),
    InvalidTable(String),
    SpreadsheetTooLarge { rows: usize, columns: usize },
}

impl fmt::Display for ExportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
            Self::Csv(error) => write!(formatter, "delimited export error: {error}"),
            Self::Xlsx(error) => write!(formatter, "XLSX export error: {error}"),
            Self::UnsupportedExtension(extension) => {
                write!(formatter, "unsupported export extension: {extension}")
            }
            Self::InvalidTable(message) => write!(formatter, "invalid table: {message}"),
            Self::SpreadsheetTooLarge { rows, columns } => write!(
                formatter,
                "table exceeds XLSX limits: {rows} rows and {columns} columns"
            ),
        }
    }
}

impl Error for ExportError {}

impl From<std::io::Error> for ExportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ExportError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<csv::Error> for ExportError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

impl From<rust_xlsxwriter::XlsxError> for ExportError {
    fn from(error: rust_xlsxwriter::XlsxError) -> Self {
        Self::Xlsx(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Tsv,
    Json,
    Jsonl,
    Xlsx,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportReceipt {
    pub schema_version: String,
    pub format: ExportFormat,
    pub output_path: String,
    pub size_bytes: u64,
}

impl ExportFormat {
    pub fn from_path(path: &Path) -> ExportResult<Self> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "csv" => Ok(Self::Csv),
            "tsv" => Ok(Self::Tsv),
            "json" => Ok(Self::Json),
            "jsonl" => Ok(Self::Jsonl),
            "xlsx" => Ok(Self::Xlsx),
            _ => Err(ExportError::UnsupportedExtension(extension)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

impl Table {
    pub fn from_json(value: &Value) -> ExportResult<Self> {
        let value = value.get("result").unwrap_or(value);
        match value {
            Value::Object(object) => Self::from_objects(std::slice::from_ref(object)),
            Value::Array(values) if values.iter().all(Value::is_object) => {
                let objects = values
                    .iter()
                    .filter_map(Value::as_object)
                    .collect::<Vec<_>>();
                Self::from_object_refs(&objects)
            }
            Value::Array(values) if values.iter().all(Value::is_array) => {
                let mut rows = values
                    .iter()
                    .filter_map(Value::as_array)
                    .cloned()
                    .collect::<Vec<_>>();
                let width = rows.iter().map(Vec::len).max().unwrap_or(0);
                for row in &mut rows {
                    row.resize(width, Value::Null);
                }
                Ok(Self {
                    columns: (1..=width).map(|index| format!("column_{index}")).collect(),
                    rows,
                })
            }
            Value::Array(values) if values.is_empty() => Ok(Self {
                columns: Vec::new(),
                rows: Vec::new(),
            }),
            _ => Err(ExportError::InvalidTable(
                "expected an object, an array of objects, or a two-dimensional array".to_owned(),
            )),
        }
    }

    fn from_objects(objects: &[Map<String, Value>]) -> ExportResult<Self> {
        let refs = objects.iter().collect::<Vec<_>>();
        Self::from_object_refs(&refs)
    }

    fn from_object_refs(objects: &[&Map<String, Value>]) -> ExportResult<Self> {
        let mut names = BTreeSet::new();
        for object in objects {
            names.extend(object.keys().cloned());
        }
        let columns = names.into_iter().collect::<Vec<_>>();
        let rows = objects
            .iter()
            .map(|object| {
                columns
                    .iter()
                    .map(|column| object.get(column).cloned().unwrap_or(Value::Null))
                    .collect()
            })
            .collect();
        Ok(Self { columns, rows })
    }
}

pub fn export_json_file(input: &Path, output: &Path) -> ExportResult<ExportReceipt> {
    ensure_distinct_input_output(input, output)?;
    let value: Value = serde_json::from_reader(BufReader::new(File::open(input)?))?;
    export_value(&value, output)
}

pub fn ensure_distinct_input_output(input: &Path, output: &Path) -> ExportResult<()> {
    if output.exists() && same_file::is_same_file(input, output)? {
        return Err(ExportError::InvalidTable(
            "input and output paths must be different".to_owned(),
        ));
    }
    Ok(())
}

pub fn export_value(value: &Value, output: &Path) -> ExportResult<ExportReceipt> {
    let format = ExportFormat::from_path(output)?;
    let output_directory = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(output_directory)?;
    match format {
        ExportFormat::Json => {
            let mut writer = BufWriter::new(temporary.as_file_mut());
            serde_json::to_writer_pretty(&mut writer, value)?;
            writer.flush()?;
        }
        ExportFormat::Jsonl => write_jsonl(value, temporary.as_file_mut())?,
        ExportFormat::Csv => {
            write_delimited(&Table::from_json(value)?, temporary.as_file_mut(), b',')?
        }
        ExportFormat::Tsv => {
            write_delimited(&Table::from_json(value)?, temporary.as_file_mut(), b'\t')?
        }
        ExportFormat::Xlsx => write_xlsx(&Table::from_json(value)?, temporary.as_file_mut())?,
    }
    let persisted = temporary
        .persist(output)
        .map_err(|error| ExportError::Io(error.error))?;
    Ok(ExportReceipt {
        schema_version: "1".to_owned(),
        format,
        output_path: output.display().to_string(),
        size_bytes: persisted.metadata()?.len(),
    })
}

fn write_jsonl<W: Write>(value: &Value, output: W) -> ExportResult<()> {
    let value = value.get("result").unwrap_or(value);
    let objects = match value {
        Value::Object(object) => vec![object],
        Value::Array(values) if values.iter().all(Value::is_object) => {
            values.iter().filter_map(Value::as_object).collect()
        }
        _ => {
            return Err(ExportError::InvalidTable(
                "JSONL export requires an object or an array of objects".to_owned(),
            ));
        }
    };

    let mut writer = BufWriter::new(output);
    for object in objects {
        serde_json::to_writer(&mut writer, object)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn write_delimited<W: Write>(table: &Table, output: W, delimiter: u8) -> ExportResult<()> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .from_writer(BufWriter::new(output));
    writer.write_record(&table.columns)?;
    for row in &table.rows {
        writer.write_record(row.iter().map(cell_text))?;
    }
    writer.flush()?;
    Ok(())
}

fn write_xlsx<W: Write + Seek + Send>(table: &Table, output: W) -> ExportResult<()> {
    const MAX_ROWS: usize = 1_048_576;
    const MAX_COLUMNS: usize = 16_384;
    let output_rows = table.rows.len().saturating_add(1);
    if output_rows > MAX_ROWS || table.columns.len() > MAX_COLUMNS {
        return Err(ExportError::SpreadsheetTooLarge {
            rows: output_rows,
            columns: table.columns.len(),
        });
    }

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name("Results")?;
    worksheet.set_freeze_panes(1, 0)?;
    let header = Format::new().set_bold();
    for (column, value) in table.columns.iter().enumerate() {
        worksheet.write_string_with_format(0, column as u16, value, &header)?;
    }
    for (row, values) in table.rows.iter().enumerate() {
        for (column, value) in values.iter().enumerate() {
            write_xlsx_cell(worksheet, (row + 1) as u32, column as u16, value)?;
        }
    }
    worksheet.autofit();
    workbook.save_to_writer(output)?;
    Ok(())
}

fn write_xlsx_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    column: u16,
    value: &Value,
) -> ExportResult<()> {
    match value {
        Value::Null => {}
        Value::Bool(value) => {
            worksheet.write_boolean(row, column, *value)?;
        }
        Value::Number(value) => match xlsx_numeric_cell(value) {
            XlsxNumericCell::Number(number) => {
                worksheet.write_number(row, column, number)?;
            }
            XlsxNumericCell::Text(text) => {
                worksheet.write_string(row, column, text)?;
            }
        },
        Value::String(value) => {
            worksheet.write_string(row, column, value)?;
        }
        Value::Array(_) | Value::Object(_) => {
            worksheet.write_string(row, column, serde_json::to_string(value)?)?;
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
enum XlsxNumericCell {
    Number(f64),
    Text(String),
}

fn xlsx_numeric_cell(value: &Number) -> XlsxNumericCell {
    if let Some(integer) = value.as_i64() {
        if integer_is_exact_f64(integer.unsigned_abs()) {
            XlsxNumericCell::Number(integer as f64)
        } else {
            XlsxNumericCell::Text(value.to_string())
        }
    } else if let Some(integer) = value.as_u64() {
        if integer_is_exact_f64(integer) {
            XlsxNumericCell::Number(integer as f64)
        } else {
            XlsxNumericCell::Text(value.to_string())
        }
    } else if let Some(number) = value.as_f64() {
        XlsxNumericCell::Number(number)
    } else {
        XlsxNumericCell::Text(value.to_string())
    }
}

fn integer_is_exact_f64(magnitude: u64) -> bool {
    if magnitude == 0 {
        return true;
    }
    let significant_bits = 64 - magnitude.leading_zeros() - magnitude.trailing_zeros();
    significant_bits <= f64::MANTISSA_DIGITS
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(_) | Value::Number(_) => value.to_string(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ExportFormat, Table, XlsxNumericCell, export_value, xlsx_numeric_cell};
    use serde_json::json;
    use std::fs;
    use std::path::Path;

    #[test]
    fn converts_result_objects_to_a_stable_table() {
        let table = Table::from_json(&json!({
            "result": {"zeta": 2, "alpha": "A"}
        }))
        .expect("table");

        assert_eq!(table.columns, ["alpha", "zeta"]);
        assert_eq!(table.rows, [vec![json!("A"), json!(2)]]);
    }

    #[test]
    fn normalizes_missing_object_fields() {
        let table = Table::from_json(&json!([
            {"sample": "A", "count": 4},
            {"sample": "B"}
        ]))
        .expect("table");

        assert_eq!(table.columns, ["count", "sample"]);
        assert_eq!(table.rows[1][0], serde_json::Value::Null);
    }

    #[test]
    fn writes_csv_tsv_json_and_xlsx() {
        let root = std::env::temp_dir().join(format!("linxira-bio-export-{}", std::process::id()));
        fs::create_dir_all(&root).expect("temporary export directory");
        let value = json!([{"sample": "样本一", "count": 12}]);

        for extension in ["csv", "tsv", "json", "jsonl", "xlsx"] {
            let output = root.join(format!("result.{extension}"));
            export_value(&value, &output).expect("export succeeds");
            assert!(fs::metadata(output).expect("output metadata").len() > 0);
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn infers_supported_extensions() {
        assert_eq!(
            ExportFormat::from_path(Path::new("result.XLSX")).expect("format"),
            ExportFormat::Xlsx
        );
        assert!(ExportFormat::from_path(Path::new("result.parquet")).is_err());
    }

    #[test]
    fn writes_one_object_per_jsonl_line() {
        let output = std::env::temp_dir().join(format!(
            "linxira-bio-export-jsonl-{}.jsonl",
            std::process::id()
        ));
        let value = json!({
            "result": [
                {"sample": "A", "count": 1},
                {"sample": "B", "count": 2}
            ]
        });

        let receipt = export_value(&value, &output).expect("JSONL export");
        let text = fs::read_to_string(&output).expect("read JSONL output");
        let lines = text.lines().collect::<Vec<_>>();

        assert_eq!(receipt.format, ExportFormat::Jsonl);
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[0]).expect("first JSONL row"),
            json!({"sample": "A", "count": 1})
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(lines[1]).expect("second JSONL row"),
            json!({"sample": "B", "count": 2})
        );
        fs::remove_file(output).expect("remove JSONL output");
    }

    #[test]
    fn rejects_non_object_jsonl_without_creating_output() {
        let output = std::env::temp_dir().join(format!(
            "linxira-bio-export-invalid-jsonl-{}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&output);

        let error = export_value(&json!([[1, 2], [3, 4]]), &output)
            .expect_err("array rows are not JSONL objects");

        assert!(error.to_string().contains("array of objects"));
        assert!(!output.exists());
    }

    #[test]
    fn preserves_non_exact_json_integers_as_xlsx_text() {
        let exact_boundary = json!(9_007_199_254_740_992_u64);
        let exact_above_boundary = json!(9_007_199_254_740_994_u64);
        let non_exact = json!(9_007_199_254_740_993_u64);
        let non_exact_negative = json!(-9_007_199_254_740_993_i64);
        let maximum = json!(u64::MAX);

        assert_eq!(
            xlsx_numeric_cell(exact_boundary.as_number().expect("number")),
            XlsxNumericCell::Number(9_007_199_254_740_992.0)
        );
        assert_eq!(
            xlsx_numeric_cell(exact_above_boundary.as_number().expect("number")),
            XlsxNumericCell::Number(9_007_199_254_740_994.0)
        );
        assert_eq!(
            xlsx_numeric_cell(non_exact.as_number().expect("number")),
            XlsxNumericCell::Text("9007199254740993".to_owned())
        );
        assert_eq!(
            xlsx_numeric_cell(non_exact_negative.as_number().expect("number")),
            XlsxNumericCell::Text("-9007199254740993".to_owned())
        );
        assert_eq!(
            xlsx_numeric_cell(maximum.as_number().expect("number")),
            XlsxNumericCell::Text("18446744073709551615".to_owned())
        );
    }

    #[test]
    fn refuses_to_overwrite_the_source_json() {
        let path = std::env::temp_dir().join(format!(
            "linxira-bio-export-source-{}.json",
            std::process::id()
        ));
        fs::write(&path, r#"{"value":1}"#).expect("write source");

        let error = super::export_json_file(&path, &path).expect_err("same path must fail");
        assert!(error.to_string().contains("must be different"));
        assert_eq!(
            fs::read_to_string(&path).expect("source remains readable"),
            r#"{"value":1}"#
        );
        fs::remove_file(path).expect("remove source");
    }

    #[test]
    fn refuses_to_overwrite_a_hard_link_to_the_source_json() {
        let root = std::env::temp_dir().join(format!(
            "linxira-bio-export-hard-link-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temporary export directory");
        let input = root.join("source.json");
        let output = root.join("alias.json");
        fs::write(&input, r#"{"value":1}"#).expect("write source");
        fs::hard_link(&input, &output).expect("create source hard link");

        let error =
            super::export_json_file(&input, &output).expect_err("hard-linked output must fail");

        assert!(error.to_string().contains("must be different"));
        assert_eq!(
            fs::read_to_string(&input).expect("source remains readable"),
            r#"{"value":1}"#
        );
        assert_eq!(
            fs::read_to_string(&output).expect("hard link remains readable"),
            r#"{"value":1}"#
        );
        fs::remove_dir_all(root).expect("remove hard-link fixture");
    }

    #[test]
    fn failed_export_does_not_replace_an_existing_output() {
        let output = std::env::temp_dir().join(format!(
            "linxira-bio-export-existing-invalid-{}.jsonl",
            std::process::id()
        ));
        fs::write(&output, "existing output\n").expect("write existing output");

        let error =
            export_value(&json!([[1, 2], [3, 4]]), &output).expect_err("invalid JSONL must fail");

        assert!(error.to_string().contains("array of objects"));
        assert_eq!(
            fs::read_to_string(&output).expect("existing output remains readable"),
            "existing output\n"
        );
        fs::remove_file(output).expect("remove existing output");
    }

    #[test]
    fn successful_export_atomically_replaces_an_existing_output() {
        let output = std::env::temp_dir().join(format!(
            "linxira-bio-export-existing-valid-{}.json",
            std::process::id()
        ));
        fs::write(&output, "stale output\n").expect("write existing output");

        export_value(&json!({"status": "fresh"}), &output)
            .expect("valid export replaces the existing output");

        let exported: serde_json::Value =
            serde_json::from_slice(&fs::read(&output).expect("read replaced output"))
                .expect("replacement is valid JSON");
        assert_eq!(exported, json!({"status": "fresh"}));
        fs::remove_file(output).expect("remove replaced output");
    }
}
