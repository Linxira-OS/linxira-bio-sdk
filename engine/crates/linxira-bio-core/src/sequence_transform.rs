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
            _ => None,
        }
    }
}

impl From<io::Error> for SequenceTransformError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
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
        SequenceExtractOptions, SequenceFilterOptions, SequenceOrfOptions, SequenceTransformError,
        SequenceTranslateOptions, extract_fasta_path, filter_fasta_path, find_orfs_fasta_path,
        reverse_complement_fasta_path, translate_fasta_path,
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
}
