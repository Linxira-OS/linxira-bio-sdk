//! Import NumPy `.npz` archives (a ZIP container of `.npy` members) into
//! delimited expression matrices (`matrix.from-npz.v1`).
//!
//! `np.savez` writes ZIP_STORED members and `np.savez_compressed` writes
//! DEFLATE members, so the minimal archive reader below supports exactly
//! those two methods. The `.npy` payload parser implements the v1.0 format:
//! magic `\x93NUMPY`, an 8-bit version pair, a little-endian `u16` header
//! length, and an ASCII Python-dict header (`descr`, `fortran_order`,
//! `shape`) followed by raw little-endian values.

use csv::WriterBuilder;
use flate2::read::DeflateDecoder;
use serde::Serialize;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Overrides for the automatic array selection inside an npz archive.
#[derive(Debug, Clone, Default)]
pub struct NpzImportOptions {
    /// Use this array (matched after stripping the `.npy` suffix) as the main
    /// 2-D matrix instead of `counts`/`matrix`/the unique 2-D entry.
    pub matrix_name: Option<String>,
    /// Use this 1-D array as row labels instead of `rows`/`row_labels`.
    pub row_labels_name: Option<String>,
    /// Use this 1-D array as column labels instead of
    /// `cols`/`col_labels`/`genes`.
    pub column_labels_name: Option<String>,
}

/// One member array of the imported npz archive.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NpzArraySummary {
    /// Array name with the `.npy` suffix stripped.
    pub name: String,
    pub shape: Vec<u64>,
    /// Raw numpy dtype descriptor, e.g. `<f8`.
    pub dtype: String,
}

/// Summary of an npz-to-CSV/TSV import (`matrix.from-npz.v1`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NpzImportResult {
    /// Every `.npy` member found in the archive (name, shape, dtype).
    pub input_arrays: Vec<NpzArraySummary>,
    /// Name of the array imported as the main matrix.
    pub matrix_array: String,
    pub rows: u64,
    pub cols: u64,
    pub cell_count: u64,
    pub output_path: String,
    pub output_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum NpzImportError {
    Io(std::io::Error),
    Csv(csv::Error),
    MissingInput(PathBuf),
    InputEqualsOutput(PathBuf),
    NotAZipArchive,
    MalformedZip(String),
    UnsupportedZipFeature(String),
    UnsupportedNpyVersion { array: String, major: u8, minor: u8 },
    UnsupportedDtype { array: String, descr: String },
    MalformedNpy { array: String, message: String },
    ArrayNotFound { name: String, arrays: Vec<String> },
    NoMatrixArray { arrays: Vec<String> },
    AmbiguousMatrixArray { arrays: Vec<String> },
    UnsupportedOutputFormat(PathBuf),
}

impl Display for NpzImportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "npz import I/O failed: {error}"),
            Self::Csv(error) => write!(formatter, "npz import table write failed: {error}"),
            Self::MissingInput(path) => {
                write!(formatter, "input file does not exist: {}", path.display())
            }
            Self::InputEqualsOutput(path) => write!(
                formatter,
                "input and output resolve to the same path: {}",
                path.display()
            ),
            Self::NotAZipArchive => {
                formatter.write_str("input is not a ZIP/npz archive (missing local file header)")
            }
            Self::MalformedZip(message) => write!(formatter, "malformed npz archive: {message}"),
            Self::UnsupportedZipFeature(message) => formatter.write_str(message),
            Self::UnsupportedNpyVersion {
                array,
                major,
                minor,
            } => write!(
                formatter,
                "array {array:?} uses npy format {major}.{minor}; only 1.x is supported"
            ),
            Self::UnsupportedDtype { array, descr } => write!(
                formatter,
                "array {array:?} has unsupported dtype {descr:?}; expected '<f8', '<f4', '<i8', \
                 or '<i4' (label arrays may also use '|S' or '<U')"
            ),
            Self::MalformedNpy { array, message } => {
                write!(
                    formatter,
                    "malformed npy data for array {array:?}: {message}"
                )
            }
            Self::ArrayNotFound { name, arrays } => write!(
                formatter,
                "array {name:?} was not found in the npz; available arrays: {}",
                arrays.join(", ")
            ),
            Self::NoMatrixArray { arrays } => write!(
                formatter,
                "no 2-D matrix array found in the npz; available arrays: {}",
                arrays.join(", ")
            ),
            Self::AmbiguousMatrixArray { arrays } => write!(
                formatter,
                "multiple 2-D arrays found ({}); name the matrix 'counts'/'matrix' or pass an \
                 explicit array name",
                arrays.join(", ")
            ),
            Self::UnsupportedOutputFormat(path) => {
                write!(
                    formatter,
                    "output must end in .csv or .tsv: {}",
                    path.display()
                )
            }
        }
    }
}

impl Error for NpzImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NpzImportError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for NpzImportError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

/// Import the main 2-D matrix of an npz archive as a delimited table.
///
/// The main matrix is the 2-D array named `counts` or `matrix`, the unique
/// 2-D entry, or the array named by
/// [`NpzImportOptions::matrix_name`](struct.NpzImportOptions.html). Row and
/// column labels come from 1-D arrays (`rows`/`row_labels` and
/// `cols`/`col_labels`/`genes`) and fall back to positional `row{i}`/`col{j}`
/// labels with a warning. The delimiter follows the output extension
/// (`.csv` comma, `.tsv` tab).
pub fn npz_to_matrix_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &NpzImportOptions,
) -> Result<NpzImportResult, NpzImportError> {
    let input = input.as_ref();
    let output = output.as_ref();
    if !input.is_file() {
        return Err(NpzImportError::MissingInput(input.to_path_buf()));
    }
    if input == output {
        return Err(NpzImportError::InputEqualsOutput(input.to_path_buf()));
    }
    let archive = std::fs::read(input)?;
    let entries = parse_zip_entries(&archive)?;
    let mut warnings: Vec<String> = Vec::new();
    let mut arrays: Vec<(String, NpyArray)> = Vec::new();
    for entry in entries {
        if entry.name.ends_with('/') {
            continue;
        }
        let name = entry
            .name
            .strip_suffix(".npy")
            .unwrap_or(entry.name.as_str())
            .to_owned();
        arrays.push((name.clone(), parse_npy(&name, &entry.payload)?));
    }

    let matrix = select_matrix(&arrays, options)?;
    let matrix_name = matrix.0.to_owned();
    let (rows, cols) = (matrix.1.shape[0], matrix.1.shape[1]);
    if rows == 0 || cols == 0 {
        return Err(NpzImportError::MalformedNpy {
            array: matrix_name,
            message: format!("matrix has an empty dimension (shape {rows}x{cols})"),
        });
    }
    let values = decode_matrix_values(&matrix_name, matrix.1)?;

    let row_labels = resolve_axis_labels(
        &arrays,
        options.row_labels_name.as_deref(),
        &["rows", "row_labels"],
        rows,
        "row",
        &mut warnings,
    )?;
    let column_labels = resolve_axis_labels(
        &arrays,
        options.column_labels_name.as_deref(),
        &["cols", "col_labels", "genes"],
        cols,
        "col",
        &mut warnings,
    )?;
    warn_about_duplicate_labels(&row_labels, "row", &mut warnings);
    warn_about_duplicate_labels(&column_labels, "column", &mut warnings);

    let delimiter = match output
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => b',',
        Some("tsv") => b'\t',
        _ => {
            return Err(NpzImportError::UnsupportedOutputFormat(
                output.to_path_buf(),
            ));
        }
    };
    let mut writer = WriterBuilder::new()
        .delimiter(delimiter)
        .from_path(output)?;
    let mut header: Vec<&str> = Vec::with_capacity(column_labels.len() + 1);
    header.push("row");
    header.extend(column_labels.iter().map(String::as_str));
    writer.write_record(header)?;
    let row_stride = cols as usize;
    for (row_index, row_label) in row_labels.iter().enumerate() {
        let mut record: Vec<String> = Vec::with_capacity(row_stride + 1);
        record.push(row_label.clone());
        for value in &values[row_index * row_stride..(row_index + 1) * row_stride] {
            record.push(value.to_string());
        }
        writer.write_record(record)?;
    }
    writer.flush()?;
    let output_bytes = std::fs::metadata(output)?.len();

    Ok(NpzImportResult {
        input_arrays: arrays
            .iter()
            .map(|(name, array)| NpzArraySummary {
                name: name.clone(),
                shape: array.shape.clone(),
                dtype: array.descr.clone(),
            })
            .collect(),
        matrix_array: matrix_name,
        rows,
        cols,
        cell_count: rows.saturating_mul(cols),
        output_path: output.display().to_string(),
        output_bytes,
        warnings,
    })
}

const ZIP_LOCAL_HEADER_SIGNATURE: [u8; 4] = [b'P', b'K', 0x03, 0x04];
const ZIP_FLAG_DATA_DESCRIPTOR: u16 = 0x0008;
const ZIP_METHOD_STORED: u16 = 0;
const ZIP_METHOD_DEFLATE: u16 = 8;

struct ZipEntry {
    name: String,
    payload: Vec<u8>,
}

/// Walk the local file headers of a ZIP archive and return every file entry
/// with its decompressed payload.
fn parse_zip_entries(archive: &[u8]) -> Result<Vec<ZipEntry>, NpzImportError> {
    if archive.len() < 4 || archive[0..4] != ZIP_LOCAL_HEADER_SIGNATURE {
        return Err(NpzImportError::NotAZipArchive);
    }
    let mut entries = Vec::new();
    let mut offset = 0_usize;
    while offset + 4 <= archive.len() && archive[offset..offset + 4] == ZIP_LOCAL_HEADER_SIGNATURE {
        let header = offset + 30;
        if header > archive.len() {
            return Err(NpzImportError::MalformedZip(
                "truncated local file header".to_owned(),
            ));
        }
        let flags = le_u16(archive, offset + 6);
        let method = le_u16(archive, offset + 8);
        let compressed_size = le_u32(archive, offset + 18);
        let uncompressed_size = le_u32(archive, offset + 22);
        let name_length = le_u16(archive, offset + 26) as usize;
        let extra_length = le_u16(archive, offset + 28) as usize;
        if compressed_size == 0xFFFF_FFFF || uncompressed_size == 0xFFFF_FFFF {
            return Err(NpzImportError::UnsupportedZipFeature(
                "ZIP64 entries are not supported".to_owned(),
            ));
        }
        let name_start = header;
        let data_start = name_start + name_length + extra_length;
        if data_start > archive.len() {
            return Err(NpzImportError::MalformedZip(
                "local file header overruns the archive".to_owned(),
            ));
        }
        let name =
            String::from_utf8_lossy(&archive[name_start..name_start + name_length]).into_owned();
        let payload = match (method, flags & ZIP_FLAG_DATA_DESCRIPTOR != 0) {
            (ZIP_METHOD_STORED, false) => {
                let end = data_start + compressed_size as usize;
                if end > archive.len() {
                    return Err(NpzImportError::MalformedZip(
                        "stored entry overruns the archive".to_owned(),
                    ));
                }
                archive[data_start..end].to_vec()
            }
            (ZIP_METHOD_DEFLATE, false) => {
                let end = data_start + compressed_size as usize;
                if end > archive.len() {
                    return Err(NpzImportError::MalformedZip(
                        "deflated entry overruns the archive".to_owned(),
                    ));
                }
                inflate(&archive[data_start..end])?
            }
            // Streamed DEFLATE entries terminate their own deflate stream,
            // so decoding to the end of the archive is safe.
            (ZIP_METHOD_DEFLATE, true) => inflate(&archive[data_start..])?,
            (ZIP_METHOD_STORED, true) => {
                return Err(NpzImportError::UnsupportedZipFeature(
                    "stored entry with streamed sizes (data descriptor) is not supported"
                        .to_owned(),
                ));
            }
            _ => {
                return Err(NpzImportError::UnsupportedZipFeature(format!(
                    "compression method {method} is not supported; only STORED (np.savez) and \
                     DEFLATE (np.savez_compressed) entries are supported"
                )));
            }
        };
        if !name.ends_with('/') {
            entries.push(ZipEntry { name, payload });
        }
        offset = data_start + compressed_size as usize;
    }
    Ok(entries)
}

fn inflate(bytes: &[u8]) -> Result<Vec<u8>, NpzImportError> {
    let mut decoder = DeflateDecoder::new(bytes);
    let mut payload = Vec::new();
    decoder.read_to_end(&mut payload).map_err(|error| {
        NpzImportError::MalformedZip(format!("DEFLATE payload is corrupt: {error}"))
    })?;
    Ok(payload)
}

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

struct NpyArray {
    descr: String,
    fortran_order: bool,
    shape: Vec<u64>,
    payload: Vec<u8>,
}

/// Parse one `.npy` v1.x member.
fn parse_npy(array: &str, bytes: &[u8]) -> Result<NpyArray, NpzImportError> {
    if bytes.len() < 10 || bytes[0..6] != *b"\x93NUMPY" {
        return Err(NpzImportError::MalformedNpy {
            array: array.to_owned(),
            message: "invalid magic string; expected \\x93NUMPY".to_owned(),
        });
    }
    let (major, minor) = (bytes[6], bytes[7]);
    if major != 1 {
        return Err(NpzImportError::UnsupportedNpyVersion {
            array: array.to_owned(),
            major,
            minor,
        });
    }
    let header_length = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let header_end = 10 + header_length;
    if header_end > bytes.len() {
        return Err(NpzImportError::MalformedNpy {
            array: array.to_owned(),
            message: "header length overruns the entry".to_owned(),
        });
    }
    let header =
        std::str::from_utf8(&bytes[10..header_end]).map_err(|_| NpzImportError::MalformedNpy {
            array: array.to_owned(),
            message: "header is not ASCII".to_owned(),
        })?;
    let (descr, fortran_order, shape) =
        parse_header_dict(header).map_err(|message| NpzImportError::MalformedNpy {
            array: array.to_owned(),
            message,
        })?;
    Ok(NpyArray {
        descr,
        fortran_order,
        shape,
        payload: bytes[header_end..].to_vec(),
    })
}

/// Extract `descr`, `fortran_order`, and `shape` from the literal Python
/// dict header of an `.npy` v1.x member.
fn parse_header_dict(header: &str) -> Result<(String, bool, Vec<u64>), String> {
    let descr = quoted_value(header, "descr")?;
    let fortran_order = boolean_value(header, "fortran_order")?;
    let shape = shape_value(header)?;
    Ok((descr, fortran_order, shape))
}

fn quoted_value(header: &str, key: &str) -> Result<String, String> {
    let key_start = header.find(key).ok_or_else(|| format!("missing {key}"))?;
    let rest = &header[key_start + key.len()..];
    let colon = rest
        .find(':')
        .ok_or_else(|| format!("missing {key} value"))?;
    let rest = rest[colon + 1..].trim_start();
    let quote = rest
        .chars()
        .next()
        .filter(|character| *character == '\'' || *character == '"')
        .ok_or_else(|| format!("{key} value must be a quoted string"))?;
    let value = &rest[1..];
    let end = value
        .find(quote)
        .ok_or_else(|| format!("{key} value is not terminated"))?;
    Ok(value[..end].to_owned())
}

fn boolean_value(header: &str, key: &str) -> Result<bool, String> {
    let key_start = header.find(key).ok_or_else(|| format!("missing {key}"))?;
    let rest = &header[key_start + key.len()..];
    let colon = rest
        .find(':')
        .ok_or_else(|| format!("missing {key} value"))?;
    let rest = rest[colon + 1..].trim_start();
    if rest.starts_with("True") {
        Ok(true)
    } else if rest.starts_with("False") {
        Ok(false)
    } else {
        Err(format!("{key} must be True or False"))
    }
}

fn shape_value(header: &str) -> Result<Vec<u64>, String> {
    let key = "shape";
    let key_start = header.find(key).ok_or_else(|| format!("missing {key}"))?;
    let rest = &header[key_start + key.len()..];
    let colon = rest
        .find(':')
        .ok_or_else(|| format!("missing {key} value"))?;
    let rest = &rest[colon + 1..];
    let open = rest
        .find('(')
        .ok_or_else(|| format!("{key} must be a tuple"))?;
    let close = rest[open..]
        .find(')')
        .map(|position| position + open)
        .ok_or_else(|| format!("{key} tuple is not closed"))?;
    let mut dimensions = Vec::new();
    for part in rest[open + 1..close].split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        dimensions.push(
            part.parse::<u64>()
                .map_err(|_| format!("invalid shape dimension {part:?}"))?,
        );
    }
    Ok(dimensions)
}

/// Item size for the supported little-endian numeric descriptors.
fn numeric_item_size(descr: &str) -> Option<usize> {
    match descr {
        "<f8" | "=f8" => Some(8),
        "<f4" | "=f4" => Some(4),
        "<i8" | "=i8" => Some(8),
        "<i4" | "=i4" => Some(4),
        _ => None,
    }
}

fn decode_numeric_unit(descr: &str, unit: &[u8]) -> f64 {
    match descr {
        "<f8" | "=f8" => f64::from_le_bytes(unit.try_into().expect("f64 unit")),
        "<f4" | "=f4" => f32::from_le_bytes(unit.try_into().expect("f32 unit")) as f64,
        "<i8" | "=i8" => i64::from_le_bytes(unit.try_into().expect("i64 unit")) as f64,
        _ => i32::from_le_bytes(unit.try_into().expect("i32 unit")) as f64,
    }
}

/// Pick the main matrix: an explicit override, a 2-D array named
/// `counts`/`matrix`, or the unique 2-D entry.
fn select_matrix<'a>(
    arrays: &'a [(String, NpyArray)],
    options: &NpzImportOptions,
) -> Result<(&'a str, &'a NpyArray), NpzImportError> {
    if let Some(requested) = options.matrix_name.as_deref() {
        let found = arrays
            .iter()
            .find(|(name, _)| name == requested)
            .ok_or_else(|| NpzImportError::ArrayNotFound {
                name: requested.to_owned(),
                arrays: arrays.iter().map(|(name, _)| name.clone()).collect(),
            })?;
        if found.1.shape.len() != 2 {
            return Err(NpzImportError::MalformedNpy {
                array: requested.to_owned(),
                message: format!(
                    "matrix array must be 2-D, found {}-D shape {:?}",
                    found.1.shape.len(),
                    found.1.shape
                ),
            });
        }
        return Ok((found.0.as_str(), &found.1));
    }
    if let Some(found) = arrays
        .iter()
        .find(|(name, array)| (name == "counts" || name == "matrix") && array.shape.len() == 2)
    {
        return Ok((found.0.as_str(), &found.1));
    }
    let two_dimensional: Vec<&(String, NpyArray)> = arrays
        .iter()
        .filter(|(_, array)| array.shape.len() == 2)
        .collect();
    let names = || {
        two_dimensional
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<String>>()
    };
    match two_dimensional.as_slice() {
        [only] => Ok((only.0.as_str(), &only.1)),
        [] => Err(NpzImportError::NoMatrixArray {
            arrays: arrays.iter().map(|(name, _)| name.clone()).collect(),
        }),
        _ => Err(NpzImportError::AmbiguousMatrixArray { arrays: names() }),
    }
}

/// Decode the main matrix into row-major `f64` values, transposing
/// Fortran-order payloads on the fly.
fn decode_matrix_values(array: &str, npy: &NpyArray) -> Result<Vec<f64>, NpzImportError> {
    let rows = npy.shape[0] as usize;
    let cols = npy.shape[1] as usize;
    let count = rows
        .checked_mul(cols)
        .ok_or_else(|| NpzImportError::MalformedNpy {
            array: array.to_owned(),
            message: "matrix shape exceeds the supported cell count".to_owned(),
        })?;
    let item_size =
        numeric_item_size(&npy.descr).ok_or_else(|| NpzImportError::UnsupportedDtype {
            array: array.to_owned(),
            descr: npy.descr.clone(),
        })?;
    if npy.payload.len() < count * item_size {
        return Err(NpzImportError::MalformedNpy {
            array: array.to_owned(),
            message: "truncated matrix payload".to_owned(),
        });
    }
    let unit = |index: usize| {
        decode_numeric_unit(
            &npy.descr,
            &npy.payload[index * item_size..(index + 1) * item_size],
        )
    };
    let mut values = vec![0.0; count];
    for row in 0..rows {
        for column in 0..cols {
            let source_index = if npy.fortran_order {
                column * rows + row
            } else {
                row * cols + column
            };
            values[row * cols + column] = unit(source_index);
        }
    }
    Ok(values)
}

/// Resolve one axis's labels, warning and falling back to positional labels
/// when no candidate array exists or its length disagrees with the matrix.
fn resolve_axis_labels(
    arrays: &[(String, NpyArray)],
    override_name: Option<&str>,
    candidates: &[&str],
    length: u64,
    prefix: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<String>, NpzImportError> {
    let selected = match override_name {
        Some(requested) => {
            let found = arrays
                .iter()
                .find(|(name, _)| name == requested)
                .ok_or_else(|| NpzImportError::ArrayNotFound {
                    name: requested.to_owned(),
                    arrays: arrays.iter().map(|(name, _)| name.clone()).collect(),
                })?;
            Some((found.0.as_str(), &found.1))
        }
        None => candidates
            .iter()
            .find_map(|candidate| arrays.iter().find(|(name, _)| name == candidate))
            .map(|found| (found.0.as_str(), &found.1)),
    };
    let positional = || -> Vec<String> {
        (0..length)
            .map(|index| format!("{prefix}{index}"))
            .collect()
    };
    let (name, npy) = match selected {
        Some((name, npy)) => (name, npy),
        None => {
            warnings.push(format!(
                "no {prefix} label array found; generated positional {prefix} labels \
                 {prefix}0..{prefix}{}",
                length.saturating_sub(1)
            ));
            return Ok(positional());
        }
    };
    if npy.shape.len() != 1 {
        return Err(NpzImportError::MalformedNpy {
            array: name.to_owned(),
            message: format!(
                "label array must be 1-D, found {}-D shape {:?}",
                npy.shape.len(),
                npy.shape
            ),
        });
    }
    let labels = decode_label_array(name, npy)?;
    if labels.len() != length as usize {
        warnings.push(format!(
            "{prefix} label array {name:?} has {} entries but the matrix has {length} \
             {prefix}s; generated positional {prefix} labels instead",
            labels.len()
        ));
        return Ok(positional());
    }
    if labels.iter().any(String::is_empty) {
        warnings.push(format!(
            "some {prefix} labels decoded as empty strings; replaced them with positional \
             {prefix} labels"
        ));
        return Ok(labels
            .into_iter()
            .zip(positional())
            .map(|(label, fallback)| if label.is_empty() { fallback } else { label })
            .collect());
    }
    Ok(labels)
}

/// Decode a 1-D label array of a supported numeric, byte-string (`|S`), or
/// unicode (`<U`) dtype.
fn decode_label_array(array: &str, npy: &NpyArray) -> Result<Vec<String>, NpzImportError> {
    let count = npy.shape[0] as usize;
    let numeric_size = numeric_item_size(&npy.descr);
    let item_size = if let Some(size) = numeric_size {
        size
    } else if let Some(width) = string_width(&npy.descr, "|S") {
        width
    } else if let Some(width) = string_width(&npy.descr, "<U").map(|units| units * 4) {
        width
    } else {
        return Err(NpzImportError::UnsupportedDtype {
            array: array.to_owned(),
            descr: npy.descr.clone(),
        });
    };
    if npy.payload.len() < count * item_size {
        return Err(NpzImportError::MalformedNpy {
            array: array.to_owned(),
            message: "truncated label payload".to_owned(),
        });
    }
    let mut labels = Vec::with_capacity(count);
    for index in 0..count {
        let unit = &npy.payload[index * item_size..(index + 1) * item_size];
        let label = if numeric_size.is_some() {
            decode_numeric_unit(&npy.descr, unit).to_string()
        } else if npy.descr.starts_with("|S") {
            String::from_utf8_lossy(trim_trailing_nuls(unit)).into_owned()
        } else {
            let text: String = unit
                .chunks_exact(4)
                .map(|code| char::from_u32(le_u32(code, 0)).unwrap_or(char::REPLACEMENT_CHARACTER))
                .collect();
            text.trim_end_matches('\0').to_owned()
        };
        labels.push(label);
    }
    Ok(labels)
}

fn string_width(descr: &str, prefix: &str) -> Option<usize> {
    descr.strip_prefix(prefix)?.parse::<usize>().ok()
}

fn trim_trailing_nuls(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == 0 {
        end -= 1;
    }
    &bytes[..end]
}

fn warn_about_duplicate_labels(labels: &[String], axis: &str, warnings: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for label in labels {
        if !seen.insert(label.clone()) {
            duplicates.insert(label.clone());
        }
    }
    if !duplicates.is_empty() {
        warnings.push(format!(
            "duplicate {axis} labels ({}); downstream matrix consumers may reject them",
            duplicates.into_iter().collect::<Vec<String>>().join(", ")
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::{NpzImportOptions, npz_to_matrix_path};
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temporary_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "linxira-npz-test-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// Build a minimal `.npy` v1.0 member with a 64-byte-aligned header.
    fn npy_bytes(descr: &str, fortran_order: bool, shape: &[u64], payload: &[u8]) -> Vec<u8> {
        let shape_text = match shape {
            [] => "()".to_owned(),
            [only] => format!("({only},)"),
            dimensions => format!(
                "({})",
                dimensions
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        let fortran_text = if fortran_order { "True" } else { "False" };
        let mut header = format!(
            "{{'descr': '{descr}', 'fortran_order': {fortran_text}, 'shape': {shape_text}, }}"
        )
        .into_bytes();
        header.push(b'\n');
        let padding = (64 - (10 + 2 + header.len()) % 64) % 64;
        header.extend(std::iter::repeat_n(b' ', padding));
        let mut bytes = Vec::with_capacity(12 + header.len() + payload.len());
        bytes.extend_from_slice(b"\x93NUMPY");
        bytes.extend_from_slice(&[1, 0]);
        bytes.extend_from_slice(&(header.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(payload);
        bytes
    }

    /// Wrap one payload as a ZIP local entry (STORED like np.savez).
    fn zip_stored(name: &str, payload: &[u8]) -> Vec<u8> {
        zip_entry(name, payload.len(), payload.to_vec(), 0)
    }

    /// Wrap one payload as a DEFLATE ZIP entry (like np.savez_compressed).
    fn zip_deflated(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload).expect("compress");
        let compressed = encoder.finish().expect("finish");
        zip_entry(name, payload.len(), compressed, 8)
    }

    fn zip_entry(
        name: &str,
        uncompressed_size: usize,
        compressed: Vec<u8>,
        method: u16,
    ) -> Vec<u8> {
        let name_bytes = name.as_bytes();
        let mut entry = Vec::with_capacity(30 + name_bytes.len() + compressed.len());
        entry.extend_from_slice(&[b'P', b'K', 0x03, 0x04]);
        entry.extend_from_slice(&20_u16.to_le_bytes()); // version needed
        entry.extend_from_slice(&0_u16.to_le_bytes()); // flags
        entry.extend_from_slice(&method.to_le_bytes());
        entry.extend_from_slice(&0_u16.to_le_bytes()); // mod time
        entry.extend_from_slice(&0_u16.to_le_bytes()); // mod date
        entry.extend_from_slice(&0_u32.to_le_bytes()); // crc32 (unchecked)
        entry.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        entry.extend_from_slice(&(uncompressed_size as u32).to_le_bytes());
        entry.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        entry.extend_from_slice(&0_u16.to_le_bytes()); // extra length
        entry.extend_from_slice(name_bytes);
        entry.extend_from_slice(&compressed);
        entry
    }

    fn f32_payload(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn f64_payload(values: &[f64]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn i32_payload(values: &[i32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn i64_payload(values: &[i64]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn bytes_payload(values: &[&str], width: usize) -> Vec<u8> {
        let mut payload = Vec::new();
        for value in values {
            let unit = value.as_bytes();
            assert!(unit.len() <= width, "label exceeds |S width");
            payload.extend_from_slice(unit);
            payload.extend(std::iter::repeat_n(0_u8, width - unit.len()));
        }
        payload
    }

    fn unicode_payload(values: &[&str], width: usize) -> Vec<u8> {
        let mut payload = Vec::new();
        for value in values {
            let units: Vec<u32> = value.chars().map(u32::from).collect();
            assert!(units.len() <= width, "label exceeds <U width");
            for unit in &units {
                payload.extend_from_slice(&unit.to_le_bytes());
            }
            for _ in units.len()..width {
                payload.extend_from_slice(&0_u32.to_le_bytes());
            }
        }
        payload
    }

    #[test]
    fn imports_2d_f8_matrix_with_labels() {
        let dir = temporary_dir();
        let counts = npy_bytes(
            "<f8",
            false,
            &[2, 3],
            &f64_payload(&[1.0, 2.5, 3.0, 4.0, 0.0, -1.25]),
        );
        let rows = npy_bytes("|S6", false, &[2], &bytes_payload(&["geneA", "geneBB"], 6));
        let cols = npy_bytes("<U2", false, &[3], &unicode_payload(&["s1", "s2", "s3"], 2));
        let norms = npy_bytes("<f4", false, &[1, 1], &f32_payload(&[9.0]));
        let mut archive = Vec::new();
        archive.extend(zip_stored("counts.npy", &counts));
        archive.extend(zip_stored("rows.npy", &rows));
        archive.extend(zip_stored("cols.npy", &cols));
        archive.extend(zip_stored("norms.npy", &norms));
        let input = dir.join("counts.npz");
        let output = dir.join("counts.tsv");
        std::fs::write(&input, &archive).expect("write npz");
        let result =
            npz_to_matrix_path(&input, &output, &NpzImportOptions::default()).expect("import npz");
        assert_eq!(result.matrix_array, "counts");
        assert_eq!(result.rows, 2);
        assert_eq!(result.cols, 3);
        assert_eq!(result.cell_count, 6);
        assert!(result.warnings.is_empty());
        assert_eq!(result.input_arrays.len(), 4);
        assert_eq!(result.input_arrays[0].name, "counts");
        assert_eq!(result.input_arrays[0].dtype, "<f8");
        assert_eq!(result.input_arrays[0].shape, vec![2, 3]);
        let table = std::fs::read_to_string(&output).expect("read output");
        assert_eq!(
            table,
            "row\ts1\ts2\ts3\ngeneA\t1\t2.5\t3\ngeneBB\t4\t0\t-1.25\n"
        );
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn generates_positional_labels_for_a_unique_2d_deflated_entry() {
        let dir = temporary_dir();
        let matrix = npy_bytes("<i8", false, &[2, 2], &i64_payload(&[10, 20, 30, 40]));
        let archive = zip_deflated("expr.npy", &matrix);
        let input = dir.join("expr.npz");
        let output = dir.join("expr.csv");
        std::fs::write(&input, &archive).expect("write npz");
        let result =
            npz_to_matrix_path(&input, &output, &NpzImportOptions::default()).expect("import npz");
        assert_eq!(result.matrix_array, "expr");
        assert_eq!(result.cell_count, 4);
        assert_eq!(result.warnings.len(), 2);
        assert!(result.warnings[0].contains("no row label array"));
        assert!(result.warnings[1].contains("no col label array"));
        let table = std::fs::read_to_string(&output).expect("read output");
        assert_eq!(table, "row,col0,col1\nrow0,10,20\nrow1,30,40\n");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn transposes_fortran_order_f4_matrices() {
        let dir = temporary_dir();
        // Fortran order stores (0,0),(1,0),(0,1),(1,1).
        let counts = npy_bytes(
            "<f4",
            true,
            &[2, 2],
            &f32_payload(&[10.5, 20.5, 30.5, 40.5]),
        );
        let rows = npy_bytes("<i4", false, &[2], &i32_payload(&[100, 101]));
        let mut archive = Vec::new();
        archive.extend(zip_stored("counts.npy", &counts));
        archive.extend(zip_stored("rows.npy", &rows));
        let input = dir.join("counts.npz");
        let output = dir.join("counts.tsv");
        std::fs::write(&input, &archive).expect("write npz");
        let result =
            npz_to_matrix_path(&input, &output, &NpzImportOptions::default()).expect("import npz");
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("no col label array"));
        let table = std::fs::read_to_string(&output).expect("read output");
        assert_eq!(table, "row\tcol0\tcol1\n100\t10.5\t30.5\n101\t20.5\t40.5\n");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn rejects_invalid_npy_magic() {
        let dir = temporary_dir();
        let archive = zip_stored("counts.npy", b"definitely-not-an-npy-payload");
        let input = dir.join("bad.npz");
        let output = dir.join("bad.tsv");
        std::fs::write(&input, &archive).expect("write npz");
        let error = npz_to_matrix_path(&input, &output, &NpzImportOptions::default())
            .expect_err("invalid magic must fail");
        assert!(error.to_string().contains("magic"), "{error}");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn rejects_unsupported_output_extensions() {
        let dir = temporary_dir();
        let archive = zip_stored(
            "counts.npy",
            &npy_bytes("<i8", false, &[1, 1], &i64_payload(&[7])),
        );
        let input = dir.join("counts.npz");
        let output = dir.join("counts.txt");
        std::fs::write(&input, &archive).expect("write npz");
        let error = npz_to_matrix_path(&input, &output, &NpzImportOptions::default())
            .expect_err("unknown extension must fail");
        assert!(error.to_string().contains(".txt"), "{error}");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn rejects_ambiguous_unnamed_matrices() {
        let dir = temporary_dir();
        let mut archive = Vec::new();
        archive.extend(zip_stored(
            "a.npy",
            &npy_bytes("<i8", false, &[1, 1], &i64_payload(&[1])),
        ));
        archive.extend(zip_stored(
            "b.npy",
            &npy_bytes("<i8", false, &[1, 1], &i64_payload(&[2])),
        ));
        let input = dir.join("ambiguous.npz");
        let output = dir.join("ambiguous.tsv");
        std::fs::write(&input, &archive).expect("write npz");
        let error = npz_to_matrix_path(&input, &output, &NpzImportOptions::default())
            .expect_err("ambiguous matrices must fail");
        assert!(error.to_string().contains("multiple 2-D arrays"), "{error}");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
