use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeSet, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_FASTA_LINE_WIDTH: usize = 80;
pub const DEFAULT_MAX_FASTA_RECORD_BASES: usize = 1_073_741_824;
const MAX_FASTA_HEADER_BYTES: usize = 65_536;
const MAX_ORFS_PER_RECORD: usize = 1_000_000;

#[derive(Debug)]
pub enum SequenceTransformError {
    Io(io::Error),
    Csv(csv::Error),
    EmptyIdentifier {
        line: usize,
    },
    InvalidHeaderEncoding {
        line: usize,
    },
    HeaderTooLong {
        line: usize,
        limit: usize,
    },
    SequenceBeforeHeader {
        line: usize,
    },
    RecordTooLarge {
        identifier: String,
        limit: usize,
    },
    NoRecords,
    OutputAlreadyExists(PathBuf),
    InvalidOption(String),
    MissingIdentifiers(Vec<String>),
    MixedNucleotideAlphabet {
        identifier: String,
    },
    InvalidNucleotide {
        identifier: String,
        position: usize,
        symbol: u8,
    },
    TooManyOrfs {
        identifier: String,
        limit: usize,
    },
}

impl Display for SequenceTransformError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "sequence operation failed: {error}"),
            Self::Csv(error) => write!(formatter, "sequence table operation failed: {error}"),
            Self::EmptyIdentifier { line } => {
                write!(formatter, "FASTA header at line {line} has no identifier")
            }
            Self::InvalidHeaderEncoding { line } => {
                write!(formatter, "FASTA header at line {line} is not valid UTF-8")
            }
            Self::HeaderTooLong { line, limit } => write!(
                formatter,
                "FASTA header at line {line} exceeds the {limit}-byte limit"
            ),
            Self::SequenceBeforeHeader { line } => write!(
                formatter,
                "sequence data appears before a FASTA header at line {line}"
            ),
            Self::RecordTooLarge { identifier, limit } => write!(
                formatter,
                "FASTA record {identifier:?} exceeds the {limit}-base safety limit"
            ),
            Self::NoRecords => write!(formatter, "FASTA contains no records"),
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::MissingIdentifiers(identifiers) => write!(
                formatter,
                "requested FASTA identifiers were not found: {}",
                identifiers.join(", ")
            ),
            Self::MixedNucleotideAlphabet { identifier } => write!(
                formatter,
                "FASTA record {identifier:?} mixes T and U nucleotides"
            ),
            Self::InvalidNucleotide {
                identifier,
                position,
                symbol,
            } => write!(
                formatter,
                "FASTA record {identifier:?} contains unsupported nucleotide byte 0x{symbol:02x} at position {position}"
            ),
            Self::TooManyOrfs { identifier, limit } => write!(
                formatter,
                "FASTA record {identifier:?} exceeds the {limit}-ORF safety limit"
            ),
        }
    }
}

impl Error for SequenceTransformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Csv(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SequenceTransformError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<csv::Error> for SequenceTransformError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SequenceRewriteSummary {
    pub input_records: u64,
    pub output_records: u64,
    pub input_residues: u64,
    pub output_residues: u64,
}

impl SequenceRewriteSummary {
    fn observe_input(&mut self, length: usize) {
        self.input_records += 1;
        self.input_residues += u64::try_from(length).expect("record length fits in u64");
    }

    fn observe_output(&mut self, length: usize) {
        self.output_records += 1;
        self.output_residues += u64::try_from(length).expect("record length fits in u64");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceExtractOptions {
    pub identifiers: Vec<String>,
    pub regions: Vec<SequenceRegion>,
    pub strict: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceStrand {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceRegion {
    pub identifier: String,
    pub start: u64,
    pub end: u64,
    pub strand: SequenceStrand,
}

impl SequenceRegion {
    pub fn label(&self) -> String {
        let strand = match self.strand {
            SequenceStrand::Forward => '+',
            SequenceStrand::Reverse => '-',
        };
        format!("{}:{}-{}:{strand}", self.identifier, self.start, self.end)
    }
}

pub fn parse_sequence_region_spec(
    specification: &str,
) -> Result<SequenceRegion, SequenceTransformError> {
    let (identifier, coordinate_specification) =
        specification.split_once(':').ok_or_else(|| {
            SequenceTransformError::InvalidOption(format!(
                "invalid region {specification:?}; expected ID:START-END or ID:START-END:+/-"
            ))
        })?;
    let (range_specification, strand) = match coordinate_specification.rsplit_once(':') {
        Some((range, "+")) => (range, SequenceStrand::Forward),
        Some((range, "-")) => (range, SequenceStrand::Reverse),
        Some(_) => {
            return Err(SequenceTransformError::InvalidOption(format!(
                "invalid region {specification:?}; strand must be + or -"
            )));
        }
        None => (coordinate_specification, SequenceStrand::Forward),
    };
    let (start, end) = range_specification.split_once('-').ok_or_else(|| {
        SequenceTransformError::InvalidOption(format!(
            "invalid region {specification:?}; expected START-END coordinates"
        ))
    })?;
    let region = SequenceRegion {
        identifier: identifier.to_owned(),
        start: start.parse::<u64>().map_err(|_| {
            SequenceTransformError::InvalidOption(format!(
                "invalid region {specification:?}; start must be a positive integer"
            ))
        })?,
        end: end.parse::<u64>().map_err(|_| {
            SequenceTransformError::InvalidOption(format!(
                "invalid region {specification:?}; end must be a positive integer"
            ))
        })?,
        strand,
    };
    validate_regions(std::slice::from_ref(&region))?;
    Ok(region)
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceExtractSummary {
    #[serde(flatten)]
    pub rewrite: SequenceRewriteSummary,
    pub requested_identifier_count: u64,
    pub matched_identifier_count: u64,
    pub requested_region_count: u64,
    pub emitted_region_count: u64,
    pub missing_identifiers: Vec<String>,
    pub missing_regions: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SequenceFilterOptions {
    pub min_length: u64,
    pub max_length: Option<u64>,
    pub min_gc_percent: Option<f64>,
    pub max_gc_percent: Option<f64>,
    pub max_n_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceFilterSummary {
    #[serde(flatten)]
    pub rewrite: SequenceRewriteSummary,
    pub rejected_by_length: u64,
    pub rejected_by_gc: u64,
    pub rejected_by_n: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceTranslateOptions {
    pub frames: Vec<i8>,
    pub trim_terminal_stop: bool,
    pub stop_at_first: bool,
}

impl Default for SequenceTranslateOptions {
    fn default() -> Self {
        Self {
            frames: vec![1],
            trim_terminal_stop: false,
            stop_at_first: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceTranslateSummary {
    #[serde(flatten)]
    pub rewrite: SequenceRewriteSummary,
    pub frames: Vec<i8>,
    pub genetic_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceOrfOptions {
    pub min_amino_acids: usize,
    pub include_reverse_strand: bool,
    pub include_partial_3prime: bool,
}

impl Default for SequenceOrfOptions {
    fn default() -> Self {
        Self {
            min_amino_acids: 30,
            include_reverse_strand: true,
            include_partial_3prime: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceOrfSummary {
    #[serde(flatten)]
    pub rewrite: SequenceRewriteSummary,
    pub records_with_orfs: u64,
    pub complete_orfs: u64,
    pub partial_orfs: u64,
    pub longest_orf_amino_acids: u64,
    pub minimum_amino_acids: u64,
    pub reverse_strand_searched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceIdNormalizeOptions {
    pub prefix: String,
    pub start: u64,
    pub width: Option<usize>,
    pub keep_description: bool,
}

impl Default for SequenceIdNormalizeOptions {
    fn default() -> Self {
        Self {
            prefix: "seq".to_owned(),
            start: 1,
            width: Some(6),
            keep_description: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceIdNormalizeSummary {
    #[serde(flatten)]
    pub rewrite: SequenceRewriteSummary,
    pub prefix: String,
    pub first_index: u64,
    pub last_index: Option<u64>,
    pub width: Option<usize>,
    pub kept_description: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SequenceMergeOptions {
    pub allow_duplicate_ids: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceMergeSummary {
    pub input_files: u64,
    pub input_records: u64,
    pub output_records: u64,
    pub input_residues: u64,
    pub output_residues: u64,
    pub duplicate_identifier_count: u64,
    pub duplicate_identifiers_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceSplitOptions {
    pub records_per_file: usize,
    pub prefix: String,
}

impl Default for SequenceSplitOptions {
    fn default() -> Self {
        Self {
            records_per_file: 1_000,
            prefix: "part".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceSplitSummary {
    pub input_records: u64,
    pub output_files: u64,
    pub input_residues: u64,
    pub output_residues: u64,
    pub records_per_file: u64,
    pub prefix: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceTableDelimiter {
    Csv,
    Tsv,
}

impl SequenceTableDelimiter {
    pub fn name(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
        }
    }

    pub fn byte(self) -> u8 {
        match self {
            Self::Csv => b',',
            Self::Tsv => b'\t',
        }
    }

    pub fn media_type(self) -> &'static str {
        match self {
            Self::Csv => "text/csv",
            Self::Tsv => "text/tab-separated-values",
        }
    }

    pub fn infer_from_path(path: &Path) -> Option<Self> {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("csv") => Some(Self::Csv),
            Some("tsv" | "tab") => Some(Self::Tsv),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceToTableOptions {
    pub delimiter: SequenceTableDelimiter,
    pub include_header: bool,
}

impl Default for SequenceToTableOptions {
    fn default() -> Self {
        Self {
            delimiter: SequenceTableDelimiter::Csv,
            include_header: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceToTableSummary {
    pub input_records: u64,
    pub output_rows: u64,
    pub input_residues: u64,
    pub delimiter: String,
    pub included_header: bool,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceFromTableOptions {
    pub delimiter: SequenceTableDelimiter,
    pub id_column: String,
    pub sequence_column: String,
    pub description_column: Option<String>,
}

impl Default for SequenceFromTableOptions {
    fn default() -> Self {
        Self {
            delimiter: SequenceTableDelimiter::Csv,
            id_column: "id".to_owned(),
            sequence_column: "sequence".to_owned(),
            description_column: Some("description".to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceFromTableSummary {
    pub input_rows: u64,
    pub output_records: u64,
    pub output_residues: u64,
    pub delimiter: String,
    pub id_column: String,
    pub sequence_column: String,
    pub description_column: Option<String>,
}

#[derive(Debug)]
struct FastaRecord {
    header: String,
    identifier: String,
    sequence: Vec<u8>,
}

#[derive(Debug)]
struct FoundOrf {
    protein: Vec<u8>,
    start: usize,
    end: usize,
    strand: char,
    frame: i8,
    complete: bool,
}

pub fn extract_fasta_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SequenceExtractOptions,
) -> Result<SequenceExtractSummary, SequenceTransformError> {
    let requested = validate_extract_options(options)?;
    let requested_regions = validate_regions(&options.regions)?;
    let mut matched = BTreeSet::new();
    let mut emitted_regions = BTreeSet::new();
    let mut rewrite = SequenceRewriteSummary::default();

    with_new_output(output.as_ref(), |writer| {
        visit_fasta_path(input.as_ref(), |record| {
            rewrite.observe_input(record.sequence.len());
            if requested.contains(&record.identifier) {
                write_fasta_record(writer, &record.header, &record.sequence)?;
                rewrite.observe_output(record.sequence.len());
                matched.insert(record.identifier.clone());
            }
            for (region_index, region) in requested_regions
                .iter()
                .enumerate()
                .filter(|(_, region)| region.identifier == record.identifier)
            {
                let start = usize::try_from(region.start - 1).map_err(|_| {
                    SequenceTransformError::InvalidOption(format!(
                        "region {} start exceeds this platform's size limit",
                        region.label()
                    ))
                })?;
                let end = usize::try_from(region.end).map_err(|_| {
                    SequenceTransformError::InvalidOption(format!(
                        "region {} end exceeds this platform's size limit",
                        region.label()
                    ))
                })?;
                if end > record.sequence.len() {
                    return Err(SequenceTransformError::InvalidOption(format!(
                        "region {} exceeds record length {}",
                        region.label(),
                        record.sequence.len()
                    )));
                }
                let mut sequence = record.sequence[start..end].to_vec();
                if region.strand == SequenceStrand::Reverse {
                    sequence = reverse_complement(&sequence, &record.identifier)?;
                }
                write_fasta_record(writer, &region.label(), &sequence)?;
                rewrite.observe_output(sequence.len());
                emitted_regions.insert(region_index);
            }
            Ok(())
        })?;

        let missing = requested
            .difference(&matched)
            .cloned()
            .collect::<Vec<String>>();
        let missing_regions = requested_regions
            .iter()
            .enumerate()
            .filter(|(index, _)| !emitted_regions.contains(index))
            .map(|(_, region)| region.label())
            .collect::<Vec<_>>();
        if options.strict && (!missing.is_empty() || !missing_regions.is_empty()) {
            return Err(SequenceTransformError::MissingIdentifiers(
                missing
                    .iter()
                    .cloned()
                    .chain(missing_regions.iter().cloned())
                    .collect(),
            ));
        }
        Ok(SequenceExtractSummary {
            rewrite: rewrite.clone(),
            requested_identifier_count: u64::try_from(requested.len())
                .expect("identifier count fits in u64"),
            matched_identifier_count: u64::try_from(matched.len())
                .expect("identifier count fits in u64"),
            requested_region_count: u64::try_from(requested_regions.len())
                .expect("region count fits in u64"),
            emitted_region_count: u64::try_from(emitted_regions.len())
                .expect("region count fits in u64"),
            missing_identifiers: missing,
            missing_regions,
        })
    })
}

pub fn filter_fasta_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SequenceFilterOptions,
) -> Result<SequenceFilterSummary, SequenceTransformError> {
    validate_filter_options(options)?;
    let mut summary = SequenceFilterSummary {
        rewrite: SequenceRewriteSummary::default(),
        rejected_by_length: 0,
        rejected_by_gc: 0,
        rejected_by_n: 0,
    };

    with_new_output(output.as_ref(), |writer| {
        visit_fasta_path(input.as_ref(), |record| {
            summary.rewrite.observe_input(record.sequence.len());
            let length = u64::try_from(record.sequence.len()).expect("record length fits in u64");
            if length < options.min_length || options.max_length.is_some_and(|max| length > max) {
                summary.rejected_by_length += 1;
                return Ok(());
            }

            let (gc_percent, n_percent) = nucleotide_percentages(&record.sequence);
            if options
                .min_gc_percent
                .is_some_and(|minimum| gc_percent < minimum)
                || options
                    .max_gc_percent
                    .is_some_and(|maximum| gc_percent > maximum)
            {
                summary.rejected_by_gc += 1;
                return Ok(());
            }
            if options
                .max_n_percent
                .is_some_and(|maximum| n_percent > maximum)
            {
                summary.rejected_by_n += 1;
                return Ok(());
            }

            write_fasta_record(writer, &record.header, &record.sequence)?;
            summary.rewrite.observe_output(record.sequence.len());
            Ok(())
        })?;
        Ok(summary.clone())
    })
}

pub fn reverse_complement_fasta_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<SequenceRewriteSummary, SequenceTransformError> {
    let mut summary = SequenceRewriteSummary::default();
    with_new_output(output.as_ref(), |writer| {
        visit_fasta_path(input.as_ref(), |record| {
            summary.observe_input(record.sequence.len());
            let transformed = reverse_complement(&record.sequence, &record.identifier)?;
            write_fasta_record(writer, &record.header, &transformed)?;
            summary.observe_output(transformed.len());
            Ok(())
        })?;
        Ok(summary.clone())
    })
}

pub fn translate_fasta_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SequenceTranslateOptions,
) -> Result<SequenceTranslateSummary, SequenceTransformError> {
    let frames = validate_frames(&options.frames)?;
    let mut rewrite = SequenceRewriteSummary::default();
    with_new_output(output.as_ref(), |writer| {
        visit_fasta_path(input.as_ref(), |record| {
            rewrite.observe_input(record.sequence.len());
            let normalized = normalize_dna(&record.sequence, &record.identifier)?;
            for frame in &frames {
                let oriented = if *frame < 0 {
                    reverse_complement_dna(&normalized)
                } else {
                    normalized.clone()
                };
                let offset = usize::from(frame.unsigned_abs()) - 1;
                let protein = translate_dna(
                    oriented.get(offset..).unwrap_or_default(),
                    options.trim_terminal_stop,
                    options.stop_at_first,
                );
                let header = format!("{}|frame={frame:+}", record.identifier);
                write_fasta_record(writer, &header, &protein)?;
                rewrite.observe_output(protein.len());
            }
            Ok(())
        })?;
        Ok(SequenceTranslateSummary {
            rewrite: rewrite.clone(),
            frames: frames.clone(),
            genetic_code: "NCBI standard code 1".to_owned(),
        })
    })
}

pub fn find_orfs_fasta_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SequenceOrfOptions,
) -> Result<SequenceOrfSummary, SequenceTransformError> {
    if options.min_amino_acids == 0 {
        return Err(SequenceTransformError::InvalidOption(
            "minimum ORF length must be at least one amino acid".to_owned(),
        ));
    }

    let mut summary = SequenceOrfSummary {
        rewrite: SequenceRewriteSummary::default(),
        records_with_orfs: 0,
        complete_orfs: 0,
        partial_orfs: 0,
        longest_orf_amino_acids: 0,
        minimum_amino_acids: u64::try_from(options.min_amino_acids)
            .expect("minimum amino acid length fits in u64"),
        reverse_strand_searched: options.include_reverse_strand,
    };

    with_new_output(output.as_ref(), |writer| {
        visit_fasta_path(input.as_ref(), |record| {
            summary.rewrite.observe_input(record.sequence.len());
            let normalized = normalize_dna(&record.sequence, &record.identifier)?;
            let mut found = find_orfs_on_strand(
                &normalized,
                normalized.len(),
                '+',
                options,
                &record.identifier,
            )?;
            if options.include_reverse_strand {
                let reverse = reverse_complement_dna(&normalized);
                found.extend(find_orfs_on_strand(
                    &reverse,
                    normalized.len(),
                    '-',
                    options,
                    &record.identifier,
                )?);
            }
            found.sort_by_key(|orf| (orf.start, orf.end, orf.strand, orf.frame));
            if !found.is_empty() {
                summary.records_with_orfs += 1;
            }
            for (index, orf) in found.into_iter().enumerate() {
                let ordinal = index + 1;
                let completeness = if orf.complete {
                    "complete"
                } else {
                    "partial-3prime"
                };
                let header = format!(
                    "{}|orf={ordinal} strand={} frame={:+} start={} end={} {completeness}",
                    record.identifier,
                    orf.strand,
                    orf.frame,
                    orf.start + 1,
                    orf.end
                );
                write_fasta_record(writer, &header, &orf.protein)?;
                summary.rewrite.observe_output(orf.protein.len());
                summary.longest_orf_amino_acids = summary
                    .longest_orf_amino_acids
                    .max(u64::try_from(orf.protein.len()).expect("ORF length fits in u64"));
                if orf.complete {
                    summary.complete_orfs += 1;
                } else {
                    summary.partial_orfs += 1;
                }
            }
            Ok(())
        })?;
        Ok(summary.clone())
    })
}

pub fn normalize_fasta_ids_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SequenceIdNormalizeOptions,
) -> Result<SequenceIdNormalizeSummary, SequenceTransformError> {
    validate_identifier_prefix(&options.prefix, "identifier prefix")?;
    validate_optional_width(options.width)?;
    if options.start == 0 {
        return Err(SequenceTransformError::InvalidOption(
            "start index must be at least 1".to_owned(),
        ));
    }

    let mut rewrite = SequenceRewriteSummary::default();
    let mut next_index = options.start;
    with_new_output(output.as_ref(), |writer| {
        visit_fasta_path(input.as_ref(), |record| {
            rewrite.observe_input(record.sequence.len());
            let new_identifier =
                format_numbered_identifier(&options.prefix, next_index, options.width);
            next_index = next_index.checked_add(1).ok_or_else(|| {
                SequenceTransformError::InvalidOption(
                    "identifier index overflowed u64 while normalizing FASTA IDs".to_owned(),
                )
            })?;
            let header = if options.keep_description {
                let description = record_description(&record);
                if description.is_empty() {
                    new_identifier
                } else {
                    format!("{new_identifier} {description}")
                }
            } else {
                new_identifier
            };
            write_fasta_record(writer, &header, &record.sequence)?;
            rewrite.observe_output(record.sequence.len());
            Ok(())
        })?;

        let last_index = if rewrite.output_records == 0 {
            None
        } else {
            Some(next_index - 1)
        };
        Ok(SequenceIdNormalizeSummary {
            rewrite: rewrite.clone(),
            prefix: options.prefix.clone(),
            first_index: options.start,
            last_index,
            width: options.width,
            kept_description: options.keep_description,
        })
    })
}

pub fn merge_fasta_paths<P: AsRef<Path>>(
    inputs: &[P],
    output: impl AsRef<Path>,
    options: &SequenceMergeOptions,
) -> Result<SequenceMergeSummary, SequenceTransformError> {
    if inputs.is_empty() {
        return Err(SequenceTransformError::InvalidOption(
            "sequence merge requires at least one input FASTA".to_owned(),
        ));
    }

    let mut seen = HashSet::new();
    let mut summary = SequenceMergeSummary {
        input_files: 0,
        input_records: 0,
        output_records: 0,
        input_residues: 0,
        output_residues: 0,
        duplicate_identifier_count: 0,
        duplicate_identifiers_allowed: options.allow_duplicate_ids,
    };

    with_new_output(output.as_ref(), |writer| {
        for input in inputs {
            summary.input_files += 1;
            visit_fasta_path(input.as_ref(), |record| {
                summary.input_records += 1;
                summary.input_residues +=
                    u64::try_from(record.sequence.len()).expect("record length fits in u64");
                if !seen.insert(record.identifier.clone()) {
                    summary.duplicate_identifier_count += 1;
                    if !options.allow_duplicate_ids {
                        return Err(SequenceTransformError::InvalidOption(format!(
                            "duplicate FASTA identifier {:?}; pass --allow-duplicate-ids to keep duplicates",
                            record.identifier
                        )));
                    }
                }
                write_fasta_record(writer, &record.header, &record.sequence)?;
                summary.output_records += 1;
                summary.output_residues +=
                    u64::try_from(record.sequence.len()).expect("record length fits in u64");
                Ok(())
            })?;
        }
        Ok(summary.clone())
    })
}

pub fn split_fasta_path(
    input: impl AsRef<Path>,
    output_directory: impl AsRef<Path>,
    options: &SequenceSplitOptions,
) -> Result<SequenceSplitSummary, SequenceTransformError> {
    validate_split_options(options)?;
    let output_directory = output_directory.as_ref();
    let mut summary = SequenceSplitSummary {
        input_records: 0,
        output_files: 0,
        input_residues: 0,
        output_residues: 0,
        records_per_file: u64::try_from(options.records_per_file)
            .expect("records per file fits in u64"),
        prefix: options.prefix.clone(),
    };
    let mut created = Vec::<PathBuf>::new();
    let mut writer: Option<BufWriter<File>> = None;
    let mut records_in_current_file = 0_usize;

    let result = (|| {
        if output_directory.exists() && !output_directory.is_dir() {
            return Err(SequenceTransformError::InvalidOption(format!(
                "split output path is not a directory: {}",
                output_directory.display()
            )));
        }
        fs::create_dir_all(output_directory)?;
        visit_fasta_path(input.as_ref(), |record| {
            if writer.is_none() || records_in_current_file == options.records_per_file {
                if let Some(mut previous) = writer.take() {
                    previous.flush()?;
                }
                let next_file_index = summary.output_files + 1;
                let output_path =
                    output_directory.join(format!("{}_{next_file_index:03}.fa", options.prefix));
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&output_path)?;
                created.push(output_path);
                writer = Some(BufWriter::new(file));
                records_in_current_file = 0;
                summary.output_files = next_file_index;
            }

            summary.input_records += 1;
            summary.input_residues +=
                u64::try_from(record.sequence.len()).expect("record length fits in u64");
            let active = writer
                .as_mut()
                .expect("split writer is created before writing");
            write_fasta_record(active, &record.header, &record.sequence)?;
            records_in_current_file += 1;
            summary.output_residues +=
                u64::try_from(record.sequence.len()).expect("record length fits in u64");
            Ok(())
        })?;
        if let Some(mut active) = writer.take() {
            active.flush()?;
        }
        Ok(summary.clone())
    })();

    if result.is_err() {
        drop(writer);
        for path in created {
            let _ = fs::remove_file(path);
        }
    }
    result
}

pub fn fasta_to_table_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SequenceToTableOptions,
) -> Result<SequenceToTableSummary, SequenceTransformError> {
    let columns = vec![
        "id".to_owned(),
        "description".to_owned(),
        "length".to_owned(),
        "sequence".to_owned(),
    ];
    let mut summary = SequenceToTableSummary {
        input_records: 0,
        output_rows: 0,
        input_residues: 0,
        delimiter: options.delimiter.name().to_owned(),
        included_header: options.include_header,
        columns: columns.clone(),
    };

    with_new_output(output.as_ref(), |writer| {
        let mut table = csv::WriterBuilder::new()
            .delimiter(options.delimiter.byte())
            .has_headers(false)
            .from_writer(writer);
        if options.include_header {
            table.write_record(columns.iter().map(String::as_str))?;
        }
        visit_fasta_path(input.as_ref(), |record| {
            summary.input_records += 1;
            summary.input_residues +=
                u64::try_from(record.sequence.len()).expect("record length fits in u64");
            let description = record_description(&record);
            let length = record.sequence.len().to_string();
            let sequence = sequence_text(&record.sequence, &record.identifier)?;
            table.write_record([
                record.identifier.as_str(),
                description,
                length.as_str(),
                sequence.as_str(),
            ])?;
            summary.output_rows += 1;
            Ok(())
        })?;
        table.flush()?;
        Ok(summary.clone())
    })
}

pub fn table_to_fasta_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SequenceFromTableOptions,
) -> Result<SequenceFromTableSummary, SequenceTransformError> {
    validate_column_name(&options.id_column, "id column")?;
    validate_column_name(&options.sequence_column, "sequence column")?;
    if let Some(column) = &options.description_column {
        validate_column_name(column, "description column")?;
    }

    let mut summary = SequenceFromTableSummary {
        input_rows: 0,
        output_records: 0,
        output_residues: 0,
        delimiter: options.delimiter.name().to_owned(),
        id_column: options.id_column.clone(),
        sequence_column: options.sequence_column.clone(),
        description_column: options.description_column.clone(),
    };

    with_new_output(output.as_ref(), |writer| {
        let reader = open_fasta(input.as_ref())?;
        let mut table = csv::ReaderBuilder::new()
            .delimiter(options.delimiter.byte())
            .from_reader(reader);
        let headers = table.headers()?.clone();
        let id_index = find_table_column(&headers, &options.id_column)?;
        let sequence_index = find_table_column(&headers, &options.sequence_column)?;
        let description_index = options
            .description_column
            .as_deref()
            .map(|column| find_table_column(&headers, column))
            .transpose()?;

        for row in table.records() {
            let row = row?;
            summary.input_rows += 1;
            let identifier = row.get(id_index).unwrap_or_default().trim();
            validate_sequence_identifier(identifier)?;
            let raw_sequence = row.get(sequence_index).unwrap_or_default();
            let sequence = raw_sequence
                .bytes()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect::<Vec<_>>();
            let header = match description_index
                .and_then(|index| row.get(index))
                .map(str::trim)
                .filter(|description| !description.is_empty())
            {
                Some(description) => format!("{identifier} {description}"),
                None => identifier.to_owned(),
            };
            write_fasta_record(writer, &header, &sequence)?;
            summary.output_records += 1;
            summary.output_residues +=
                u64::try_from(sequence.len()).expect("sequence length fits in u64");
        }
        if summary.output_records == 0 {
            return Err(SequenceTransformError::NoRecords);
        }
        Ok(summary.clone())
    })
}

fn validate_extract_options(
    options: &SequenceExtractOptions,
) -> Result<BTreeSet<String>, SequenceTransformError> {
    if options.identifiers.is_empty() && options.regions.is_empty() {
        return Err(SequenceTransformError::InvalidOption(
            "sequence extraction requires at least one identifier or region".to_owned(),
        ));
    }
    let mut requested = BTreeSet::new();
    for identifier in &options.identifiers {
        if identifier.is_empty() || identifier.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(SequenceTransformError::InvalidOption(format!(
                "invalid FASTA identifier: {identifier:?}"
            )));
        }
        requested.insert(identifier.clone());
    }
    Ok(requested)
}

fn validate_regions(
    regions: &[SequenceRegion],
) -> Result<Vec<SequenceRegion>, SequenceTransformError> {
    let mut validated = Vec::with_capacity(regions.len());
    for region in regions {
        if region.identifier.is_empty()
            || region
                .identifier
                .bytes()
                .any(|byte| byte.is_ascii_whitespace())
        {
            return Err(SequenceTransformError::InvalidOption(format!(
                "invalid FASTA region identifier: {:?}",
                region.identifier
            )));
        }
        if region.start == 0 || region.end < region.start {
            return Err(SequenceTransformError::InvalidOption(format!(
                "invalid FASTA region {}; coordinates are 1-based inclusive and require start <= end",
                region.label()
            )));
        }
        validated.push(region.clone());
    }
    Ok(validated)
}

fn validate_filter_options(options: &SequenceFilterOptions) -> Result<(), SequenceTransformError> {
    if options
        .max_length
        .is_some_and(|maximum| maximum < options.min_length)
    {
        return Err(SequenceTransformError::InvalidOption(
            "maximum sequence length must be at least the minimum length".to_owned(),
        ));
    }
    for (name, value) in [
        ("minimum GC percentage", options.min_gc_percent),
        ("maximum GC percentage", options.max_gc_percent),
        ("maximum N percentage", options.max_n_percent),
    ] {
        if value.is_some_and(|percent| !percent.is_finite() || !(0.0..=100.0).contains(&percent)) {
            return Err(SequenceTransformError::InvalidOption(format!(
                "{name} must be between 0 and 100"
            )));
        }
    }
    if matches!(
        (options.min_gc_percent, options.max_gc_percent),
        (Some(minimum), Some(maximum)) if minimum > maximum
    ) {
        return Err(SequenceTransformError::InvalidOption(
            "maximum GC percentage must be at least the minimum GC percentage".to_owned(),
        ));
    }
    Ok(())
}

fn validate_identifier_prefix(prefix: &str, label: &str) -> Result<(), SequenceTransformError> {
    if prefix.is_empty() || prefix.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(SequenceTransformError::InvalidOption(format!(
            "{label} must be non-empty and cannot contain whitespace"
        )));
    }
    Ok(())
}

fn validate_sequence_identifier(identifier: &str) -> Result<(), SequenceTransformError> {
    if identifier.is_empty() || identifier.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(SequenceTransformError::InvalidOption(format!(
            "invalid FASTA identifier in table row: {identifier:?}"
        )));
    }
    Ok(())
}

fn validate_optional_width(width: Option<usize>) -> Result<(), SequenceTransformError> {
    if width.is_some_and(|width| width == 0 || width > 32) {
        return Err(SequenceTransformError::InvalidOption(
            "identifier width must be between 1 and 32".to_owned(),
        ));
    }
    Ok(())
}

fn validate_split_options(options: &SequenceSplitOptions) -> Result<(), SequenceTransformError> {
    if options.records_per_file == 0 {
        return Err(SequenceTransformError::InvalidOption(
            "records-per-file must be at least 1".to_owned(),
        ));
    }
    validate_path_prefix(&options.prefix)
}

fn validate_path_prefix(prefix: &str) -> Result<(), SequenceTransformError> {
    if prefix.is_empty()
        || prefix == "."
        || prefix == ".."
        || prefix.bytes().any(|byte| {
            byte.is_ascii_whitespace()
                || matches!(
                    byte,
                    b'/' | b'\\' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|'
                )
        })
    {
        return Err(SequenceTransformError::InvalidOption(
            "split prefix must be a non-empty safe filename fragment".to_owned(),
        ));
    }
    Ok(())
}

fn validate_column_name(column: &str, label: &str) -> Result<(), SequenceTransformError> {
    if column.trim().is_empty() {
        return Err(SequenceTransformError::InvalidOption(format!(
            "{label} cannot be empty"
        )));
    }
    Ok(())
}

fn format_numbered_identifier(prefix: &str, index: u64, width: Option<usize>) -> String {
    match width {
        Some(width) => format!("{prefix}{index:0width$}"),
        None => format!("{prefix}{index}"),
    }
}

fn record_description(record: &FastaRecord) -> &str {
    record
        .header
        .get(record.identifier.len()..)
        .unwrap_or_default()
        .trim_start()
}

fn sequence_text(sequence: &[u8], identifier: &str) -> Result<String, SequenceTransformError> {
    std::str::from_utf8(sequence)
        .map(str::to_owned)
        .map_err(|_| {
            SequenceTransformError::InvalidOption(format!(
                "FASTA record {identifier:?} sequence is not valid UTF-8 text"
            ))
        })
}

fn find_table_column(
    headers: &csv::StringRecord,
    column: &str,
) -> Result<usize, SequenceTransformError> {
    headers
        .iter()
        .position(|header| header == column)
        .ok_or_else(|| {
            SequenceTransformError::InvalidOption(format!(
                "table is missing required column {column:?}"
            ))
        })
}

fn validate_frames(frames: &[i8]) -> Result<Vec<i8>, SequenceTransformError> {
    if frames.is_empty() {
        return Err(SequenceTransformError::InvalidOption(
            "translation requires at least one frame".to_owned(),
        ));
    }
    let mut seen = HashSet::new();
    let mut validated = Vec::with_capacity(frames.len());
    for frame in frames {
        if !matches!(frame, -3..=-1 | 1..=3) {
            return Err(SequenceTransformError::InvalidOption(format!(
                "unsupported translation frame {frame}; expected -3, -2, -1, 1, 2, or 3"
            )));
        }
        if seen.insert(*frame) {
            validated.push(*frame);
        }
    }
    Ok(validated)
}

fn open_fasta(path: &Path) -> Result<Box<dyn BufRead>, SequenceTransformError> {
    let mut prefix = [0_u8; 2];
    let prefix_length = File::open(path)?.read(&mut prefix)?;
    let input: Box<dyn Read> = if prefix_length == 2 && prefix == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    Ok(Box::new(BufReader::new(input)))
}

fn visit_fasta_path(
    path: &Path,
    mut visitor: impl FnMut(FastaRecord) -> Result<(), SequenceTransformError>,
) -> Result<u64, SequenceTransformError> {
    let mut reader = open_fasta(path)?;
    let mut line = Vec::new();
    let mut line_number = 0_usize;
    let mut header: Option<(String, String)> = None;
    let mut sequence = Vec::new();
    let mut records = 0_u64;

    loop {
        line.clear();
        let length = reader.read_until(b'\n', &mut line)?;
        if length == 0 {
            break;
        }
        line_number += 1;
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        let trimmed = trim_ascii(&line);
        if trimmed.is_empty() {
            continue;
        }

        if trimmed[0] == b'>' {
            if let Some((previous_header, identifier)) = header.take() {
                visitor(FastaRecord {
                    header: previous_header,
                    identifier,
                    sequence: std::mem::take(&mut sequence),
                })?;
                records += 1;
            }
            let header_bytes = trim_ascii(&trimmed[1..]);
            if header_bytes.is_empty() {
                return Err(SequenceTransformError::EmptyIdentifier { line: line_number });
            }
            if header_bytes.len() > MAX_FASTA_HEADER_BYTES {
                return Err(SequenceTransformError::HeaderTooLong {
                    line: line_number,
                    limit: MAX_FASTA_HEADER_BYTES,
                });
            }
            let header_text = std::str::from_utf8(header_bytes)
                .map_err(|_| SequenceTransformError::InvalidHeaderEncoding { line: line_number })?
                .to_owned();
            let identifier = header_text
                .split_ascii_whitespace()
                .next()
                .ok_or(SequenceTransformError::EmptyIdentifier { line: line_number })?
                .to_owned();
            header = Some((header_text, identifier));
            continue;
        }

        let identifier = header
            .as_ref()
            .map(|(_, identifier)| identifier)
            .ok_or(SequenceTransformError::SequenceBeforeHeader { line: line_number })?;
        for byte in trimmed
            .iter()
            .copied()
            .filter(|byte| !byte.is_ascii_whitespace())
        {
            if sequence.len() == DEFAULT_MAX_FASTA_RECORD_BASES {
                return Err(SequenceTransformError::RecordTooLarge {
                    identifier: identifier.clone(),
                    limit: DEFAULT_MAX_FASTA_RECORD_BASES,
                });
            }
            sequence.push(byte);
        }
    }

    if let Some((last_header, identifier)) = header {
        visitor(FastaRecord {
            header: last_header,
            identifier,
            sequence,
        })?;
        records += 1;
    }
    if records == 0 {
        return Err(SequenceTransformError::NoRecords);
    }
    Ok(records)
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn with_new_output<T>(
    output: &Path,
    operation: impl FnOnce(&mut BufWriter<File>) -> Result<T, SequenceTransformError>,
) -> Result<T, SequenceTransformError> {
    if output.exists() {
        return Err(SequenceTransformError::OutputAlreadyExists(
            output.to_owned(),
        ));
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut writer = BufWriter::new(file);
    match operation(&mut writer).and_then(|value| {
        writer.flush()?;
        Ok(value)
    }) {
        Ok(value) => Ok(value),
        Err(error) => {
            drop(writer);
            let _ = fs::remove_file(output);
            Err(error)
        }
    }
}

fn write_fasta_record(
    writer: &mut impl Write,
    header: &str,
    sequence: &[u8],
) -> Result<(), SequenceTransformError> {
    writer.write_all(b">")?;
    writer.write_all(header.as_bytes())?;
    writer.write_all(b"\n")?;
    for chunk in sequence.chunks(DEFAULT_FASTA_LINE_WIDTH) {
        writer.write_all(chunk)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn nucleotide_percentages(sequence: &[u8]) -> (f64, f64) {
    let mut gc = 0_u64;
    let mut canonical = 0_u64;
    let mut n = 0_u64;
    for byte in sequence {
        match byte.to_ascii_uppercase() {
            b'G' | b'C' => {
                gc += 1;
                canonical += 1;
            }
            b'A' | b'T' | b'U' => canonical += 1,
            b'N' => n += 1,
            _ => {}
        }
    }
    (
        percentage(gc, canonical),
        percentage(
            n,
            u64::try_from(sequence.len()).expect("record length fits in u64"),
        ),
    )
}

fn percentage(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 * 100.0 / denominator as f64
    }
}

fn normalize_dna(sequence: &[u8], identifier: &str) -> Result<Vec<u8>, SequenceTransformError> {
    let mut has_t = false;
    let mut has_u = false;
    let mut normalized = Vec::with_capacity(sequence.len());
    for (index, byte) in sequence.iter().copied().enumerate() {
        let upper = byte.to_ascii_uppercase();
        match upper {
            b'T' => has_t = true,
            b'U' => has_u = true,
            b'A' | b'C' | b'G' | b'R' | b'Y' | b'S' | b'W' | b'K' | b'M' | b'B' | b'D' | b'H'
            | b'V' | b'N' | b'-' | b'.' => {}
            _ => {
                return Err(SequenceTransformError::InvalidNucleotide {
                    identifier: identifier.to_owned(),
                    position: index + 1,
                    symbol: byte,
                });
            }
        }
        normalized.push(if upper == b'U' { b'T' } else { upper });
    }
    if has_t && has_u {
        return Err(SequenceTransformError::MixedNucleotideAlphabet {
            identifier: identifier.to_owned(),
        });
    }
    Ok(normalized)
}

fn reverse_complement(
    sequence: &[u8],
    identifier: &str,
) -> Result<Vec<u8>, SequenceTransformError> {
    let mut has_t = false;
    let mut has_u = false;
    for (index, byte) in sequence.iter().copied().enumerate() {
        match byte.to_ascii_uppercase() {
            b'T' => has_t = true,
            b'U' => has_u = true,
            b'A' | b'C' | b'G' | b'R' | b'Y' | b'S' | b'W' | b'K' | b'M' | b'B' | b'D' | b'H'
            | b'V' | b'N' | b'-' | b'.' => {}
            _ => {
                return Err(SequenceTransformError::InvalidNucleotide {
                    identifier: identifier.to_owned(),
                    position: index + 1,
                    symbol: byte,
                });
            }
        }
    }
    if has_t && has_u {
        return Err(SequenceTransformError::MixedNucleotideAlphabet {
            identifier: identifier.to_owned(),
        });
    }
    let rna = has_u;
    Ok(sequence
        .iter()
        .rev()
        .map(|byte| complement(byte.to_ascii_uppercase(), rna))
        .collect())
}

fn reverse_complement_dna(sequence: &[u8]) -> Vec<u8> {
    sequence
        .iter()
        .rev()
        .map(|byte| complement(*byte, false))
        .collect()
}

fn complement(base: u8, rna: bool) -> u8 {
    match base {
        b'A' if rna => b'U',
        b'A' => b'T',
        b'T' | b'U' => b'A',
        b'C' => b'G',
        b'G' => b'C',
        b'R' => b'Y',
        b'Y' => b'R',
        b'S' => b'S',
        b'W' => b'W',
        b'K' => b'M',
        b'M' => b'K',
        b'B' => b'V',
        b'V' => b'B',
        b'D' => b'H',
        b'H' => b'D',
        b'N' => b'N',
        b'-' => b'-',
        b'.' => b'.',
        _ => unreachable!("nucleotide validation runs before complement"),
    }
}

fn translate_dna(sequence: &[u8], trim_terminal_stop: bool, stop_at_first: bool) -> Vec<u8> {
    let mut protein = Vec::with_capacity(sequence.len() / 3);
    for codon in sequence.chunks_exact(3) {
        let amino_acid = standard_amino_acid(codon);
        if stop_at_first && amino_acid == b'*' {
            break;
        }
        protein.push(amino_acid);
    }
    if trim_terminal_stop && protein.last() == Some(&b'*') {
        protein.pop();
    }
    protein
}

fn standard_amino_acid(codon: &[u8]) -> u8 {
    match codon {
        b"TTT" | b"TTC" => b'F',
        b"TTA" | b"TTG" | b"CTT" | b"CTC" | b"CTA" | b"CTG" => b'L',
        b"ATT" | b"ATC" | b"ATA" => b'I',
        b"ATG" => b'M',
        b"GTT" | b"GTC" | b"GTA" | b"GTG" => b'V',
        b"TCT" | b"TCC" | b"TCA" | b"TCG" | b"AGT" | b"AGC" => b'S',
        b"CCT" | b"CCC" | b"CCA" | b"CCG" => b'P',
        b"ACT" | b"ACC" | b"ACA" | b"ACG" => b'T',
        b"GCT" | b"GCC" | b"GCA" | b"GCG" => b'A',
        b"TAT" | b"TAC" => b'Y',
        b"TAA" | b"TAG" | b"TGA" => b'*',
        b"CAT" | b"CAC" => b'H',
        b"CAA" | b"CAG" => b'Q',
        b"AAT" | b"AAC" => b'N',
        b"AAA" | b"AAG" => b'K',
        b"GAT" | b"GAC" => b'D',
        b"GAA" | b"GAG" => b'E',
        b"TGT" | b"TGC" => b'C',
        b"TGG" => b'W',
        b"CGT" | b"CGC" | b"CGA" | b"CGG" | b"AGA" | b"AGG" => b'R',
        b"GGT" | b"GGC" | b"GGA" | b"GGG" => b'G',
        _ => b'X',
    }
}

fn find_orfs_on_strand(
    oriented: &[u8],
    original_length: usize,
    strand: char,
    options: &SequenceOrfOptions,
    identifier: &str,
) -> Result<Vec<FoundOrf>, SequenceTransformError> {
    let mut found = Vec::new();
    for frame_offset in 0..3 {
        let frame = i8::try_from(frame_offset + 1).expect("frame fits in i8");
        let frame = if strand == '-' { -frame } else { frame };
        let mut start = None;
        let complete_end =
            frame_offset + oriented.len().saturating_sub(frame_offset).div_euclid(3) * 3;
        let mut position = frame_offset;
        while position + 3 <= oriented.len() {
            let codon = &oriented[position..position + 3];
            if start.is_none() && codon == b"ATG" {
                start = Some(position);
            }
            if matches!(codon, b"TAA" | b"TAG" | b"TGA")
                && let Some(start_position) = start.take()
            {
                add_orf(
                    &mut found,
                    oriented,
                    original_length,
                    strand,
                    frame,
                    start_position,
                    position + 3,
                    true,
                    options.min_amino_acids,
                    identifier,
                )?;
            }
            position += 3;
        }
        if options.include_partial_3prime
            && let Some(start_position) = start
            && complete_end > start_position
        {
            add_orf(
                &mut found,
                oriented,
                original_length,
                strand,
                frame,
                start_position,
                complete_end,
                false,
                options.min_amino_acids,
                identifier,
            )?;
        }
    }
    Ok(found)
}

#[allow(clippy::too_many_arguments)]
fn add_orf(
    found: &mut Vec<FoundOrf>,
    oriented: &[u8],
    original_length: usize,
    strand: char,
    frame: i8,
    oriented_start: usize,
    oriented_end: usize,
    complete: bool,
    minimum_amino_acids: usize,
    identifier: &str,
) -> Result<(), SequenceTransformError> {
    let coding_end = if complete {
        oriented_end.saturating_sub(3)
    } else {
        oriented_end
    };
    let protein = translate_dna(&oriented[oriented_start..coding_end], false, false);
    if protein.len() < minimum_amino_acids {
        return Ok(());
    }
    if found.len() == MAX_ORFS_PER_RECORD {
        return Err(SequenceTransformError::TooManyOrfs {
            identifier: identifier.to_owned(),
            limit: MAX_ORFS_PER_RECORD,
        });
    }
    let (start, end) = if strand == '+' {
        (oriented_start, oriented_end)
    } else {
        (
            original_length - oriented_end,
            original_length - oriented_start,
        )
    };
    found.push(FoundOrf {
        protein,
        start,
        end,
        strand,
        frame,
        complete,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        SequenceExtractOptions, SequenceFilterOptions, SequenceFromTableOptions,
        SequenceIdNormalizeOptions, SequenceMergeOptions, SequenceOrfOptions, SequenceSplitOptions,
        SequenceTableDelimiter, SequenceToTableOptions, SequenceTransformError,
        SequenceTranslateOptions, extract_fasta_path, fasta_to_table_path, filter_fasta_path,
        find_orfs_fasta_path, merge_fasta_paths, normalize_fasta_ids_path,
        reverse_complement_fasta_path, split_fasta_path, table_to_fasta_path, translate_fasta_path,
    };
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    fn fixture_path(suffix: &str) -> PathBuf {
        let ordinal = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-sequence-transform-{}-{ordinal}.{suffix}",
            std::process::id()
        ))
    }

    #[test]
    fn extracts_requested_identifiers_and_reports_missing_values() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">one first\nACGT\n>two\nNN\n>three\nGGA\n").unwrap();
        let summary = extract_fasta_path(
            &input,
            &output,
            &SequenceExtractOptions {
                identifiers: vec!["three".to_owned(), "missing".to_owned()],
                regions: Vec::new(),
                strict: false,
            },
        )
        .unwrap();

        assert_eq!(summary.rewrite.input_records, 3);
        assert_eq!(summary.rewrite.output_records, 1);
        assert_eq!(summary.missing_identifiers, ["missing"]);
        assert_eq!(fs::read_to_string(&output).unwrap(), ">three\nGGA\n");
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn strict_extraction_removes_partial_output() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">one\nACGT\n").unwrap();
        let error = extract_fasta_path(
            &input,
            &output,
            &SequenceExtractOptions {
                identifiers: vec!["missing".to_owned()],
                regions: Vec::new(),
                strict: true,
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            SequenceTransformError::MissingIdentifiers(_)
        ));
        assert!(!output.exists());
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn extracts_forward_and_reverse_coordinate_regions() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">chr1\nAACCGGTT\n").unwrap();
        let summary = extract_fasta_path(
            &input,
            &output,
            &SequenceExtractOptions {
                identifiers: Vec::new(),
                regions: vec![
                    super::parse_sequence_region_spec("chr1:2-5").unwrap(),
                    super::parse_sequence_region_spec("chr1:2-5:-").unwrap(),
                ],
                strict: true,
            },
        )
        .unwrap();

        assert_eq!(summary.requested_region_count, 2);
        assert_eq!(summary.emitted_region_count, 2);
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            ">chr1:2-5:+\nACCG\n>chr1:2-5:-\nCGGT\n"
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn filters_by_length_gc_and_n_content() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(
            &input,
            b">keep\nGCGCAT\n>short\nGC\n>low-gc\nAAAAAA\n>many-n\nGCNNNN\n",
        )
        .unwrap();
        let summary = filter_fasta_path(
            &input,
            &output,
            &SequenceFilterOptions {
                min_length: 4,
                max_length: Some(10),
                min_gc_percent: Some(50.0),
                max_gc_percent: None,
                max_n_percent: Some(50.0),
            },
        )
        .unwrap();

        assert_eq!(summary.rewrite.output_records, 1);
        assert_eq!(summary.rejected_by_length, 1);
        assert_eq!(summary.rejected_by_gc, 1);
        assert_eq!(summary.rejected_by_n, 1);
        assert_eq!(fs::read_to_string(&output).unwrap(), ">keep\nGCGCAT\n");
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn reverse_complements_dna_rna_and_iupac_symbols() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">dna\nACGTRYMKBDHVN\n>rna\nAUGC\n").unwrap();
        reverse_complement_fasta_path(&input, &output).unwrap();
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            ">dna\nNBDHVMKRYACGT\n>rna\nGCAU\n"
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn rejects_mixed_dna_rna_alphabet_and_removes_output() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">mixed\nATUG\n").unwrap();
        let error = reverse_complement_fasta_path(&input, &output).unwrap_err();
        assert!(matches!(
            error,
            SequenceTransformError::MixedNucleotideAlphabet { .. }
        ));
        assert!(!output.exists());
        fs::remove_file(input).unwrap();
    }

    #[test]
    fn translates_standard_code_and_negative_frame() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(
            &input,
            b">coding\nATGGCCATTGTAATGGGCCGCTGAAAGGGTGCCCGATAG\n",
        )
        .unwrap();
        let summary = translate_fasta_path(
            &input,
            &output,
            &SequenceTranslateOptions {
                frames: vec![1, -1],
                trim_terminal_stop: false,
                stop_at_first: false,
            },
        )
        .unwrap();
        let text = fs::read_to_string(&output).unwrap();

        assert_eq!(summary.frames, [1, -1]);
        assert!(text.contains(">coding|frame=+1\nMAIVMGR*KGAR*\n"));
        assert!(text.contains(">coding|frame=-1\nLSGTLSAAHYNGH\n"));
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn finds_complete_and_partial_orfs_with_coordinates() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">gene\nCCCATGAAACCCTAAATGAAAGGG\n").unwrap();
        let summary = find_orfs_fasta_path(
            &input,
            &output,
            &SequenceOrfOptions {
                min_amino_acids: 2,
                include_reverse_strand: false,
                include_partial_3prime: true,
            },
        )
        .unwrap();
        let text = fs::read_to_string(&output).unwrap();

        assert_eq!(summary.complete_orfs, 1);
        assert_eq!(summary.partial_orfs, 1);
        assert!(text.contains("strand=+ frame=+1 start=4 end=15 complete\nMKP\n"));
        assert!(text.contains("strand=+ frame=+1 start=16 end=24 partial-3prime\nMKG\n"));
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn reads_gzip_input_by_magic_bytes() {
        let input = fixture_path("data");
        let output = fixture_path("out.fa");
        let mut encoder = GzEncoder::new(fs::File::create(&input).unwrap(), Compression::default());
        encoder.write_all(b">one\nACGT\n").unwrap();
        encoder.finish().unwrap();

        let summary = reverse_complement_fasta_path(&input, &output).unwrap();
        assert_eq!(summary.output_records, 1);
        assert_eq!(fs::read_to_string(&output).unwrap(), ">one\nACGT\n");
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_existing_output() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">one\nACGT\n").unwrap();
        fs::write(&output, b"keep").unwrap();

        let error = reverse_complement_fasta_path(&input, &output).unwrap_err();
        assert!(matches!(
            error,
            SequenceTransformError::OutputAlreadyExists(_)
        ));
        assert_eq!(fs::read(&output).unwrap(), b"keep");
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn normalizes_fasta_identifiers_deterministically() {
        let input = fixture_path("fa");
        let output = fixture_path("out.fa");
        fs::write(&input, b">geneA alpha desc\nACGT\n>geneB beta desc\nNN\n").unwrap();

        let summary = normalize_fasta_ids_path(
            &input,
            &output,
            &SequenceIdNormalizeOptions {
                prefix: "lx".to_owned(),
                start: 7,
                width: Some(3),
                keep_description: true,
            },
        )
        .unwrap();

        assert_eq!(summary.rewrite.output_records, 2);
        assert_eq!(summary.last_index, Some(8));
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            ">lx007 alpha desc\nACGT\n>lx008 beta desc\nNN\n"
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(output).unwrap();
    }

    #[test]
    fn merges_fasta_files_and_rejects_duplicate_ids_by_default() {
        let first = fixture_path("a.fa");
        let second = fixture_path("b.fa");
        let output = fixture_path("merged.fa");
        fs::write(&first, b">one\nACGT\n").unwrap();
        fs::write(&second, b">one duplicate\nTT\n").unwrap();

        let error = merge_fasta_paths(
            &[first.clone(), second.clone()],
            &output,
            &SequenceMergeOptions::default(),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("duplicate FASTA identifier \"one\"")
        );
        assert!(!output.exists());

        let allowed_output = fixture_path("merged-allowed.fa");
        let summary = merge_fasta_paths(
            &[first.clone(), second.clone()],
            &allowed_output,
            &SequenceMergeOptions {
                allow_duplicate_ids: true,
            },
        )
        .unwrap();

        assert_eq!(summary.input_files, 2);
        assert_eq!(summary.duplicate_identifier_count, 1);
        assert_eq!(
            fs::read_to_string(&allowed_output).unwrap(),
            ">one\nACGT\n>one duplicate\nTT\n"
        );
        fs::remove_file(first).unwrap();
        fs::remove_file(second).unwrap();
        fs::remove_file(allowed_output).unwrap();
    }

    #[test]
    fn splits_fasta_into_deterministic_chunks() {
        let input = fixture_path("fa");
        let directory = std::env::temp_dir().join(format!(
            "linxira-sequence-split-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&input, b">one\nA\n>two\nCC\n>three\nGGG\n").unwrap();

        let summary = split_fasta_path(
            &input,
            &directory,
            &SequenceSplitOptions {
                records_per_file: 2,
                prefix: "chunk".to_owned(),
            },
        )
        .unwrap();

        assert_eq!(summary.input_records, 3);
        assert_eq!(summary.output_files, 2);
        assert_eq!(
            fs::read_to_string(directory.join("chunk_001.fa")).unwrap(),
            ">one\nA\n>two\nCC\n"
        );
        assert_eq!(
            fs::read_to_string(directory.join("chunk_002.fa")).unwrap(),
            ">three\nGGG\n"
        );
        fs::remove_file(input).unwrap();
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn converts_fasta_to_table_and_back() {
        let input = fixture_path("fa");
        let table = fixture_path("tsv");
        let roundtrip = fixture_path("roundtrip.fa");
        fs::write(&input, b">one desc\nACGT\n>two\nNN\n").unwrap();

        let to_table = fasta_to_table_path(
            &input,
            &table,
            &SequenceToTableOptions {
                delimiter: SequenceTableDelimiter::Tsv,
                include_header: true,
            },
        )
        .unwrap();
        assert_eq!(to_table.output_rows, 2);
        assert_eq!(
            fs::read_to_string(&table).unwrap(),
            "id\tdescription\tlength\tsequence\none\tdesc\t4\tACGT\ntwo\t\t2\tNN\n"
        );

        let from_table = table_to_fasta_path(
            &table,
            &roundtrip,
            &SequenceFromTableOptions {
                delimiter: SequenceTableDelimiter::Tsv,
                id_column: "id".to_owned(),
                sequence_column: "sequence".to_owned(),
                description_column: Some("description".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(from_table.output_records, 2);
        assert_eq!(
            fs::read_to_string(&roundtrip).unwrap(),
            ">one desc\nACGT\n>two\nNN\n"
        );
        fs::remove_file(input).unwrap();
        fs::remove_file(table).unwrap();
        fs::remove_file(roundtrip).unwrap();
    }
}
