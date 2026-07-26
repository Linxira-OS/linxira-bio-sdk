use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub const MAX_ANNOTATION_DECOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_ANNOTATION_RECORDS: usize = 2_000_000;
pub const DEFAULT_PROMOTER_LENGTH: u64 = 1_000;
pub const DEFAULT_GENE_DENSITY_WINDOW: u64 = 1_000_000;
pub const MAX_GENE_DENSITY_BINS: usize = 2_000_000;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AnnotationStats {
    pub record_count: u64,
    pub directive_count: u64,
    pub comment_count: u64,
    pub sequence_region_count: u64,
    pub records_with_id: u64,
    pub records_with_parent: u64,
    pub min_start: Option<u64>,
    pub max_end: Option<u64>,
    pub feature_type_counts: BTreeMap<String, u64>,
    pub sequence_counts: BTreeMap<String, u64>,
    pub source_counts: BTreeMap<String, u64>,
    pub strand_counts: BTreeMap<String, u64>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnnotationVisualFeature {
    pub seqid: String,
    pub feature_type: String,
    pub start: u64,
    pub end: u64,
    pub strand: char,
    pub id: Option<String>,
    pub label: Option<String>,
    pub parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneDensityOptions {
    pub feature_types: Vec<String>,
    pub window_size: u64,
    pub step_size: u64,
}

impl Default for GeneDensityOptions {
    fn default() -> Self {
        Self {
            feature_types: vec!["gene".to_owned()],
            window_size: DEFAULT_GENE_DENSITY_WINDOW,
            step_size: DEFAULT_GENE_DENSITY_WINDOW,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeneDensityBin {
    pub seqid: String,
    pub start: u64,
    pub end: u64,
    pub feature_count: u64,
    pub features_per_megabase: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GeneDensityResult {
    pub input_record_count: u64,
    pub selected_feature_count: u64,
    pub sequence_count: u64,
    pub feature_types: Vec<String>,
    pub window_size: u64,
    pub step_size: u64,
    pub bins: Vec<GeneDensityBin>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnnotationNormalizeOptions {
    pub sort: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AnnotationNormalizeSummary {
    pub input_record_count: u64,
    pub output_record_count: u64,
    pub sorted: bool,
    pub converted_gtf_attribute_records: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenePositionOptions {
    pub feature_types: Vec<String>,
}

impl Default for GenePositionOptions {
    fn default() -> Self {
        Self {
            feature_types: vec!["gene".to_owned()],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct GenePositionSummary {
    pub input_record_count: u64,
    pub output_record_count: u64,
    pub feature_type_counts: BTreeMap<String, u64>,
    pub missing_identifier_count: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationExtractOptions {
    pub feature_type: String,
    pub promoter_length: u64,
}

impl Default for AnnotationExtractOptions {
    fn default() -> Self {
        Self {
            feature_type: "gene".to_owned(),
            promoter_length: DEFAULT_PROMOTER_LENGTH,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AnnotationExtractSummary {
    pub annotation_record_count: u64,
    pub matched_feature_count: u64,
    pub output_sequence_count: u64,
    pub output_base_count: u64,
    pub missing_reference_count: u64,
    pub skipped_feature_count: u64,
    pub feature_type: String,
    pub promoter_length: Option<u64>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum AnnotationError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MalformedRecord { line: usize, message: String },
    OutputAlreadyExists(PathBuf),
    LimitExceeded { resource: &'static str, limit: u64 },
    InvalidOption(String),
}

impl Display for AnnotationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "annotation I/O failed: {error}"),
            Self::ReadLine { line, source } => {
                write!(
                    formatter,
                    "failed to read annotation at line {line}: {source}"
                )
            }
            Self::MalformedRecord { line, message } => {
                write!(
                    formatter,
                    "malformed annotation record at line {line}: {message}"
                )
            }
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "annotation processing exceeds the deterministic {resource} limit of {limit}"
            ),
            Self::InvalidOption(message) => formatter.write_str(message),
        }
    }
}

impl Error for AnnotationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            Self::MalformedRecord { .. }
            | Self::OutputAlreadyExists(_)
            | Self::LimitExceeded { .. }
            | Self::InvalidOption(_) => None,
        }
    }
}

impl From<io::Error> for AnnotationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnnotationRecord {
    seqid: String,
    source: String,
    feature_type: String,
    start: u64,
    end: u64,
    score: String,
    strand: char,
    phase: Option<u8>,
    attributes: BTreeMap<String, Vec<String>>,
    used_gtf_attributes: bool,
}

#[derive(Debug, Default)]
struct ParsedAnnotation {
    records: Vec<AnnotationRecord>,
    directive_count: u64,
    comment_count: u64,
    sequence_region_count: u64,
}

#[derive(Debug, Clone)]
struct ExtractGroup {
    seqid: String,
    strand: char,
    segments: Vec<ExtractSegment>,
}

#[derive(Debug, Clone, Copy)]
struct ExtractSegment {
    start: u64,
    end: u64,
    phase: Option<u8>,
}

pub fn annotation_stats_path(path: impl AsRef<Path>) -> Result<AnnotationStats, AnnotationError> {
    let parsed = read_annotation_path(path.as_ref())?;
    let mut stats = AnnotationStats {
        directive_count: parsed.directive_count,
        comment_count: parsed.comment_count,
        sequence_region_count: parsed.sequence_region_count,
        ..Default::default()
    };
    for record in &parsed.records {
        stats.record_count = checked_add(stats.record_count, 1)?;
        increment(&mut stats.feature_type_counts, &record.feature_type)?;
        increment(&mut stats.sequence_counts, &record.seqid)?;
        increment(&mut stats.source_counts, &record.source)?;
        increment(&mut stats.strand_counts, &record.strand.to_string())?;
        if attribute_first(record, &["ID", "gene_id", "transcript_id"]).is_some() {
            stats.records_with_id = checked_add(stats.records_with_id, 1)?;
        }
        if attribute_first(record, &["Parent"]).is_some() {
            stats.records_with_parent = checked_add(stats.records_with_parent, 1)?;
        }
        stats.min_start = Some(
            stats
                .min_start
                .map_or(record.start, |value| value.min(record.start)),
        );
        stats.max_end = Some(
            stats
                .max_end
                .map_or(record.end, |value| value.max(record.end)),
        );
    }
    if stats.record_count == 0 {
        stats
            .warnings
            .push("annotation input contains no feature records".to_owned());
    }
    Ok(stats)
}

pub fn annotation_visual_features_path(
    path: impl AsRef<Path>,
) -> Result<Vec<AnnotationVisualFeature>, AnnotationError> {
    let parsed = read_annotation_path(path.as_ref())?;
    Ok(parsed
        .records
        .into_iter()
        .map(|record| {
            let id =
                attribute_first(&record, &["ID", "gene_id", "transcript_id"]).map(str::to_owned);
            let label = attribute_first(&record, &["Name", "gene_name", "gene", "product", "ID"])
                .map(str::to_owned);
            let parents = record.attributes.get("Parent").cloned().unwrap_or_default();
            AnnotationVisualFeature {
                seqid: record.seqid,
                feature_type: record.feature_type,
                start: record.start,
                end: record.end,
                strand: record.strand,
                id,
                label,
                parents,
            }
        })
        .collect())
}

pub fn gene_density_path(
    path: impl AsRef<Path>,
    options: GeneDensityOptions,
) -> Result<GeneDensityResult, AnnotationError> {
    if options.feature_types.is_empty() {
        return Err(AnnotationError::InvalidOption(
            "gene-density feature_types must not be empty".to_owned(),
        ));
    }
    if options.window_size == 0 || options.step_size == 0 {
        return Err(AnnotationError::InvalidOption(
            "gene-density window_size and step_size must be positive".to_owned(),
        ));
    }
    let selected_types = options
        .feature_types
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if selected_types.is_empty() {
        return Err(AnnotationError::InvalidOption(
            "gene-density feature_types must contain a non-empty name".to_owned(),
        ));
    }
    let parsed = read_annotation_path(path.as_ref())?;
    let mut lengths = BTreeMap::<String, u64>::new();
    for record in &parsed.records {
        lengths
            .entry(record.seqid.clone())
            .and_modify(|length| *length = (*length).max(record.end))
            .or_insert(record.end);
    }
    let mut differences = BTreeMap::<String, Vec<i64>>::new();
    let mut total_bins = 0_usize;
    for (seqid, length) in &lengths {
        let bin_count_u64 = length
            .saturating_sub(1)
            .checked_div(options.step_size)
            .and_then(|value| value.checked_add(1))
            .ok_or(AnnotationError::LimitExceeded {
                resource: "gene-density bin count",
                limit: MAX_GENE_DENSITY_BINS as u64,
            })?;
        let bin_count =
            usize::try_from(bin_count_u64).map_err(|_| AnnotationError::LimitExceeded {
                resource: "gene-density bin count",
                limit: MAX_GENE_DENSITY_BINS as u64,
            })?;
        total_bins = total_bins
            .checked_add(bin_count)
            .ok_or(AnnotationError::LimitExceeded {
                resource: "gene-density bin count",
                limit: MAX_GENE_DENSITY_BINS as u64,
            })?;
        if total_bins > MAX_GENE_DENSITY_BINS {
            return Err(AnnotationError::LimitExceeded {
                resource: "gene-density bin count",
                limit: MAX_GENE_DENSITY_BINS as u64,
            });
        }
        differences.insert(seqid.clone(), vec![0; bin_count.saturating_add(1)]);
    }
    let mut selected_feature_count = 0_u64;
    for record in &parsed.records {
        if !selected_types.contains(&record.feature_type.to_ascii_lowercase()) {
            continue;
        }
        selected_feature_count = checked_add(selected_feature_count, 1)?;
        let Some(diff) = differences.get_mut(&record.seqid) else {
            continue;
        };
        let first = record
            .start
            .saturating_sub(options.window_size)
            .div_ceil(options.step_size);
        let last = record.end.saturating_sub(1) / options.step_size;
        let first = usize::try_from(first).map_err(|_| AnnotationError::LimitExceeded {
            resource: "gene-density bin index",
            limit: MAX_GENE_DENSITY_BINS as u64,
        })?;
        let last = usize::try_from(last).map_err(|_| AnnotationError::LimitExceeded {
            resource: "gene-density bin index",
            limit: MAX_GENE_DENSITY_BINS as u64,
        })?;
        if first < diff.len().saturating_sub(1) {
            diff[first] = diff[first]
                .checked_add(1)
                .ok_or(AnnotationError::LimitExceeded {
                    resource: "gene-density feature count",
                    limit: u64::MAX,
                })?;
            let after = last.saturating_add(1).min(diff.len() - 1);
            diff[after] = diff[after]
                .checked_sub(1)
                .ok_or(AnnotationError::LimitExceeded {
                    resource: "gene-density feature count",
                    limit: u64::MAX,
                })?;
        }
    }
    let mut bins = Vec::with_capacity(total_bins);
    for (seqid, length) in &lengths {
        let diff = differences
            .get(seqid)
            .expect("difference array exists for every sequence");
        let mut active = 0_i64;
        for (index, change) in diff.iter().take(diff.len().saturating_sub(1)).enumerate() {
            active = active
                .checked_add(*change)
                .ok_or(AnnotationError::LimitExceeded {
                    resource: "gene-density feature count",
                    limit: u64::MAX,
                })?;
            let start = (index as u64)
                .checked_mul(options.step_size)
                .and_then(|value| value.checked_add(1))
                .ok_or(AnnotationError::LimitExceeded {
                    resource: "gene-density coordinate",
                    limit: u64::MAX,
                })?;
            let end = start
                .saturating_add(options.window_size.saturating_sub(1))
                .min(*length);
            let width = end.saturating_sub(start).saturating_add(1);
            let feature_count = u64::try_from(active).map_err(|_| {
                AnnotationError::InvalidOption(
                    "gene-density internal overlap count became negative".to_owned(),
                )
            })?;
            bins.push(GeneDensityBin {
                seqid: seqid.clone(),
                start,
                end,
                feature_count,
                features_per_megabase: feature_count as f64 * 1_000_000.0 / width as f64,
            });
        }
    }
    let mut warnings = vec![
        "sequence lengths were inferred from the maximum annotation end coordinate".to_owned(),
    ];
    if selected_feature_count == 0 {
        warnings.push("no annotation records matched the requested feature types".to_owned());
    }
    Ok(GeneDensityResult {
        input_record_count: parsed.records.len() as u64,
        selected_feature_count,
        sequence_count: lengths.len() as u64,
        feature_types: selected_types.into_iter().collect(),
        window_size: options.window_size,
        step_size: options.step_size,
        bins,
        warnings,
    })
}

pub fn normalize_annotation_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: AnnotationNormalizeOptions,
) -> Result<AnnotationNormalizeSummary, AnnotationError> {
    let mut parsed = read_annotation_path(input.as_ref())?;
    if options.sort {
        parsed.records.sort_by(|left, right| {
            (
                &left.seqid,
                left.start,
                left.end,
                &left.feature_type,
                &left.source,
            )
                .cmp(&(
                    &right.seqid,
                    right.start,
                    right.end,
                    &right.feature_type,
                    &right.source,
                ))
        });
    }
    let converted_gtf_attribute_records = parsed
        .records
        .iter()
        .filter(|record| record.used_gtf_attributes)
        .count() as u64;
    let mut summary = AnnotationNormalizeSummary {
        input_record_count: parsed.records.len() as u64,
        output_record_count: parsed.records.len() as u64,
        sorted: options.sort,
        converted_gtf_attribute_records,
        warnings: Vec::new(),
    };
    if parsed.records.is_empty() {
        summary
            .warnings
            .push("annotation input contains no feature records".to_owned());
    }
    with_new_output(output.as_ref(), |writer| {
        writeln!(writer, "##gff-version 3")?;
        for record in &parsed.records {
            write_gff3_record(writer, record)?;
        }
        Ok(())
    })?;
    Ok(summary)
}

pub fn annotation_gene_positions_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &GenePositionOptions,
) -> Result<GenePositionSummary, AnnotationError> {
    let parsed = read_annotation_path(input.as_ref())?;
    let wanted = normalize_feature_types(&options.feature_types)?;
    let mut summary = GenePositionSummary {
        input_record_count: parsed.records.len() as u64,
        ..Default::default()
    };
    with_new_output(output.as_ref(), |writer| {
        writeln!(
            writer,
            "id\tname\tseqid\tstart\tend\tstrand\tfeature_type\tparent\tsource"
        )?;
        for record in &parsed.records {
            if !wanted.contains(&record.feature_type.to_ascii_lowercase()) {
                continue;
            }
            let identifier = attribute_first(
                record,
                &["ID", "gene_id", "transcript_id", "locus_tag", "Name"],
            );
            let Some(identifier) = identifier else {
                summary.missing_identifier_count =
                    checked_add(summary.missing_identifier_count, 1)?;
                continue;
            };
            let name =
                attribute_first(record, &["Name", "gene_name", "product"]).unwrap_or(identifier);
            let parent = attribute_first(record, &["Parent"]).unwrap_or("");
            writeln!(
                writer,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                sanitize_table_field(identifier),
                sanitize_table_field(name),
                sanitize_table_field(&record.seqid),
                record.start,
                record.end,
                record.strand,
                sanitize_table_field(&record.feature_type),
                sanitize_table_field(parent),
                sanitize_table_field(&record.source)
            )?;
            summary.output_record_count = checked_add(summary.output_record_count, 1)?;
            increment(&mut summary.feature_type_counts, &record.feature_type)?;
        }
        Ok(())
    })?;
    if summary.output_record_count == 0 {
        summary
            .warnings
            .push("no annotation records matched the requested feature types".to_owned());
    }
    if summary.missing_identifier_count > 0 {
        summary.warnings.push(format!(
            "{} matching records were skipped because they had no usable identifier",
            summary.missing_identifier_count
        ));
    }
    Ok(summary)
}

pub fn extract_annotation_sequences_path(
    annotation: impl AsRef<Path>,
    reference_fasta: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &AnnotationExtractOptions,
) -> Result<AnnotationExtractSummary, AnnotationError> {
    let parsed = read_annotation_path(annotation.as_ref())?;
    let references = read_fasta_path(reference_fasta.as_ref())?;
    let requested = canonical_requested_feature_type(&options.feature_type)?;
    if requested == "promoter" && options.promoter_length == 0 {
        return Err(AnnotationError::InvalidOption(
            "promoter_length must be greater than zero".to_owned(),
        ));
    }
    let mut summary = AnnotationExtractSummary {
        annotation_record_count: parsed.records.len() as u64,
        feature_type: requested.to_owned(),
        promoter_length: (requested == "promoter").then_some(options.promoter_length),
        ..Default::default()
    };
    let groups = build_extract_groups(&parsed.records, requested, options, &mut summary)?;
    with_new_output(output.as_ref(), |writer| {
        for (identifier, group) in groups {
            let Some(reference) = references.get(&group.seqid) else {
                summary.missing_reference_count = checked_add(summary.missing_reference_count, 1)?;
                continue;
            };
            let Some(sequence) = extract_group_sequence(reference, &group, requested)? else {
                summary.skipped_feature_count = checked_add(summary.skipped_feature_count, 1)?;
                continue;
            };
            writeln!(
                writer,
                ">{} feature={} seqid={} strand={}",
                sanitize_fasta_identifier(&identifier),
                requested,
                sanitize_fasta_identifier(&group.seqid),
                group.strand
            )?;
            write_wrapped_sequence(writer, &sequence)?;
            summary.output_sequence_count = checked_add(summary.output_sequence_count, 1)?;
            summary.output_base_count = checked_add(
                summary.output_base_count,
                u64::try_from(sequence.len()).map_err(|_| AnnotationError::LimitExceeded {
                    resource: "output base count",
                    limit: u64::MAX,
                })?,
            )?;
        }
        Ok(())
    })?;
    if summary.output_sequence_count == 0 {
        summary
            .warnings
            .push("no sequences were produced for the requested feature type".to_owned());
    }
    if summary.missing_reference_count > 0 {
        summary.warnings.push(format!(
            "{} feature groups referenced sequence identifiers absent from the FASTA input",
            summary.missing_reference_count
        ));
    }
    Ok(summary)
}

fn build_extract_groups(
    records: &[AnnotationRecord],
    requested: &str,
    options: &AnnotationExtractOptions,
    summary: &mut AnnotationExtractSummary,
) -> Result<BTreeMap<String, ExtractGroup>, AnnotationError> {
    let mut groups = BTreeMap::new();
    let direct = matches!(requested, "gene" | "transcript" | "promoter");
    for record in records {
        let matches = if requested == "promoter" {
            feature_matches(&record.feature_type, "gene")
        } else {
            feature_matches(&record.feature_type, requested)
        };
        if !matches {
            continue;
        }
        summary.matched_feature_count = checked_add(summary.matched_feature_count, 1)?;
        let identifier = if direct {
            attribute_first(
                record,
                &["ID", "gene_id", "transcript_id", "locus_tag", "Name"],
            )
        } else {
            attribute_first(record, &["Parent", "transcript_id", "gene_id", "ID"])
        };
        let Some(identifier) = identifier else {
            summary.skipped_feature_count = checked_add(summary.skipped_feature_count, 1)?;
            continue;
        };
        let (start, end) = if requested == "promoter" {
            promoter_coordinates(record, options.promoter_length)?
        } else {
            (record.start, record.end)
        };
        let group = groups
            .entry(identifier.to_owned())
            .or_insert_with(|| ExtractGroup {
                seqid: record.seqid.clone(),
                strand: record.strand,
                segments: Vec::new(),
            });
        if group.seqid != record.seqid || group.strand != record.strand {
            return Err(AnnotationError::InvalidOption(format!(
                "feature group {identifier:?} spans multiple sequences or strands"
            )));
        }
        group.segments.push(ExtractSegment {
            start,
            end,
            phase: record.phase,
        });
    }
    Ok(groups)
}

fn promoter_coordinates(
    record: &AnnotationRecord,
    length: u64,
) -> Result<(u64, u64), AnnotationError> {
    match record.strand {
        '-' => Ok((
            record.end.checked_add(1).ok_or_else(|| {
                AnnotationError::InvalidOption("promoter coordinate overflow".to_owned())
            })?,
            record.end.checked_add(length).ok_or_else(|| {
                AnnotationError::InvalidOption("promoter coordinate overflow".to_owned())
            })?,
        )),
        '+' | '.' | '?' => {
            let end = record.start.saturating_sub(1);
            let start = end.saturating_sub(length.saturating_sub(1)).max(1);
            Ok((start, end))
        }
        strand => Err(AnnotationError::InvalidOption(format!(
            "unsupported annotation strand {strand:?}"
        ))),
    }
}

fn extract_group_sequence(
    reference: &[u8],
    group: &ExtractGroup,
    requested: &str,
) -> Result<Option<Vec<u8>>, AnnotationError> {
    let mut segments = group.segments.clone();
    segments.sort_unstable_by_key(|segment| (segment.start, segment.end));
    let mut sequence = Vec::new();
    for segment in segments {
        if segment.start == 0 || segment.end < segment.start {
            return Ok(None);
        }
        let mut start = usize::try_from(segment.start - 1).map_err(|_| {
            AnnotationError::InvalidOption(
                "annotation coordinate exceeds platform range".to_owned(),
            )
        })?;
        let mut end = usize::try_from(segment.end).map_err(|_| {
            AnnotationError::InvalidOption(
                "annotation coordinate exceeds platform range".to_owned(),
            )
        })?;
        if start >= reference.len() {
            return Ok(None);
        }
        end = end.min(reference.len());
        if requested == "cds" {
            let phase = usize::from(segment.phase.unwrap_or(0));
            if group.strand == '-' {
                end = end.saturating_sub(phase);
            } else {
                start = start.saturating_add(phase);
            }
        }
        if start >= end {
            return Ok(None);
        }
        sequence.extend(reference[start..end].iter().map(u8::to_ascii_uppercase));
    }
    if group.strand == '-' {
        reverse_complement_in_place(&mut sequence);
    }
    Ok((!sequence.is_empty()).then_some(sequence))
}

fn canonical_requested_feature_type(value: &str) -> Result<&'static str, AnnotationError> {
    match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "gene" => Ok("gene"),
        "transcript" | "mrna" => Ok("transcript"),
        "cds" => Ok("cds"),
        "exon" => Ok("exon"),
        "utr" => Ok("utr"),
        "five_prime_utr" | "5_prime_utr" | "5utr" => Ok("five_prime_utr"),
        "three_prime_utr" | "3_prime_utr" | "3utr" => Ok("three_prime_utr"),
        "promoter" => Ok("promoter"),
        value => Err(AnnotationError::InvalidOption(format!(
            "unsupported feature_type {value:?}; expected gene, transcript, cds, exon, utr, five_prime_utr, three_prime_utr, or promoter"
        ))),
    }
}

fn feature_matches(actual: &str, requested: &str) -> bool {
    let actual = actual.to_ascii_lowercase().replace('-', "_");
    match requested {
        "transcript" => matches!(actual.as_str(), "transcript" | "mrna"),
        "utr" => matches!(
            actual.as_str(),
            "utr" | "five_prime_utr" | "three_prime_utr" | "5_prime_utr" | "3_prime_utr"
        ),
        "five_prime_utr" => matches!(actual.as_str(), "five_prime_utr" | "5_prime_utr"),
        "three_prime_utr" => matches!(actual.as_str(), "three_prime_utr" | "3_prime_utr"),
        _ => actual == requested,
    }
}

fn normalize_feature_types(values: &[String]) -> Result<BTreeSet<String>, AnnotationError> {
    if values.is_empty() {
        return Err(AnnotationError::InvalidOption(
            "feature_types must contain at least one value".to_owned(),
        ));
    }
    let values = values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    if values.contains("") {
        return Err(AnnotationError::InvalidOption(
            "feature_types cannot contain an empty value".to_owned(),
        ));
    }
    Ok(values)
}

fn read_annotation_path(path: &Path) -> Result<ParsedAnnotation, AnnotationError> {
    let input = open_maybe_gzip(path)?;
    read_annotation(BufReader::new(input))
}

fn read_annotation(reader: impl BufRead) -> Result<ParsedAnnotation, AnnotationError> {
    let mut reader = reader.take(MAX_ANNOTATION_DECOMPRESSED_BYTES.saturating_add(1));
    let mut parsed = ParsedAnnotation::default();
    let mut line_number = 0_usize;
    let mut decompressed_bytes = 0_u64;
    let mut buffer = String::new();
    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read =
            reader
                .read_line(&mut buffer)
                .map_err(|source| AnnotationError::ReadLine {
                    line: line_number,
                    source,
                })?;
        if bytes_read == 0 {
            break;
        }
        decompressed_bytes = checked_add(decompressed_bytes, bytes_read as u64)?;
        if decompressed_bytes > MAX_ANNOTATION_DECOMPRESSED_BYTES {
            return Err(AnnotationError::LimitExceeded {
                resource: "decompressed byte count",
                limit: MAX_ANNOTATION_DECOMPRESSED_BYTES,
            });
        }
        let line = buffer.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        if line.starts_with("##") {
            parsed.directive_count = checked_add(parsed.directive_count, 1)?;
            if line.starts_with("##sequence-region") {
                parsed.sequence_region_count = checked_add(parsed.sequence_region_count, 1)?;
            }
            continue;
        }
        if line.starts_with('#') {
            parsed.comment_count = checked_add(parsed.comment_count, 1)?;
            continue;
        }
        if parsed.records.len() >= MAX_ANNOTATION_RECORDS {
            return Err(AnnotationError::LimitExceeded {
                resource: "record count",
                limit: MAX_ANNOTATION_RECORDS as u64,
            });
        }
        parsed.records.push(parse_record(line, line_number)?);
    }
    Ok(parsed)
}

fn parse_record(line: &str, line_number: usize) -> Result<AnnotationRecord, AnnotationError> {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() != 9 {
        return malformed(
            line_number,
            format!("expected 9 tab-separated fields, found {}", fields.len()),
        );
    }
    if fields[0].is_empty() || fields[2].is_empty() {
        return malformed(
            line_number,
            "sequence identifier and feature type must be non-empty",
        );
    }
    let start = parse_coordinate(fields[3], line_number, "start")?;
    let end = parse_coordinate(fields[4], line_number, "end")?;
    if start == 0 || end < start {
        return malformed(
            line_number,
            format!("coordinates must be 1-based with end >= start, got {start}-{end}"),
        );
    }
    let strand = match fields[6] {
        "+" => '+',
        "-" => '-',
        "." => '.',
        "?" => '?',
        value => return malformed(line_number, format!("invalid strand {value:?}")),
    };
    let phase = match fields[7] {
        "." => None,
        "0" => Some(0),
        "1" => Some(1),
        "2" => Some(2),
        value => return malformed(line_number, format!("invalid phase {value:?}")),
    };
    let (attributes, used_gtf_attributes) = parse_attributes(fields[8], line_number)?;
    Ok(AnnotationRecord {
        seqid: fields[0].to_owned(),
        source: fields[1].to_owned(),
        feature_type: fields[2].to_owned(),
        start,
        end,
        score: fields[5].to_owned(),
        strand,
        phase,
        attributes,
        used_gtf_attributes,
    })
}

fn parse_attributes(
    value: &str,
    line_number: usize,
) -> Result<(BTreeMap<String, Vec<String>>, bool), AnnotationError> {
    let mut attributes = BTreeMap::new();
    if value == "." || value.trim().is_empty() {
        return Ok((attributes, false));
    }
    let uses_gff3 = value
        .split(';')
        .filter(|part| !part.trim().is_empty())
        .all(|part| {
            part.split_once('=')
                .is_some_and(|(key, _)| !key.trim().is_empty())
        });
    if uses_gff3 {
        for part in value
            .split(';')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let (key, values) = part.split_once('=').expect("checked above");
            let key = decode_gff3(key.trim(), line_number)?;
            let entries = values
                .split(',')
                .map(str::trim)
                .map(|value| decode_gff3(value, line_number))
                .collect::<Result<Vec<_>, _>>()?;
            attributes
                .entry(key)
                .or_insert_with(Vec::new)
                .extend(entries);
        }
        return Ok((attributes, false));
    }
    for part in value
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let Some(split) = part.find(char::is_whitespace) else {
            return malformed(line_number, format!("invalid GTF attribute {part:?}"));
        };
        let key = part[..split].trim();
        let raw_value = part[split..].trim();
        if key.is_empty() || raw_value.is_empty() {
            return malformed(line_number, format!("invalid GTF attribute {part:?}"));
        }
        let decoded = raw_value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .unwrap_or(raw_value);
        attributes
            .entry(key.to_owned())
            .or_insert_with(Vec::new)
            .push(decoded.to_owned());
    }
    Ok((attributes, true))
}

fn write_gff3_record(
    writer: &mut dyn Write,
    record: &AnnotationRecord,
) -> Result<(), AnnotationError> {
    let phase = record
        .phase
        .map(|value| value.to_string())
        .unwrap_or_else(|| ".".to_owned());
    let attributes = if record.attributes.is_empty() {
        ".".to_owned()
    } else {
        record
            .attributes
            .iter()
            .map(|(key, values)| {
                format!(
                    "{}={}",
                    encode_gff3(key),
                    values
                        .iter()
                        .map(|value| encode_gff3(value))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    };
    writeln!(
        writer,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        record.seqid,
        record.source,
        record.feature_type,
        record.start,
        record.end,
        record.score,
        record.strand,
        phase,
        attributes
    )?;
    Ok(())
}

fn encode_gff3(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\t' => output.push_str("%09"),
            '\n' => output.push_str("%0A"),
            '\r' => output.push_str("%0D"),
            '%' => output.push_str("%25"),
            ';' => output.push_str("%3B"),
            '=' => output.push_str("%3D"),
            ',' => output.push_str("%2C"),
            character => output.push(character),
        }
    }
    output
}

fn decode_gff3(value: &str, line_number: usize) -> Result<String, AnnotationError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return malformed(
                    line_number,
                    format!("incomplete percent escape in {value:?}"),
                );
            }
            let high = hex_value(bytes[index + 1]);
            let low = hex_value(bytes[index + 2]);
            let (Some(high), Some(low)) = (high, low) else {
                return malformed(line_number, format!("invalid percent escape in {value:?}"));
            };
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| AnnotationError::MalformedRecord {
        line: line_number,
        message: format!("attribute contains invalid UTF-8 after decoding: {value:?}"),
    })
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn read_fasta_path(path: &Path) -> Result<BTreeMap<String, Vec<u8>>, AnnotationError> {
    let input = open_maybe_gzip(path)?;
    let mut reader =
        BufReader::new(input).take(MAX_ANNOTATION_DECOMPRESSED_BYTES.saturating_add(1));
    let mut sequences = BTreeMap::new();
    let mut current_id: Option<String> = None;
    let mut current_sequence = Vec::new();
    let mut buffer = String::new();
    let mut line_number = 0_usize;
    let mut bytes = 0_u64;
    loop {
        line_number += 1;
        buffer.clear();
        let read = reader
            .read_line(&mut buffer)
            .map_err(|source| AnnotationError::ReadLine {
                line: line_number,
                source,
            })?;
        if read == 0 {
            break;
        }
        bytes = checked_add(bytes, read as u64)?;
        if bytes > MAX_ANNOTATION_DECOMPRESSED_BYTES {
            return Err(AnnotationError::LimitExceeded {
                resource: "reference FASTA decompressed byte count",
                limit: MAX_ANNOTATION_DECOMPRESSED_BYTES,
            });
        }
        let line = buffer.trim_end_matches(['\r', '\n']);
        if let Some(header) = line.strip_prefix('>') {
            if let Some(identifier) = current_id.take()
                && sequences
                    .insert(identifier.clone(), std::mem::take(&mut current_sequence))
                    .is_some()
            {
                return Err(AnnotationError::InvalidOption(format!(
                    "duplicate FASTA identifier {identifier:?}"
                )));
            }
            let identifier = header.split_whitespace().next().unwrap_or_default();
            if identifier.is_empty() {
                return malformed(line_number, "FASTA identifier is empty");
            }
            current_id = Some(identifier.to_owned());
        } else if line.trim().is_empty() {
            continue;
        } else if current_id.is_none() {
            return malformed(
                line_number,
                "FASTA sequence appears before the first header",
            );
        } else {
            for byte in line.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
                if !byte.is_ascii_alphabetic() && byte != b'*' && byte != b'-' {
                    return malformed(line_number, format!("invalid FASTA byte 0x{byte:02x}"));
                }
                current_sequence.push(byte.to_ascii_uppercase());
            }
        }
    }
    if let Some(identifier) = current_id
        && sequences
            .insert(identifier.clone(), current_sequence)
            .is_some()
    {
        return Err(AnnotationError::InvalidOption(format!(
            "duplicate FASTA identifier {identifier:?}"
        )));
    }
    Ok(sequences)
}

fn open_maybe_gzip(path: &Path) -> Result<Box<dyn Read>, AnnotationError> {
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Ok(Box::new(MultiGzDecoder::new(File::open(path)?)))
    } else {
        Ok(Box::new(File::open(path)?))
    }
}

fn with_new_output<T>(
    path: &Path,
    operation: impl FnOnce(&mut BufWriter<File>) -> Result<T, AnnotationError>,
) -> Result<T, AnnotationError> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                AnnotationError::OutputAlreadyExists(path.to_owned())
            } else {
                AnnotationError::Io(error)
            }
        })?;
    let mut writer = BufWriter::new(file);
    match operation(&mut writer).and_then(|value| {
        writer.flush()?;
        Ok(value)
    }) {
        Ok(value) => Ok(value),
        Err(error) => {
            drop(writer);
            let _ = fs::remove_file(path);
            Err(error)
        }
    }
}

fn attribute_first<'a>(record: &'a AnnotationRecord, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        record
            .attributes
            .get(*key)
            .and_then(|values| values.first())
            .map(String::as_str)
    })
}

fn increment(map: &mut BTreeMap<String, u64>, key: &str) -> Result<(), AnnotationError> {
    let value = map.entry(key.to_owned()).or_default();
    *value = checked_add(*value, 1)?;
    Ok(())
}

fn checked_add(left: u64, right: u64) -> Result<u64, AnnotationError> {
    left.checked_add(right)
        .ok_or(AnnotationError::LimitExceeded {
            resource: "counter",
            limit: u64::MAX,
        })
}

fn parse_coordinate(value: &str, line: usize, label: &str) -> Result<u64, AnnotationError> {
    value
        .parse::<u64>()
        .map_err(|_| AnnotationError::MalformedRecord {
            line,
            message: format!("invalid {label} coordinate {value:?}"),
        })
}

fn malformed<T>(line: usize, message: impl Into<String>) -> Result<T, AnnotationError> {
    Err(AnnotationError::MalformedRecord {
        line,
        message: message.into(),
    })
}

fn sanitize_table_field(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ")
}

fn sanitize_fasta_identifier(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_whitespace() || matches!(character, '>' | '|' | '=') {
                '_'
            } else {
                character
            }
        })
        .collect()
}

fn write_wrapped_sequence(writer: &mut dyn Write, sequence: &[u8]) -> Result<(), AnnotationError> {
    for chunk in sequence.chunks(80) {
        writer.write_all(chunk)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn reverse_complement_in_place(sequence: &mut [u8]) {
    sequence.reverse();
    for base in sequence {
        *base = match *base {
            b'A' => b'T',
            b'C' => b'G',
            b'G' => b'C',
            b'T' | b'U' => b'A',
            b'R' => b'Y',
            b'Y' => b'R',
            b'S' => b'S',
            b'W' => b'W',
            b'K' => b'M',
            b'M' => b'K',
            b'B' => b'V',
            b'D' => b'H',
            b'H' => b'D',
            b'V' => b'B',
            b'N' => b'N',
            base => base,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Cursor;
    use std::time::{SystemTime, UNIX_EPOCH};

    const GFF: &str = "##gff-version 3\nchr1\tsrc\tgene\t2\t12\t.\t+\t.\tID=g1;Name=Gene1\nchr1\tsrc\tmRNA\t2\t12\t.\t+\t.\tID=t1;Parent=g1\nchr1\tsrc\texon\t2\t4\t.\t+\t.\tParent=t1\nchr1\tsrc\texon\t9\t12\t.\t+\t.\tParent=t1\n";

    #[test]
    fn parses_gff3_and_counts_features() {
        let parsed = read_annotation(Cursor::new(GFF)).expect("annotation parses");
        assert_eq!(parsed.directive_count, 1);
        assert_eq!(parsed.records.len(), 4);
        assert_eq!(parsed.records[0].attributes["ID"], vec!["g1"]);
    }

    #[test]
    fn parses_gtf_attributes() {
        let input = "chr1\tsrc\texon\t1\t3\t.\t-\t.\tgene_id \"g1\"; transcript_id \"t1\";\n";
        let parsed = read_annotation(Cursor::new(input)).expect("GTF parses");
        assert!(parsed.records[0].used_gtf_attributes);
        assert_eq!(parsed.records[0].attributes["gene_id"], vec!["g1"]);
    }

    #[test]
    fn gff3_attribute_escaping_round_trips_unicode_and_reserved_bytes() {
        let decoded = "蛋白;alpha,beta%";
        let encoded = encode_gff3(decoded);
        assert_eq!(encoded, "蛋白%3Balpha%2Cbeta%25");
        assert_eq!(decode_gff3(&encoded, 1).expect("decode"), decoded);
    }

    #[test]
    fn spliced_minus_strand_sequence_is_reverse_complemented() {
        let group = ExtractGroup {
            seqid: "chr1".to_owned(),
            strand: '-',
            segments: vec![
                ExtractSegment {
                    start: 1,
                    end: 3,
                    phase: None,
                },
                ExtractSegment {
                    start: 7,
                    end: 9,
                    phase: None,
                },
            ],
        };
        let sequence = extract_group_sequence(b"AAACCCGGG", &group, "exon")
            .expect("extracts")
            .expect("sequence");
        assert_eq!(sequence, b"CCCTTT");
    }

    #[test]
    fn rejects_invalid_coordinates() {
        let input = "chr1\tsrc\tgene\t0\t3\t.\t+\t.\tID=g1\n";
        assert!(matches!(
            read_annotation(Cursor::new(input)),
            Err(AnnotationError::MalformedRecord { .. })
        ));
    }

    #[test]
    fn computes_overlapping_gene_density_windows() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("linxira-density-{stamp}.gff3"));
        fs::write(
            &path,
            "chr1\tsrc\tgene\t2\t6\t.\t+\t.\tID=g1\nchr1\tsrc\tgene\t9\t12\t.\t+\t.\tID=g2\nchr1\tsrc\texon\t2\t3\t.\t+\t.\tParent=g1\n",
        )
        .expect("write density fixture");
        let result = gene_density_path(
            &path,
            GeneDensityOptions {
                feature_types: vec!["gene".to_owned()],
                window_size: 6,
                step_size: 4,
            },
        )
        .expect("compute gene density");
        fs::remove_file(path).expect("remove density fixture");
        assert_eq!(result.selected_feature_count, 2);
        assert_eq!(result.bins.len(), 3);
        assert_eq!(result.bins[0].feature_count, 1);
        assert_eq!(result.bins[1].feature_count, 2);
        assert_eq!(result.bins[2].feature_count, 1);
    }
}
