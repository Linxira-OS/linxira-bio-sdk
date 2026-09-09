#![forbid(unsafe_code)]

//! Unified output framework for Linxira Bio.
//!
//! The authoritative analysis result is always the V2 JSON envelope;
//! traditional bioinformatics formats (FASTA, FASTQ, BED, GFF3, GTF, VCF,
//! SAM) are *reversible projections*: [`BioDataWriter`] renders them from
//! canonical JSON records, and [`BioDataReader`] parses them back into the
//! same canonical shape. Every writer persists its bytes atomically through
//! `linxira_bio_export::write_atomic_bytes`, so consumers never observe a
//! partially written artifact.
//!
//! # Canonical record shapes
//!
//! Writers and readers share one record convention. Input JSON may be a bare
//! array of record objects, an object with a `records` array, or a V1/V2
//! result envelope whose `result` follows either shape.
//!
//! | Format | Record fields |
//! |---|---|
//! | FASTA | `id` (required), `description`, `sequence` (required) |
//! | FASTQ | `id` (required), `description`, `sequence` (required), `quality` (required) |
//! | BED | `chrom`, `start`, `end` (required), then `name`, `score`, `strand`, `thickStart`, `thickEnd`, `itemRgb`, `blockCount`, `blockSizes`, `blockStarts` |
//! | GFF3 | `seqid`, `source`, `type`, `start`, `end`, `score`, `strand`, `phase`, `attributes` |
//! | GTF | `seqid`, `source`, `type`, `start`, `end`, `score`, `strand`, `frame`, `attributes` |
//! | VCF | `chrom`, `pos`, `id`, `ref`, `alt`, `qual`, `filter`, `info`, `format`, `samples` |
//! | SAM | `qname`, `flag`, `rname`, `pos`, `mapq`, `cigar`, `rnext`, `pnext`, `tlen`, `seq`, `qual`, `tags` |
//!
//! Interval coordinates inside JSON are always **1-based closed** to match
//! the engine's capability results. [`CoordinateSystem`] selects the on-disk
//! convention when writing; each interval format defaults to its native one
//! (BED is 0-based half-open, GFF3/GTF/VCF/SAM are 1-based). Array-valued
//! ALT alleles, GFF3 attributes, and VCF `info` values render comma-joined
//! and read back as plain strings, so canonical round-trips use scalar
//! values for those fields.

mod path_resolver;
mod plot;
mod readers;
mod writers;

pub use path_resolver::{
    classify_output_dir, resolve_input_path, timestamp_suffix, workspace_root,
};
pub use plot::{PlotFigure, PlotFont, PlotOutput, PlotOutputFormat, PlotSpec, PlotTheme};
pub use readers::BioDataReader;
pub use writers::BioDataWriter;

use linxira_bio_protocol::BioDataFormat;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

/// Schema version of the [`OutputSpec`] document.
pub const OUTPUT_SPEC_SCHEMA_VERSION: &str = "1";

/// The canonical BED column order shared by the writer and the reader.
pub(crate) const BED_COLUMNS: [&str; 12] = [
    "chrom",
    "start",
    "end",
    "name",
    "score",
    "strand",
    "thickStart",
    "thickEnd",
    "itemRgb",
    "blockCount",
    "blockSizes",
    "blockStarts",
];

pub type OutputResult<T> = Result<T, OutputError>;

#[derive(Debug)]
pub enum OutputError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Export(linxira_bio_export::ExportError),
    /// The `OutputSpec` document or an artifact declaration is invalid.
    InvalidSpec(String),
    /// A canonical record does not carry the fields its format requires.
    InvalidRecord(String),
    /// The format has no writer or reader yet (renderers arrive with M1).
    UnsupportedFormat(String),
    /// An input path could not be resolved into a usable file path.
    InvalidPath(String),
}

impl fmt::Display for OutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "bio output I/O error: {error}"),
            Self::Json(error) => write!(formatter, "bio output JSON error: {error}"),
            Self::Export(error) => write!(formatter, "bio table export error: {error}"),
            Self::InvalidSpec(message) => write!(formatter, "invalid output spec: {message}"),
            Self::InvalidRecord(message) => write!(formatter, "invalid record: {message}"),
            Self::UnsupportedFormat(message) => write!(formatter, "unsupported format: {message}"),
            Self::InvalidPath(message) => write!(formatter, "invalid input path: {message}"),
        }
    }
}

impl std::error::Error for OutputError {}

impl From<std::io::Error> for OutputError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for OutputError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<linxira_bio_export::ExportError> for OutputError {
    fn from(error: linxira_bio_export::ExportError) -> Self {
        Self::Export(error)
    }
}

/// Declares the artifact set an analysis produces. The spec is declarative:
/// it names roles, formats, and relative paths; the JSON envelope remains the
/// authoritative data source that artifacts project from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSpec {
    /// Must be the integer version `"1"` (never `"1.0"`).
    pub schema_version: String,
    pub capability: String,
    pub artifacts: Vec<ArtifactSpec>,
}

impl OutputSpec {
    pub fn validate(&self) -> OutputResult<()> {
        if self.schema_version != OUTPUT_SPEC_SCHEMA_VERSION {
            return Err(OutputError::InvalidSpec(format!(
                "schema_version must be the integer version {:?}, got {:?}",
                OUTPUT_SPEC_SCHEMA_VERSION, self.schema_version
            )));
        }
        if self.capability.trim().is_empty() {
            return Err(OutputError::InvalidSpec(
                "capability must not be empty".to_owned(),
            ));
        }
        let mut roles = BTreeSet::new();
        for artifact in &self.artifacts {
            if artifact.role.trim().is_empty() {
                return Err(OutputError::InvalidSpec(
                    "artifact role must not be empty".to_owned(),
                ));
            }
            if !roles.insert(artifact.role.as_str()) {
                return Err(OutputError::InvalidSpec(format!(
                    "duplicate artifact role {:?}",
                    artifact.role
                )));
            }
            if Path::new(&artifact.path).is_absolute() || Path::new(&artifact.path).has_root() {
                return Err(OutputError::InvalidSpec(format!(
                    "artifact path {:?} must be relative to the workspace output directory",
                    artifact.path
                )));
            }
        }
        Ok(())
    }
}

/// One declared artifact: a role, the on-disk format, and a workspace-relative
/// path. `columns` overrides table column order, `header` appends extra
/// header lines (VCF meta lines, SAM `@SQ` lines), and `coords` overrides the
/// on-disk coordinate convention for interval formats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub role: String,
    pub format: BioDataFormat,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coords: Option<CoordinateSystem>,
}

/// The on-disk coordinate convention for interval data.
///
/// JSON records are always 1-based closed; `Bed` converts the start position
/// to 0-based half-open on write and back on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoordinateSystem {
    /// 0-based half-open (BED).
    Bed,
    /// 1-based inclusive (GFF3/GTF/SAM).
    Gff,
    /// 1-based inclusive (VCF).
    Vcf,
}

impl CoordinateSystem {
    /// The native on-disk convention of a format.
    pub fn for_format(format: BioDataFormat) -> Self {
        match format {
            BioDataFormat::Bed => Self::Bed,
            BioDataFormat::Vcf => Self::Vcf,
            BioDataFormat::Fasta
            | BioDataFormat::Fastq
            | BioDataFormat::Gff3
            | BioDataFormat::Gtf
            | BioDataFormat::Sam => Self::Gff,
            _ => Self::Gff,
        }
    }

    /// Converts a 1-based closed JSON start into this coordinate system.
    pub fn output_start(self, json_start: u64) -> OutputResult<u64> {
        match self {
            Self::Gff | Self::Vcf => Ok(json_start),
            Self::Bed => json_start.checked_sub(1).ok_or_else(|| {
                OutputError::InvalidRecord(
                    "interval start 0 cannot be written to the 0-based half-open BED convention; \
                     JSON interval coordinates are 1-based closed"
                        .to_owned(),
                )
            }),
        }
    }

    /// Converts an on-disk start in this coordinate system back to the
    /// 1-based closed JSON convention.
    pub fn json_start(self, on_disk_start: u64) -> u64 {
        match self {
            Self::Gff | Self::Vcf => on_disk_start,
            Self::Bed => on_disk_start.saturating_add(1),
        }
    }
}

/// Per-artifact overrides for a single write, mirroring [`ArtifactSpec`] but
/// usable without a full spec (the CLI `export bio` path).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriteOptions {
    pub coords: Option<CoordinateSystem>,
    pub columns: Option<Vec<String>>,
    pub header: Option<Vec<String>>,
    /// FASTA sequence wrap width; defaults to the standard 60 columns.
    pub fasta_line_width: Option<usize>,
    /// VCF sample names; defaults to `SAMPLE1..N`.
    pub sample_names: Option<Vec<String>>,
}

impl WriteOptions {
    fn from_artifact(artifact: &ArtifactSpec) -> Self {
        Self {
            coords: artifact.coords,
            columns: artifact.columns.clone(),
            header: artifact.header.clone(),
            fasta_line_width: None,
            sample_names: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReceipt {
    pub role: String,
    pub format: BioDataFormat,
    pub output_path: std::path::PathBuf,
    pub size_bytes: u64,
}

/// Infers the [`BioDataFormat`] from a path's final extension. Compressed
/// suffixes are rejected: decompression is `format_probe`/`import probe`
/// territory, and writers only emit uncompressed artifacts.
pub fn format_from_path(path: &Path) -> OutputResult<BioDataFormat> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "fa" | "fasta" | "fna" | "faa" => Ok(BioDataFormat::Fasta),
        "fq" | "fastq" => Ok(BioDataFormat::Fastq),
        "bed" => Ok(BioDataFormat::Bed),
        "gff" | "gff3" => Ok(BioDataFormat::Gff3),
        "gtf" => Ok(BioDataFormat::Gtf),
        "vcf" => Ok(BioDataFormat::Vcf),
        "sam" => Ok(BioDataFormat::Sam),
        "csv" => Ok(BioDataFormat::Csv),
        "tsv" => Ok(BioDataFormat::Tsv),
        "json" => Ok(BioDataFormat::Json),
        "jsonl" => Ok(BioDataFormat::Jsonl),
        "xlsx" => Ok(BioDataFormat::Xlsx),
        "gz" | "bgz" | "bz2" | "xz" | "zst" | "zip" | "7z" => Err(OutputError::UnsupportedFormat(
            "compressed outputs are not written directly; decompress first with `import probe` \
             or supply an uncompressed path"
                .to_owned(),
        )),
        other => Err(OutputError::UnsupportedFormat(format!(
            "no writer is registered for the .{other} extension"
        ))),
    }
}

/// Extracts canonical records from arbitrary result JSON.
///
/// Accepted shapes: a bare array of record objects, an object carrying a
/// `records` array, a single record object, or a V1/V2 envelope whose
/// `result` follows any of those shapes.
pub fn records_from_json(value: &Value) -> OutputResult<Vec<serde_json::Map<String, Value>>> {
    let value = value.get("result").unwrap_or(value);
    match value {
        Value::Array(values) => object_records(values, "the JSON array"),
        Value::Object(object) => match object.get("records") {
            Some(Value::Array(values)) => object_records(values, "the `records` array"),
            Some(_) => Err(OutputError::InvalidRecord(
                "`records` must be an array of record objects".to_owned(),
            )),
            None => Ok(vec![object.clone()]),
        },
        _ => Err(OutputError::InvalidRecord(
            "expected an array of records, a `records` array, or a result envelope".to_owned(),
        )),
    }
}

fn object_records(
    values: &[Value],
    context: &str,
) -> OutputResult<Vec<serde_json::Map<String, Value>>> {
    values
        .iter()
        .map(|value| {
            value.as_object().cloned().ok_or_else(|| {
                OutputError::InvalidRecord(format!("{context} must contain only record objects"))
            })
        })
        .collect()
}

/// Builds a table from canonical records: sorted union of keys as columns,
/// `null` filling missing fields (mirrors `Table::from_json` semantics but
/// over the already-extracted records).
pub fn table_from_records(records: &[serde_json::Map<String, Value>]) -> linxira_bio_export::Table {
    let mut names = BTreeSet::new();
    for record in records {
        names.extend(record.keys().cloned());
    }
    let columns: Vec<String> = names.into_iter().collect();
    let rows = records
        .iter()
        .map(|record| {
            columns
                .iter()
                .map(|column| record.get(column).cloned().unwrap_or(Value::Null))
                .collect()
        })
        .collect();
    linxira_bio_export::Table { columns, rows }
}

/// Maps a role to the JSON document backing it for [`BioDataWriter::write_spec`].
pub type RoleValues = BTreeMap<String, Value>;

#[cfg(test)]
mod tests {
    use super::{
        ArtifactSpec, CoordinateSystem, OUTPUT_SPEC_SCHEMA_VERSION, OutputSpec, format_from_path,
        records_from_json, table_from_records,
    };
    use linxira_bio_protocol::BioDataFormat;
    use serde_json::{Value, json};
    use std::path::Path;

    fn sample_spec() -> OutputSpec {
        OutputSpec {
            schema_version: OUTPUT_SPEC_SCHEMA_VERSION.to_owned(),
            capability: "sequence.extract.v1".to_owned(),
            artifacts: vec![ArtifactSpec {
                role: "sequences".to_owned(),
                format: BioDataFormat::Fasta,
                path: "analysis/sequence/sequences.fa".to_owned(),
                columns: None,
                header: None,
                coords: None,
            }],
        }
    }

    #[test]
    fn spec_validation_accepts_integer_schema_version() {
        sample_spec()
            .validate()
            .expect("integer schema version is valid");
    }

    #[test]
    fn spec_validation_rejects_decimal_schema_version() {
        let mut spec = sample_spec();
        spec.schema_version = "1.0".to_owned();
        let error = spec.validate().expect_err("1.0 is the historical mistake");
        assert!(error.to_string().contains("integer version"));
    }

    #[test]
    fn spec_validation_rejects_absolute_and_duplicate_artifacts() {
        let mut spec = sample_spec();
        spec.artifacts[0].path = "/etc/passwd".to_owned();
        assert!(spec.validate().is_err());

        let mut spec = sample_spec();
        spec.artifacts.push(spec.artifacts[0].clone());
        let error = spec.validate().expect_err("duplicate role");
        assert!(error.to_string().contains("duplicate artifact role"));
    }

    #[test]
    fn bed_defaults_to_zero_based_half_open_and_gff_stays_one_based() {
        assert_eq!(
            CoordinateSystem::for_format(BioDataFormat::Bed),
            CoordinateSystem::Bed
        );
        assert_eq!(
            CoordinateSystem::for_format(BioDataFormat::Gff3),
            CoordinateSystem::Gff
        );
        assert_eq!(
            CoordinateSystem::for_format(BioDataFormat::Sam),
            CoordinateSystem::Gff
        );
        assert_eq!(
            CoordinateSystem::Bed
                .output_start(1)
                .expect("start 1 maps to bed start 0"),
            0
        );
        let error = CoordinateSystem::Bed
            .output_start(0)
            .expect_err("1-based JSON cannot express a bed start below 0");
        assert!(error.to_string().contains("1-based closed"));
        assert_eq!(CoordinateSystem::Bed.json_start(0), 1);
        assert_eq!(CoordinateSystem::Gff.json_start(7), 7);
    }

    #[test]
    fn extracts_records_from_every_accepted_shape() {
        let records = json!([
            {"id": "a", "sequence": "AC"},
            {"id": "b", "sequence": "GT"}
        ]);
        assert_eq!(records_from_json(&records).expect("bare array").len(), 2);

        let wrapped = json!({"records": records});
        assert_eq!(records_from_json(&wrapped).expect("records key").len(), 2);

        let envelope = json!({"result": records, "status": "ok"});
        assert_eq!(records_from_json(&envelope).expect("envelope").len(), 2);

        let nested = json!({"result": {"records": records}});
        assert_eq!(records_from_json(&nested).expect("nested").len(), 2);

        let single = json!({"result": {"id": "a", "sequence": "AC"}});
        assert_eq!(records_from_json(&single).expect("single record").len(), 1);
    }

    #[test]
    fn rejects_malformed_record_containers() {
        let mixed = json!([{"id": "a"}, "not-an-object"]);
        let error = records_from_json(&mixed).expect_err("mixed array");
        assert!(error.to_string().contains("only record objects"));

        let bad_records = json!({"records": 3});
        assert!(records_from_json(&bad_records).is_err());
    }

    #[test]
    fn tables_from_records_use_the_sorted_key_union() {
        let records = vec![
            serde_json::from_value(json!({"sample": "A", "count": 4})).expect("record"),
            serde_json::from_value(json!({"sample": "B"})).expect("record"),
        ];
        let table = table_from_records(&records);
        assert_eq!(table.columns, ["count", "sample"]);
        assert_eq!(table.rows[1][0], Value::Null);
    }

    #[test]
    fn infers_formats_from_extensions() {
        assert_eq!(
            format_from_path(Path::new("out.GFF3")).expect("gff3"),
            BioDataFormat::Gff3
        );
        assert_eq!(
            format_from_path(Path::new("contigs.fa")).expect("fasta"),
            BioDataFormat::Fasta
        );
        assert!(format_from_path(Path::new("reads.fastq.gz")).is_err());
        assert!(format_from_path(Path::new("matrix.parquet")).is_err());
    }
}
