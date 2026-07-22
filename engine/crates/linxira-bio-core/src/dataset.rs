use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

pub const DEFAULT_PREVIEW_RECORD_LIMIT: usize = 200;
pub const DEFAULT_PREVIEW_BYTE_LIMIT: u64 = 10 * 1024 * 1024;

const DETECTION_BYTE_LIMIT: u64 = 64 * 1024;
const SEQUENCE_EXCERPT_LENGTH: usize = 120;
const TEXT_EXCERPT_LENGTH: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatasetInspectionOptions {
    pub max_preview_records: usize,
    pub max_preview_bytes: u64,
}

impl Default for DatasetInspectionOptions {
    fn default() -> Self {
        Self {
            max_preview_records: DEFAULT_PREVIEW_RECORD_LIMIT,
            max_preview_bytes: DEFAULT_PREVIEW_BYTE_LIMIT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetFormat {
    Fasta,
    Fastq,
    Csv,
    Tsv,
    Bed,
    Gff3,
    Gtf,
    Vcf,
    Sam,
    Bam,
    Zip,
    Bcf,
    Cram,
    H5ad,
    Loom,
    Hdf5,
    Rds,
    Pdb,
    Mmcif,
    Unknown,
}

impl DatasetFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fasta => "fasta",
            Self::Fastq => "fastq",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Bed => "bed",
            Self::Gff3 => "gff3",
            Self::Gtf => "gtf",
            Self::Vcf => "vcf",
            Self::Sam => "sam",
            Self::Bam => "bam",
            Self::Zip => "zip",
            Self::Bcf => "bcf",
            Self::Cram => "cram",
            Self::H5ad => "h5ad",
            Self::Loom => "loom",
            Self::Hdf5 => "hdf5",
            Self::Rds => "rds",
            Self::Pdb => "pdb",
            Self::Mmcif => "mmcif",
            Self::Unknown => "unknown",
        }
    }
}

impl Display for DatasetFormat {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetCompression {
    None,
    Gzip,
    Bgzip,
    Zip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetSupport {
    Supported,
    RecognizedUnsupported,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DetectionConfidence {
    High,
    Medium,
    Low,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewKind {
    Sequence,
    Table,
    Alignment,
    Variant,
    Binary,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetPreview {
    pub kind: PreviewKind,
    pub columns: Vec<String>,
    pub records: Vec<Vec<String>>,
    pub records_shown: usize,
    pub bytes_read: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionIssue {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetInspection {
    pub schema_version: String,
    pub path: PathBuf,
    pub file_name: String,
    pub size_bytes: u64,
    pub format: DatasetFormat,
    pub compression: DatasetCompression,
    pub support: DatasetSupport,
    pub confidence: DetectionConfidence,
    pub preview: Option<DatasetPreview>,
    pub warnings: Vec<InspectionIssue>,
    pub errors: Vec<InspectionIssue>,
}

#[derive(Debug)]
pub enum DatasetInspectionError {
    InvalidOptions(String),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl Display for DatasetInspectionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions(message) => formatter.write_str(message),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} dataset {}: {source}",
                path.display()
            ),
        }
    }
}

impl Error for DatasetInspectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidOptions(_) => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Detection {
    format: DatasetFormat,
    confidence: DetectionConfidence,
}

#[derive(Debug)]
struct PreviewOutcome {
    preview: DatasetPreview,
    warnings: Vec<InspectionIssue>,
    errors: Vec<InspectionIssue>,
}

pub fn inspect_dataset(
    path: impl AsRef<Path>,
) -> Result<DatasetInspection, DatasetInspectionError> {
    inspect_dataset_with_options(path, DatasetInspectionOptions::default())
}

pub fn inspect_dataset_with_options(
    path: impl AsRef<Path>,
    options: DatasetInspectionOptions,
) -> Result<DatasetInspection, DatasetInspectionError> {
    validate_options(options)?;
    let path = path.as_ref();
    let metadata = fs::metadata(path).map_err(|source| io_error("inspect", path, source))?;
    if !metadata.is_file() {
        return Err(DatasetInspectionError::InvalidOptions(format!(
            "dataset path is not a regular file: {}",
            path.display()
        )));
    }

    let raw_prefix = read_file_prefix(path, DETECTION_BYTE_LIMIT)
        .map_err(|source| io_error("read", path, source))?;
    let compression = detect_compression(&raw_prefix);
    let extension_detection = detect_from_extension(path);

    if compression == DatasetCompression::Zip {
        let preview_prefix_length = usize::try_from(options.max_preview_bytes.min(32))
            .expect("binary preview prefix length fits in usize")
            .min(raw_prefix.len());
        return Ok(DatasetInspection {
            schema_version: "1".to_owned(),
            path: path.to_path_buf(),
            file_name: display_file_name(path),
            size_bytes: metadata.len(),
            format: DatasetFormat::Zip,
            compression,
            support: DatasetSupport::RecognizedUnsupported,
            confidence: DetectionConfidence::High,
            preview: Some(binary_preview(
                &raw_prefix[..preview_prefix_length],
                u64::try_from(raw_prefix.len())
                    .unwrap_or(u64::MAX)
                    .min(options.max_preview_bytes),
                metadata.len() > options.max_preview_bytes,
            )),
            warnings: vec![issue(
                "unsupported-archive",
                "ZIP archives are recognized but are not imported; extract the files before inspection",
                None,
            )],
            errors: Vec::new(),
        });
    }

    let payload_prefix = match read_payload_prefix(path, compression, DETECTION_BYTE_LIMIT) {
        Ok(prefix) => prefix,
        Err(error) => {
            let detection = extension_detection.unwrap_or(Detection {
                format: DatasetFormat::Unknown,
                confidence: DetectionConfidence::None,
            });
            return Ok(DatasetInspection {
                schema_version: "1".to_owned(),
                path: path.to_path_buf(),
                file_name: display_file_name(path),
                size_bytes: metadata.len(),
                format: detection.format,
                compression,
                support: support_for(detection.format),
                confidence: detection.confidence,
                preview: None,
                warnings: Vec::new(),
                errors: vec![issue(
                    "decompression-failed",
                    format!("failed to decompress the dataset preview: {error}"),
                    None,
                )],
            });
        }
    };

    let content_detection = detect_from_content(&payload_prefix, path);
    let (detection, mut warnings) = resolve_detection(content_detection, extension_detection);
    let mut errors = Vec::new();
    if payload_prefix.is_empty() {
        errors.push(issue("empty-file", "the dataset contains no data", None));
    }

    let preview = if payload_prefix.is_empty() {
        None
    } else {
        match build_preview(path, compression, detection.format, options) {
            Ok(outcome) => {
                warnings.extend(outcome.warnings);
                errors.extend(outcome.errors);
                Some(outcome.preview)
            }
            Err(error) => {
                errors.push(issue(
                    "preview-failed",
                    format!("failed to build the dataset preview: {error}"),
                    None,
                ));
                None
            }
        }
    };

    if support_for(detection.format) == DatasetSupport::RecognizedUnsupported {
        warnings.push(issue(
            "recognized-unsupported-format",
            format!(
                "{} is recognized, but this release does not provide an executable importer for it",
                detection.format
            ),
            None,
        ));
    }

    Ok(DatasetInspection {
        schema_version: "1".to_owned(),
        path: path.to_path_buf(),
        file_name: display_file_name(path),
        size_bytes: metadata.len(),
        format: detection.format,
        compression,
        support: support_for(detection.format),
        confidence: detection.confidence,
        preview,
        warnings,
        errors,
    })
}

fn validate_options(options: DatasetInspectionOptions) -> Result<(), DatasetInspectionError> {
    if options.max_preview_records == 0 {
        return Err(DatasetInspectionError::InvalidOptions(
            "max_preview_records must be greater than zero".to_owned(),
        ));
    }
    if options.max_preview_bytes == 0 {
        return Err(DatasetInspectionError::InvalidOptions(
            "max_preview_bytes must be greater than zero".to_owned(),
        ));
    }
    Ok(())
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> DatasetInspectionError {
    DatasetInspectionError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}

fn read_file_prefix(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_payload_prefix(
    path: &Path,
    compression: DatasetCompression,
    limit: u64,
) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    open_payload(path, compression)?
        .take(limit)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn open_payload(path: &Path, compression: DatasetCompression) -> io::Result<Box<dyn Read>> {
    let file = File::open(path)?;
    match compression {
        DatasetCompression::None => Ok(Box::new(file)),
        DatasetCompression::Gzip | DatasetCompression::Bgzip => {
            Ok(Box::new(MultiGzDecoder::new(file)))
        }
        DatasetCompression::Zip => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ZIP payloads are not supported",
        )),
    }
}

fn detect_compression(prefix: &[u8]) -> DatasetCompression {
    if is_zip(prefix) {
        DatasetCompression::Zip
    } else if prefix.starts_with(&[0x1f, 0x8b]) {
        if is_bgzip(prefix) {
            DatasetCompression::Bgzip
        } else {
            DatasetCompression::Gzip
        }
    } else {
        DatasetCompression::None
    }
}

fn is_zip(prefix: &[u8]) -> bool {
    prefix.starts_with(b"PK\x03\x04")
        || prefix.starts_with(b"PK\x05\x06")
        || prefix.starts_with(b"PK\x07\x08")
}

fn is_bgzip(prefix: &[u8]) -> bool {
    if prefix.len() < 12 || prefix[3] & 0x04 == 0 {
        return false;
    }
    let extra_length = usize::from(u16::from_le_bytes([prefix[10], prefix[11]]));
    let extra_end = 12_usize.saturating_add(extra_length).min(prefix.len());
    let mut offset: usize = 12;
    while offset.saturating_add(4) <= extra_end {
        let subfield_length =
            usize::from(u16::from_le_bytes([prefix[offset + 2], prefix[offset + 3]]));
        let next = offset.saturating_add(4).saturating_add(subfield_length);
        if next > extra_end {
            return false;
        }
        if &prefix[offset..offset + 2] == b"BC" && subfield_length == 2 {
            return true;
        }
        offset = next;
    }
    false
}

fn detect_from_extension(path: &Path) -> Option<Detection> {
    let mut name = display_file_name(path).to_ascii_lowercase();
    for suffix in [".bgzf", ".bgz", ".gz"] {
        if name.ends_with(suffix) {
            name.truncate(name.len() - suffix.len());
            break;
        }
    }
    let extension = Path::new(&name).extension()?.to_str()?;
    let format = match extension {
        "fa" | "fasta" | "fna" | "ffn" | "faa" | "frn" => DatasetFormat::Fasta,
        "fq" | "fastq" => DatasetFormat::Fastq,
        "csv" => DatasetFormat::Csv,
        "tsv" | "tab" => DatasetFormat::Tsv,
        "bed" => DatasetFormat::Bed,
        "gff" | "gff3" => DatasetFormat::Gff3,
        "gtf" => DatasetFormat::Gtf,
        "vcf" => DatasetFormat::Vcf,
        "sam" => DatasetFormat::Sam,
        "bam" => DatasetFormat::Bam,
        "zip" => DatasetFormat::Zip,
        "bcf" => DatasetFormat::Bcf,
        "cram" => DatasetFormat::Cram,
        "h5ad" => DatasetFormat::H5ad,
        "loom" => DatasetFormat::Loom,
        "h5" | "hdf5" => DatasetFormat::Hdf5,
        "rds" | "rda" | "rdata" => DatasetFormat::Rds,
        "pdb" | "ent" => DatasetFormat::Pdb,
        "cif" | "mmcif" => DatasetFormat::Mmcif,
        _ => return None,
    };
    Some(Detection {
        format,
        confidence: DetectionConfidence::Low,
    })
}

fn detect_from_content(prefix: &[u8], path: &Path) -> Option<Detection> {
    if prefix.starts_with(b"BAM\x01") {
        return high(DatasetFormat::Bam);
    }
    if prefix.starts_with(b"BCF\x02") {
        return high(DatasetFormat::Bcf);
    }
    if prefix.starts_with(b"CRAM") {
        return high(DatasetFormat::Cram);
    }
    if prefix.starts_with(b"RDX2\n")
        || prefix.starts_with(b"RDX3\n")
        || prefix.starts_with(b"RDA2\n")
        || prefix.starts_with(b"RDA3\n")
    {
        return high(DatasetFormat::Rds);
    }
    if prefix.starts_with(b"\x89HDF\r\n\x1a\n") {
        return high(
            match detect_from_extension(path).map(|value| value.format) {
                Some(DatasetFormat::H5ad) => DatasetFormat::H5ad,
                Some(DatasetFormat::Loom) => DatasetFormat::Loom,
                _ => DatasetFormat::Hdf5,
            },
        );
    }

    let text = std::str::from_utf8(prefix)
        .ok()?
        .trim_start_matches('\u{feff}');
    let lines = text.lines().collect::<Vec<_>>();
    let nonempty = lines
        .iter()
        .copied()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .take(128)
        .collect::<Vec<_>>();
    let first = *nonempty.first()?;

    if first.starts_with("##fileformat=VCF")
        || nonempty
            .iter()
            .any(|line| line.starts_with("#CHROM\tPOS\tID\tREF\tALT"))
    {
        return high(DatasetFormat::Vcf);
    }
    if first.starts_with("##gff-version 3") {
        return high(DatasetFormat::Gff3);
    }
    if looks_like_fastq(&nonempty) {
        return high(DatasetFormat::Fastq);
    }
    if first.starts_with('>') && nonempty.iter().skip(1).any(|line| !line.starts_with('>')) {
        return high(DatasetFormat::Fasta);
    }
    if looks_like_mmcif(&nonempty) {
        return high(DatasetFormat::Mmcif);
    }
    if looks_like_pdb(&nonempty) {
        return high(DatasetFormat::Pdb);
    }
    if looks_like_sam(&nonempty) {
        return high(DatasetFormat::Sam);
    }
    if let Some(format) = detect_nine_column_annotation(&nonempty, path) {
        return medium(format);
    }
    if looks_like_bed(&nonempty) {
        return medium(DatasetFormat::Bed);
    }

    let csv_score = delimited_score(prefix, b',');
    let tsv_score = delimited_score(prefix, b'\t');
    if csv_score >= 2 || tsv_score >= 2 {
        let preferred = detect_from_extension(path).map(|value| value.format);
        let format = if csv_score == tsv_score {
            match preferred {
                Some(DatasetFormat::Tsv) => DatasetFormat::Tsv,
                _ => DatasetFormat::Csv,
            }
        } else if csv_score > tsv_score {
            DatasetFormat::Csv
        } else {
            DatasetFormat::Tsv
        };
        return medium(format);
    }
    None
}

fn high(format: DatasetFormat) -> Option<Detection> {
    Some(Detection {
        format,
        confidence: DetectionConfidence::High,
    })
}

fn medium(format: DatasetFormat) -> Option<Detection> {
    Some(Detection {
        format,
        confidence: DetectionConfidence::Medium,
    })
}

fn looks_like_fastq(lines: &[&str]) -> bool {
    if lines.first().is_none_or(|line| !line.starts_with('@')) {
        return false;
    }
    let mut sequence_length = 0;
    let mut plus_index = None;
    for (index, line) in lines.iter().enumerate().skip(1) {
        if line.starts_with('+') {
            plus_index = Some(index);
            break;
        }
        sequence_length += line.trim().len();
    }
    let Some(plus_index) = plus_index else {
        return false;
    };
    if sequence_length == 0 {
        return false;
    }
    let quality_length = lines
        .iter()
        .skip(plus_index + 1)
        .map(|line| line.trim_end().len())
        .scan(0_usize, |total, length| {
            *total += length;
            Some(*total)
        })
        .find(|length| *length >= sequence_length);
    quality_length == Some(sequence_length)
}

fn looks_like_sam(lines: &[&str]) -> bool {
    let has_sam_header = lines.iter().any(|line| {
        ["@HD\t", "@SQ\t", "@RG\t", "@PG\t", "@CO\t"]
            .iter()
            .any(|prefix| line.starts_with(prefix))
    });
    let has_alignment = lines.iter().any(|line| {
        if line.starts_with('@') {
            return false;
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        fields.len() >= 11
            && fields[1].parse::<u16>().is_ok()
            && fields[3].parse::<u64>().is_ok()
            && fields[4].parse::<u8>().is_ok()
    });
    has_alignment || (has_sam_header && lines.len() > 1)
}

fn detect_nine_column_annotation(lines: &[&str], path: &Path) -> Option<DatasetFormat> {
    for line in lines.iter().filter(|line| !line.starts_with('#')) {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 9
            || fields[3].parse::<u64>().is_err()
            || fields[4].parse::<u64>().is_err()
        {
            continue;
        }
        if fields[8].contains("gene_id \"") || fields[8].contains("transcript_id \"") {
            return Some(DatasetFormat::Gtf);
        }
        if fields[8].contains('=') {
            return Some(DatasetFormat::Gff3);
        }
        return match detect_from_extension(path).map(|value| value.format) {
            Some(DatasetFormat::Gtf) => Some(DatasetFormat::Gtf),
            Some(DatasetFormat::Gff3) => Some(DatasetFormat::Gff3),
            _ => None,
        };
    }
    None
}

fn looks_like_bed(lines: &[&str]) -> bool {
    let records = lines
        .iter()
        .filter(|line| {
            !line.starts_with('#') && !line.starts_with("track ") && !line.starts_with("browser ")
        })
        .take(4)
        .collect::<Vec<_>>();
    !records.is_empty()
        && records.iter().all(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (3..=12).contains(&fields.len())
                && fields[1].parse::<u64>().is_ok()
                && fields[2].parse::<u64>().is_ok()
        })
}

fn looks_like_pdb(lines: &[&str]) -> bool {
    lines.iter().take(32).any(|line| {
        ["HEADER", "ATOM  ", "HETATM", "MODEL ", "TITLE "]
            .iter()
            .any(|prefix| line.starts_with(prefix))
    })
}

fn looks_like_mmcif(lines: &[&str]) -> bool {
    lines.first().is_some_and(|line| line.starts_with("data_"))
        && lines
            .iter()
            .any(|line| line.starts_with("_atom_site.") || line.starts_with("_entry.id"))
}

fn delimited_score(bytes: &[u8], delimiter: u8) -> usize {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(false)
        .from_reader(bytes);
    let widths = reader
        .byte_records()
        .take(6)
        .filter_map(Result::ok)
        .map(|record| record.len())
        .collect::<Vec<_>>();
    if widths.len() < 2 || widths[0] < 2 {
        return 0;
    }
    widths.iter().filter(|width| **width == widths[0]).count()
}

fn resolve_detection(
    content: Option<Detection>,
    extension: Option<Detection>,
) -> (Detection, Vec<InspectionIssue>) {
    match (content, extension) {
        (Some(content), Some(extension)) if content.format == extension.format => (
            Detection {
                format: content.format,
                confidence: DetectionConfidence::High,
            },
            Vec::new(),
        ),
        (Some(content), Some(extension)) if content.format != extension.format => (
            content,
            vec![issue(
                "format-extension-mismatch",
                format!(
                    "content identifies {} but the file extension suggests {}",
                    content.format, extension.format
                ),
                None,
            )],
        ),
        (Some(content), _) => (content, Vec::new()),
        (None, Some(extension)) => (
            extension,
            vec![issue(
                "extension-only-detection",
                format!(
                    "{} was inferred from the file extension because no conclusive content signature was found",
                    extension.format
                ),
                None,
            )],
        ),
        (None, None) => (
            Detection {
                format: DatasetFormat::Unknown,
                confidence: DetectionConfidence::None,
            },
            vec![issue(
                "unknown-format",
                "the dataset format could not be identified",
                None,
            )],
        ),
    }
}

fn support_for(format: DatasetFormat) -> DatasetSupport {
    match format {
        DatasetFormat::Fasta
        | DatasetFormat::Fastq
        | DatasetFormat::Csv
        | DatasetFormat::Tsv
        | DatasetFormat::Bed
        | DatasetFormat::Gff3
        | DatasetFormat::Gtf
        | DatasetFormat::Vcf
        | DatasetFormat::Sam => DatasetSupport::Supported,
        DatasetFormat::Bam
        | DatasetFormat::Zip
        | DatasetFormat::Bcf
        | DatasetFormat::Cram
        | DatasetFormat::H5ad
        | DatasetFormat::Loom
        | DatasetFormat::Hdf5
        | DatasetFormat::Rds
        | DatasetFormat::Pdb
        | DatasetFormat::Mmcif => DatasetSupport::RecognizedUnsupported,
        DatasetFormat::Unknown => DatasetSupport::Unknown,
    }
}

fn build_preview(
    path: &Path,
    compression: DatasetCompression,
    format: DatasetFormat,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    match format {
        DatasetFormat::Fasta => preview_fasta(path, compression, options),
        DatasetFormat::Fastq => preview_fastq(path, compression, options),
        DatasetFormat::Csv => preview_csv(path, compression, options, b','),
        DatasetFormat::Tsv => preview_csv(path, compression, options, b'\t'),
        DatasetFormat::Bed => preview_bed(path, compression, options),
        DatasetFormat::Gff3 | DatasetFormat::Gtf => preview_annotation(path, compression, options),
        DatasetFormat::Vcf => preview_vcf(path, compression, options),
        DatasetFormat::Sam => preview_sam(path, compression, options),
        DatasetFormat::Bam
        | DatasetFormat::Bcf
        | DatasetFormat::Cram
        | DatasetFormat::H5ad
        | DatasetFormat::Loom
        | DatasetFormat::Hdf5
        | DatasetFormat::Rds
        | DatasetFormat::Zip => Ok(PreviewOutcome {
            preview: preview_binary(path, compression, options.max_preview_bytes)?,
            warnings: Vec::new(),
            errors: Vec::new(),
        }),
        DatasetFormat::Pdb | DatasetFormat::Mmcif | DatasetFormat::Unknown => {
            preview_text(path, compression, options)
        }
    }
}

fn preview_fasta(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    let mut reader = LimitedLines::new(open_payload(path, compression)?, options.max_preview_bytes);
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut current: Option<(String, String, usize, String)> = None;
    let mut extra_record_seen = false;

    while let Some(line) = reader.next_line()? {
        let text = decode_line(&line.bytes, line.number, &mut errors);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('>') {
            if let Some(record) = current.take() {
                records.push(fasta_row(record));
                if records.len() == options.max_preview_records {
                    extra_record_seen = true;
                    break;
                }
            }
            let mut parts = header.splitn(2, char::is_whitespace);
            let identifier = parts.next().unwrap_or_default().to_owned();
            if identifier.is_empty() {
                errors.push(issue(
                    "empty-fasta-identifier",
                    "FASTA header has no identifier",
                    Some(line.number),
                ));
            }
            current = Some((
                identifier,
                parts.next().unwrap_or_default().trim().to_owned(),
                0,
                String::new(),
            ));
        } else if let Some((_, _, length, excerpt)) = current.as_mut() {
            for byte in trimmed.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
                *length += 1;
                if excerpt.len() < SEQUENCE_EXCERPT_LENGTH {
                    excerpt.push(char::from(byte));
                }
            }
        } else {
            errors.push(issue(
                "fasta-sequence-before-header",
                "sequence data appears before the first FASTA header",
                Some(line.number),
            ));
        }
    }
    if !extra_record_seen
        && records.len() < options.max_preview_records
        && let Some(record) = current
    {
        records.push(fasta_row(record));
    }
    let truncated = extra_record_seen || reader.hit_byte_limit();
    Ok(PreviewOutcome {
        preview: tabular_preview(
            PreviewKind::Sequence,
            ["identifier", "description", "length", "sequence_excerpt"],
            records,
            reader.bytes_read(),
            truncated,
        ),
        warnings: Vec::new(),
        errors,
    })
}

fn fasta_row(record: (String, String, usize, String)) -> Vec<String> {
    vec![record.0, record.1, record.2.to_string(), record.3]
}

fn preview_fastq(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    let mut reader = LimitedLines::new(open_payload(path, compression)?, options.max_preview_bytes);
    let mut records = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    while let Some(header_line) = next_nonempty_line(&mut reader, &mut errors)? {
        let header = decode_line(&header_line.bytes, header_line.number, &mut errors);
        let Some(header) = header.trim_end().strip_prefix('@') else {
            errors.push(issue(
                "invalid-fastq-header",
                "FASTQ record must begin with an @ header",
                Some(header_line.number),
            ));
            break;
        };
        let mut sequence = String::new();
        let plus_line_number;
        loop {
            let Some(line) = reader.next_line()? else {
                errors.push(issue(
                    "truncated-fastq-sequence",
                    "FASTQ record ended before the + separator",
                    Some(header_line.number),
                ));
                return Ok(fastq_outcome(reader, records, warnings, errors, true));
            };
            let text = decode_line(&line.bytes, line.number, &mut errors);
            if text.starts_with('+') {
                plus_line_number = line.number;
                break;
            }
            sequence.push_str(text.trim_end());
        }
        if sequence.is_empty() {
            warnings.push(issue(
                "empty-fastq-sequence",
                "FASTQ record has an empty sequence",
                Some(header_line.number),
            ));
        }
        let mut quality = String::new();
        while quality.len() < sequence.len() {
            let Some(line) = reader.next_line()? else {
                errors.push(issue(
                    "truncated-fastq-quality",
                    "FASTQ quality data is shorter than the sequence",
                    Some(plus_line_number),
                ));
                return Ok(fastq_outcome(reader, records, warnings, errors, true));
            };
            let text = decode_line(&line.bytes, line.number, &mut errors);
            quality.push_str(text.trim_end());
        }
        if quality.len() != sequence.len() {
            errors.push(issue(
                "fastq-length-mismatch",
                format!(
                    "FASTQ sequence has {} symbols but quality has {}",
                    sequence.len(),
                    quality.len()
                ),
                Some(header_line.number),
            ));
        }
        let mut header_parts = header.splitn(2, char::is_whitespace);
        records.push(vec![
            header_parts.next().unwrap_or_default().to_owned(),
            header_parts.next().unwrap_or_default().trim().to_owned(),
            sequence.len().to_string(),
            excerpt(&sequence, SEQUENCE_EXCERPT_LENGTH),
            excerpt(&quality, SEQUENCE_EXCERPT_LENGTH),
        ]);
        if records.len() == options.max_preview_records {
            let more = reader.next_line()?.is_some();
            return Ok(fastq_outcome(reader, records, warnings, errors, more));
        }
    }
    Ok(fastq_outcome(reader, records, warnings, errors, false))
}

fn fastq_outcome(
    reader: LimitedLines<Box<dyn Read>>,
    records: Vec<Vec<String>>,
    warnings: Vec<InspectionIssue>,
    errors: Vec<InspectionIssue>,
    extra_record_or_error: bool,
) -> PreviewOutcome {
    let truncated = extra_record_or_error || reader.hit_byte_limit();
    PreviewOutcome {
        preview: tabular_preview(
            PreviewKind::Sequence,
            [
                "identifier",
                "description",
                "length",
                "sequence_excerpt",
                "quality_excerpt",
            ],
            records,
            reader.bytes_read(),
            truncated,
        ),
        warnings,
        errors,
    }
}

fn preview_csv(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
    delimiter: u8,
) -> io::Result<PreviewOutcome> {
    let source = open_payload(path, compression)?.take(options.max_preview_bytes);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(source);
    let mut raw_records = Vec::new();
    let mut errors = Vec::new();
    let mut record_limit_hit = false;
    for result in reader.byte_records() {
        match result {
            Ok(record) => {
                if raw_records.len() == options.max_preview_records + 1 {
                    record_limit_hit = true;
                    break;
                }
                raw_records.push(
                    record
                        .iter()
                        .map(|field| String::from_utf8_lossy(field).into_owned())
                        .collect::<Vec<_>>(),
                );
            }
            Err(error) => {
                errors.push(issue(
                    "invalid-delimited-record",
                    error.to_string(),
                    error.position().map(|position| position.line()),
                ));
                break;
            }
        }
    }
    let bytes_read = reader.position().byte();
    let byte_limit_hit = bytes_read >= options.max_preview_bytes;
    let width = raw_records.iter().map(Vec::len).max().unwrap_or(0);
    let has_header = raw_records.len() >= 2 && looks_like_header(&raw_records[0], &raw_records[1]);
    let columns = if has_header {
        raw_records.remove(0)
    } else {
        (1..=width).map(|index| format!("column_{index}")).collect()
    };
    if raw_records.len() > options.max_preview_records {
        raw_records.truncate(options.max_preview_records);
        record_limit_hit = true;
    }
    Ok(PreviewOutcome {
        preview: DatasetPreview {
            kind: PreviewKind::Table,
            columns,
            records_shown: raw_records.len(),
            records: raw_records,
            bytes_read,
            truncated: byte_limit_hit || record_limit_hit,
        },
        warnings: Vec::new(),
        errors,
    })
}

fn looks_like_header(first: &[String], second: &[String]) -> bool {
    if first.is_empty()
        || first.iter().any(|value| value.trim().is_empty())
        || first.len() != second.len()
    {
        return false;
    }
    let first_unique = first
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == first.len();
    let typed_difference = first.iter().zip(second).any(|(left, right)| {
        !looks_numeric(left) && (looks_numeric(right) || right.eq_ignore_ascii_case("true"))
    });
    first_unique && typed_difference
}

fn looks_numeric(value: &str) -> bool {
    value.trim().parse::<f64>().is_ok()
}

fn preview_bed(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    let (mut outcome, source_lines) =
        preview_tab_lines(path, compression, options, PreviewKind::Table, |line| {
            !line.starts_with('#') && !line.starts_with("track ") && !line.starts_with("browser ")
        })?;
    let max_width = outcome
        .preview
        .records
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(3);
    let names = [
        "chrom",
        "chrom_start",
        "chrom_end",
        "name",
        "score",
        "strand",
        "thick_start",
        "thick_end",
        "item_rgb",
        "block_count",
        "block_sizes",
        "block_starts",
    ];
    outcome.preview.columns = names
        .iter()
        .take(max_width.min(names.len()))
        .map(|name| (*name).to_owned())
        .collect();
    for (record, source_line) in outcome.preview.records.iter().zip(source_lines) {
        if record.len() < 3
            || record[1].parse::<u64>().is_err()
            || record[2].parse::<u64>().is_err()
        {
            outcome.errors.push(issue(
                "invalid-bed-record",
                "BED records require chromosome, integer start, and integer end fields",
                Some(source_line),
            ));
        }
    }
    Ok(outcome)
}

fn preview_annotation(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    let (mut outcome, source_lines) =
        preview_tab_lines(path, compression, options, PreviewKind::Table, |line| {
            !line.starts_with('#')
        })?;
    outcome.preview.columns = strings([
        "seqid",
        "source",
        "type",
        "start",
        "end",
        "score",
        "strand",
        "phase",
        "attributes",
    ]);
    for (record, source_line) in outcome.preview.records.iter().zip(source_lines) {
        if record.len() != 9
            || record
                .get(3)
                .is_none_or(|value| value.parse::<u64>().is_err())
            || record
                .get(4)
                .is_none_or(|value| value.parse::<u64>().is_err())
        {
            outcome.errors.push(issue(
                "invalid-annotation-record",
                "GFF/GTF records require nine fields and integer start/end coordinates",
                Some(source_line),
            ));
        }
    }
    Ok(outcome)
}

fn preview_vcf(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    let mut reader = LimitedLines::new(open_payload(path, compression)?, options.max_preview_bytes);
    let mut columns = strings(["CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER", "INFO"]);
    let mut records = Vec::new();
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut extra = false;
    let mut saw_fileformat = false;
    while let Some(line) = reader.next_line()? {
        let text = decode_line(&line.bytes, line.number, &mut errors);
        let trimmed = text.trim_end();
        if trimmed.starts_with("##fileformat=VCF") {
            saw_fileformat = true;
            continue;
        }
        if trimmed.starts_with("##") || trimmed.is_empty() {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('#') {
            columns = header.split('\t').map(str::to_owned).collect();
            continue;
        }
        let fields = trimmed.split('\t').map(str::to_owned).collect::<Vec<_>>();
        if fields.len() < 8 || fields[1].parse::<u64>().is_err() {
            errors.push(issue(
                "invalid-vcf-record",
                "VCF records require at least eight fields and an integer POS value",
                Some(line.number),
            ));
        }
        records.push(fields);
        if records.len() == options.max_preview_records {
            extra = reader.next_line()?.is_some();
            break;
        }
    }
    if !saw_fileformat {
        warnings.push(issue(
            "missing-vcf-fileformat",
            "VCF preview does not contain a ##fileformat declaration",
            None,
        ));
    }
    Ok(PreviewOutcome {
        preview: DatasetPreview {
            kind: PreviewKind::Variant,
            columns,
            records_shown: records.len(),
            records,
            bytes_read: reader.bytes_read(),
            truncated: extra || reader.hit_byte_limit(),
        },
        warnings,
        errors,
    })
}

fn preview_sam(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    let mut reader = LimitedLines::new(open_payload(path, compression)?, options.max_preview_bytes);
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut extra = false;
    while let Some(line) = reader.next_line()? {
        let text = decode_line(&line.bytes, line.number, &mut errors);
        let trimmed = text.trim_end();
        if trimmed.is_empty() || trimmed.starts_with('@') {
            continue;
        }
        let mut fields = trimmed.split('\t').map(str::to_owned).collect::<Vec<_>>();
        if fields.len() < 11
            || fields[1].parse::<u16>().is_err()
            || fields[3].parse::<u64>().is_err()
            || fields[4].parse::<u8>().is_err()
        {
            errors.push(issue(
                "invalid-sam-record",
                "SAM records require eleven core fields with numeric FLAG, POS, and MAPQ values",
                Some(line.number),
            ));
        }
        if fields.len() > 11 {
            let optional = fields.split_off(11).join("\t");
            fields.push(optional);
        }
        records.push(fields);
        if records.len() == options.max_preview_records {
            extra = reader.next_line()?.is_some();
            break;
        }
    }
    Ok(PreviewOutcome {
        preview: tabular_preview(
            PreviewKind::Alignment,
            [
                "QNAME", "FLAG", "RNAME", "POS", "MAPQ", "CIGAR", "RNEXT", "PNEXT", "TLEN", "SEQ",
                "QUAL", "OPTIONAL",
            ],
            records,
            reader.bytes_read(),
            extra || reader.hit_byte_limit(),
        ),
        warnings: Vec::new(),
        errors,
    })
}

fn preview_tab_lines<F>(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
    kind: PreviewKind,
    include: F,
) -> io::Result<(PreviewOutcome, Vec<u64>)>
where
    F: Fn(&str) -> bool,
{
    let mut reader = LimitedLines::new(open_payload(path, compression)?, options.max_preview_bytes);
    let mut records = Vec::new();
    let mut source_lines = Vec::new();
    let mut errors = Vec::new();
    let mut extra = false;
    while let Some(line) = reader.next_line()? {
        let text = decode_line(&line.bytes, line.number, &mut errors);
        let trimmed = text.trim_end();
        if trimmed.is_empty() || !include(trimmed) {
            continue;
        }
        records.push(trimmed.split('\t').map(str::to_owned).collect());
        source_lines.push(line.number);
        if records.len() == options.max_preview_records {
            extra = reader.next_line()?.is_some();
            break;
        }
    }
    let width = records.iter().map(Vec::len).max().unwrap_or(0);
    Ok((
        PreviewOutcome {
            preview: DatasetPreview {
                kind,
                columns: (1..=width).map(|index| format!("column_{index}")).collect(),
                records_shown: records.len(),
                records,
                bytes_read: reader.bytes_read(),
                truncated: extra || reader.hit_byte_limit(),
            },
            warnings: Vec::new(),
            errors,
        },
        source_lines,
    ))
}

fn preview_text(
    path: &Path,
    compression: DatasetCompression,
    options: DatasetInspectionOptions,
) -> io::Result<PreviewOutcome> {
    let mut reader = LimitedLines::new(open_payload(path, compression)?, options.max_preview_bytes);
    let mut records = Vec::new();
    let mut errors = Vec::new();
    let mut extra = false;
    while let Some(line) = reader.next_line()? {
        let text = decode_line(&line.bytes, line.number, &mut errors);
        records.push(vec![
            line.number.to_string(),
            excerpt(text.trim_end(), TEXT_EXCERPT_LENGTH),
        ]);
        if records.len() == options.max_preview_records {
            extra = reader.next_line()?.is_some();
            break;
        }
    }
    Ok(PreviewOutcome {
        preview: tabular_preview(
            PreviewKind::Text,
            ["line", "content"],
            records,
            reader.bytes_read(),
            extra || reader.hit_byte_limit(),
        ),
        warnings: Vec::new(),
        errors,
    })
}

fn preview_binary(
    path: &Path,
    compression: DatasetCompression,
    byte_limit: u64,
) -> io::Result<DatasetPreview> {
    let mut input: Box<dyn Read> = if compression == DatasetCompression::Zip {
        Box::new(File::open(path)?)
    } else {
        open_payload(path, compression)?
    };
    let probe_limit = byte_limit.saturating_add(1);
    let prefix_limit =
        usize::try_from(byte_limit.min(32)).expect("binary preview prefix length fits in usize");
    let mut prefix = Vec::with_capacity(prefix_limit);
    let mut bytes_seen = 0_u64;
    let mut buffer = [0_u8; 8 * 1024];

    while bytes_seen < probe_limit {
        let remaining = probe_limit - bytes_seen;
        let read_length = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("binary preview buffer length fits in usize");
        let count = input.read(&mut buffer[..read_length])?;
        if count == 0 {
            break;
        }
        let prefix_remaining = prefix_limit.saturating_sub(prefix.len());
        prefix.extend_from_slice(&buffer[..count.min(prefix_remaining)]);
        bytes_seen = bytes_seen.saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
    }

    Ok(binary_preview(
        &prefix,
        bytes_seen.min(byte_limit),
        bytes_seen > byte_limit,
    ))
}

fn binary_preview(prefix: &[u8], bytes_read: u64, truncated: bool) -> DatasetPreview {
    let magic = prefix
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    DatasetPreview {
        kind: PreviewKind::Binary,
        columns: strings(["property", "value"]),
        records: vec![vec!["magic_bytes".to_owned(), magic]],
        records_shown: 1,
        bytes_read,
        truncated,
    }
}

fn tabular_preview<const N: usize>(
    kind: PreviewKind,
    columns: [&str; N],
    records: Vec<Vec<String>>,
    bytes_read: u64,
    truncated: bool,
) -> DatasetPreview {
    DatasetPreview {
        kind,
        columns: strings(columns),
        records_shown: records.len(),
        records,
        bytes_read,
        truncated,
    }
}

fn strings<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(str::to_owned).collect()
}

fn issue(code: &str, message: impl Into<String>, line: Option<u64>) -> InspectionIssue {
    InspectionIssue {
        code: code.to_owned(),
        message: message.into(),
        line,
    }
}

fn excerpt(value: &str, max_chars: usize) -> String {
    let mut result = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        result.push_str("...");
    }
    result
}

#[derive(Debug)]
struct LimitedLine {
    number: u64,
    bytes: Vec<u8>,
}

struct LimitedLines<R: Read> {
    reader: BufReader<R>,
    max_bytes: u64,
    bytes_read: u64,
    line_number: u64,
    hit_byte_limit: bool,
}

impl<R: Read> LimitedLines<R> {
    fn new(reader: R, max_bytes: u64) -> Self {
        Self {
            reader: BufReader::new(reader),
            max_bytes,
            bytes_read: 0,
            line_number: 0,
            hit_byte_limit: false,
        }
    }

    fn next_line(&mut self) -> io::Result<Option<LimitedLine>> {
        if self.bytes_read >= self.max_bytes {
            self.hit_byte_limit = true;
            return Ok(None);
        }
        let remaining = self.max_bytes - self.bytes_read;
        let mut bytes = Vec::new();
        let count = self
            .reader
            .by_ref()
            .take(remaining.saturating_add(1))
            .read_until(b'\n', &mut bytes)?;
        if count == 0 {
            return Ok(None);
        }
        if u64::try_from(count).unwrap_or(u64::MAX) > remaining {
            self.bytes_read = self.max_bytes;
            self.hit_byte_limit = true;
            return Ok(None);
        }
        self.bytes_read += u64::try_from(count).unwrap_or(u64::MAX);
        self.line_number += 1;
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        Ok(Some(LimitedLine {
            number: self.line_number,
            bytes,
        }))
    }

    fn bytes_read(&self) -> u64 {
        self.bytes_read
    }

    fn hit_byte_limit(&self) -> bool {
        self.hit_byte_limit
    }
}

fn next_nonempty_line<R: Read>(
    reader: &mut LimitedLines<R>,
    errors: &mut Vec<InspectionIssue>,
) -> io::Result<Option<LimitedLine>> {
    while let Some(line) = reader.next_line()? {
        let text = decode_line(&line.bytes, line.number, errors);
        if !text.trim().is_empty() {
            return Ok(Some(line));
        }
    }
    Ok(None)
}

fn decode_line(bytes: &[u8], line: u64, errors: &mut Vec<InspectionIssue>) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            if !errors.iter().any(|error| error.code == "invalid-utf8") {
                errors.push(issue(
                    "invalid-utf8",
                    "text preview contains invalid UTF-8 and was decoded lossily",
                    Some(line),
                ));
            }
            String::from_utf8_lossy(bytes).into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DatasetCompression, DatasetFormat, DatasetInspectionOptions, DatasetSupport,
        DetectionConfidence, PreviewKind, detect_compression, inspect_dataset,
        inspect_dataset_with_options,
    };
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn detects_and_previews_supported_text_formats() {
        let cases = [
            ("sequences.fa", DatasetFormat::Fasta, PreviewKind::Sequence),
            ("reads.fastq", DatasetFormat::Fastq, PreviewKind::Sequence),
            ("table.csv", DatasetFormat::Csv, PreviewKind::Table),
            ("matrix.tsv", DatasetFormat::Tsv, PreviewKind::Table),
            ("regions.bed", DatasetFormat::Bed, PreviewKind::Table),
            ("annotations.gff3", DatasetFormat::Gff3, PreviewKind::Table),
            ("transcripts.gtf", DatasetFormat::Gtf, PreviewKind::Table),
            ("variants.vcf", DatasetFormat::Vcf, PreviewKind::Variant),
            ("alignments.sam", DatasetFormat::Sam, PreviewKind::Alignment),
        ];

        for (name, format, kind) in cases {
            let inspection = inspect_dataset(fixture(name)).expect("inspect fixture");
            assert_eq!(inspection.format, format, "fixture {name}");
            assert_eq!(
                inspection.support,
                DatasetSupport::Supported,
                "fixture {name}"
            );
            assert_eq!(
                inspection.confidence,
                DetectionConfidence::High,
                "fixture {name}"
            );
            assert_eq!(
                inspection.preview.as_ref().map(|preview| preview.kind),
                Some(kind),
                "fixture {name}"
            );
            assert!(
                inspection.errors.is_empty(),
                "fixture {name}: {:?}",
                inspection.errors
            );
        }
    }

    #[test]
    fn parses_quoted_and_multiline_csv_fields() {
        let inspection = inspect_dataset(fixture("table.csv")).expect("CSV inspection");
        let preview = inspection.preview.expect("CSV preview");

        assert_eq!(preview.columns, ["sample", "value", "note"]);
        assert_eq!(preview.records.len(), 2);
        assert_eq!(preview.records[0][2], "alpha,beta");
        assert_eq!(preview.records[1][2], "two\nlines");
    }

    #[test]
    fn supports_wrapped_fastq_records() {
        let inspection = inspect_dataset(fixture("reads.fastq")).expect("FASTQ inspection");
        let preview = inspection.preview.expect("FASTQ preview");

        assert_eq!(preview.records.len(), 2);
        assert_eq!(preview.records[0][2], "8");
        assert_eq!(preview.records[0][3], "ACGTTGCA");
        assert!(inspection.errors.is_empty());
    }

    #[test]
    fn decompresses_gzip_before_detecting_content() {
        let path = temporary_path("reads.fastq.gz");
        let file = fs::File::create(&path).expect("create gzip fixture");
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder
            .write_all(b"@read\nACGT\n+\nIIII\n")
            .expect("write gzip payload");
        encoder.finish().expect("finish gzip payload");

        let inspection = inspect_dataset(&path).expect("inspect gzip");
        fs::remove_file(&path).expect("remove gzip fixture");

        assert_eq!(inspection.compression, DatasetCompression::Gzip);
        assert_eq!(inspection.format, DatasetFormat::Fastq);
        assert_eq!(inspection.preview.expect("preview").records_shown, 1);
    }

    #[test]
    fn recognizes_bgzip_extra_field() {
        let header = [
            0x1f, 0x8b, 0x08, 0x04, 0, 0, 0, 0, 0, 0xff, 0x06, 0x00, b'B', b'C', 0x02, 0x00, 0x1b,
            0x00,
        ];
        assert_eq!(detect_compression(&header), DatasetCompression::Bgzip);
    }

    #[test]
    fn recognizes_binary_signatures_and_marks_planned_formats_unsupported() {
        let cases: [(&str, &[u8], DatasetFormat); 7] = [
            ("sample.bam", b"BAM\x01rest", DatasetFormat::Bam),
            ("sample.bcf", b"BCF\x02\x02rest", DatasetFormat::Bcf),
            ("sample.cram", b"CRAM\x03\x00rest", DatasetFormat::Cram),
            ("sample.rds", b"RDX3\nX\nrest", DatasetFormat::Rds),
            ("sample.h5ad", b"\x89HDF\r\n\x1a\nrest", DatasetFormat::H5ad),
            ("sample.loom", b"\x89HDF\r\n\x1a\nrest", DatasetFormat::Loom),
            ("sample.h5", b"\x89HDF\r\n\x1a\nrest", DatasetFormat::Hdf5),
        ];
        for (name, bytes, expected) in cases {
            let path = write_temporary(name, bytes);
            let inspection = inspect_dataset(&path).expect("inspect binary signature");
            fs::remove_file(&path).expect("remove binary fixture");
            assert_eq!(inspection.format, expected, "{name}");
            assert_eq!(inspection.confidence, DetectionConfidence::High, "{name}");
            assert_eq!(
                inspection.support,
                DatasetSupport::RecognizedUnsupported,
                "{name}"
            );
            assert_eq!(
                inspection.preview.as_ref().map(|preview| preview.kind),
                Some(PreviewKind::Binary),
                "{name}"
            );
        }
    }

    #[test]
    fn binary_preview_truncation_uses_the_payload_size() {
        let mut payload = vec![0_u8; 256];
        payload[..4].copy_from_slice(b"BAM\x01");
        let path = write_temporary("large.bam", &payload);
        let inspection = inspect_dataset_with_options(
            &path,
            DatasetInspectionOptions {
                max_preview_records: 200,
                max_preview_bytes: 128,
            },
        )
        .expect("inspect byte-capped BAM");
        fs::remove_file(&path).expect("remove BAM fixture");

        let preview = inspection.preview.expect("binary preview");
        assert_eq!(preview.kind, PreviewKind::Binary);
        assert_eq!(preview.bytes_read, 128);
        assert!(preview.truncated);
    }

    #[test]
    fn genomic_table_errors_keep_source_line_numbers_after_comments() {
        let cases: [(&str, &[u8], &str, u64); 3] = [
            (
                "invalid.bed",
                b"# comment\ntrack name=example\nbrowser position chr1\n\nchr1\tbad\t10\n",
                "invalid-bed-record",
                5,
            ),
            (
                "invalid.gff3",
                b"##gff-version 3\n# comment\n\nchr1\tsource\tgene\tbad\t10\t.\t+\t.\tID=gene1\n",
                "invalid-annotation-record",
                4,
            ),
            (
                "invalid.gtf",
                b"# comment\n# another comment\n\nchr1\tsource\tgene\tbad\t10\t.\t+\t.\tgene_id \"gene1\";\n",
                "invalid-annotation-record",
                4,
            ),
        ];

        for (name, contents, code, expected_line) in cases {
            let path = write_temporary(name, contents);
            let inspection = inspect_dataset(&path).expect("inspect invalid genomic table");
            fs::remove_file(&path).expect("remove genomic table fixture");

            let error = inspection
                .errors
                .iter()
                .find(|error| error.code == code)
                .unwrap_or_else(|| panic!("missing {code} diagnostic for {name}"));
            assert_eq!(error.line, Some(expected_line), "{name}");
        }
    }

    #[test]
    fn recognizes_zip_without_attempting_to_extract_it() {
        let path = write_temporary("archive.zip", b"PK\x03\x04payload");
        let inspection = inspect_dataset(&path).expect("inspect ZIP");
        fs::remove_file(&path).expect("remove ZIP fixture");

        assert_eq!(inspection.format, DatasetFormat::Zip);
        assert_eq!(inspection.compression, DatasetCompression::Zip);
        assert_eq!(inspection.support, DatasetSupport::RecognizedUnsupported);
        assert!(
            inspection
                .warnings
                .iter()
                .any(|warning| warning.code == "unsupported-archive")
        );
    }

    #[test]
    fn recognizes_structural_text_formats_but_does_not_claim_import_support() {
        for (name, expected) in [
            ("structure.pdb", DatasetFormat::Pdb),
            ("structure.cif", DatasetFormat::Mmcif),
        ] {
            let inspection = inspect_dataset(fixture(name)).expect("inspect structure fixture");
            assert_eq!(inspection.format, expected);
            assert_eq!(inspection.support, DatasetSupport::RecognizedUnsupported);
            assert_eq!(
                inspection.preview.expect("text preview").kind,
                PreviewKind::Text
            );
        }
    }

    #[test]
    fn content_wins_over_a_misleading_extension() {
        let path = write_temporary("misnamed.csv", b">sequence\nACGT\n");
        let inspection = inspect_dataset(&path).expect("inspect mismatched extension");
        fs::remove_file(&path).expect("remove temporary fixture");

        assert_eq!(inspection.format, DatasetFormat::Fasta);
        assert!(
            inspection
                .warnings
                .iter()
                .any(|warning| warning.code == "format-extension-mismatch")
        );
    }

    #[test]
    fn caps_preview_by_record_count() {
        let path = temporary_path("many.fa");
        let mut file = fs::File::create(&path).expect("create many-record fixture");
        for index in 0..5 {
            writeln!(file, ">record-{index}\nACGT").expect("write FASTA record");
        }
        drop(file);
        let inspection = inspect_dataset_with_options(
            &path,
            DatasetInspectionOptions {
                max_preview_records: 2,
                max_preview_bytes: 1024,
            },
        )
        .expect("inspect capped FASTA");
        fs::remove_file(&path).expect("remove many-record fixture");

        let preview = inspection.preview.expect("preview");
        assert_eq!(preview.records_shown, 2);
        assert!(preview.truncated);
        assert!(preview.bytes_read < 1024);
    }

    #[test]
    fn caps_preview_by_uncompressed_byte_count() {
        let path = write_temporary("long.fa", b">long\nACGTACGTACGTACGTACGT\n>second\nAA\n");
        let inspection = inspect_dataset_with_options(
            &path,
            DatasetInspectionOptions {
                max_preview_records: 200,
                max_preview_bytes: 12,
            },
        )
        .expect("inspect byte-capped FASTA");
        fs::remove_file(&path).expect("remove long FASTA fixture");

        let preview = inspection.preview.expect("preview");
        assert!(preview.truncated);
        assert!(preview.bytes_read <= 12);
    }

    #[test]
    fn reports_truncated_fastq_as_a_structured_error() {
        let path = write_temporary("truncated.fastq", b"@read\nACGT\n+\nII\n");
        let inspection = inspect_dataset(&path).expect("inspect invalid FASTQ");
        fs::remove_file(&path).expect("remove invalid FASTQ fixture");

        assert_eq!(inspection.format, DatasetFormat::Fastq);
        assert!(
            inspection
                .errors
                .iter()
                .any(|error| error.code == "truncated-fastq-quality")
        );
    }

    #[test]
    fn reports_empty_and_unknown_files_without_panicking() {
        let empty = write_temporary("empty.dat", b"");
        let inspection = inspect_dataset(&empty).expect("inspect empty file");
        fs::remove_file(&empty).expect("remove empty file");
        assert_eq!(inspection.format, DatasetFormat::Unknown);
        assert_eq!(inspection.support, DatasetSupport::Unknown);
        assert!(
            inspection
                .errors
                .iter()
                .any(|error| error.code == "empty-file")
        );

        let unknown = write_temporary("unknown.dat", b"one opaque line\n");
        let inspection = inspect_dataset(&unknown).expect("inspect unknown file");
        fs::remove_file(&unknown).expect("remove unknown file");
        assert_eq!(inspection.format, DatasetFormat::Unknown);
        assert_eq!(
            inspection.preview.expect("text preview").kind,
            PreviewKind::Text
        );
    }

    #[test]
    fn rejects_zero_preview_limits() {
        let error = inspect_dataset_with_options(
            fixture("sequences.fa"),
            DatasetInspectionOptions {
                max_preview_records: 0,
                max_preview_bytes: 1,
            },
        )
        .expect_err("zero record limit must fail");
        assert!(error.to_string().contains("greater than zero"));
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/data-inspection")
            .join(name)
    }

    fn write_temporary(name: &str, contents: &[u8]) -> PathBuf {
        let path = temporary_path(name);
        fs::write(&path, contents).expect("write temporary dataset");
        path
    }

    fn temporary_path(name: &str) -> PathBuf {
        let counter = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-bio-dataset-{}-{counter}-{name}",
            std::process::id()
        ))
    }
}
