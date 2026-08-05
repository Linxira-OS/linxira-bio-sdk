use crate::sequence_transform::{SequenceTransformError, visit_fasta_path};
use crate::variant::{VcfError, vcf_stats_path};
use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum VariantTransformError {
    Io(io::Error),
    Vcf(VcfError),
    Fasta(SequenceTransformError),
    OutputAlreadyExists(PathBuf),
    InvalidOption(String),
    InvalidRecord { line: usize, message: String },
}

impl Display for VariantTransformError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "variant operation failed: {error}"),
            Self::Vcf(error) => Display::fmt(error, formatter),
            Self::Fasta(error) => Display::fmt(error, formatter),
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::InvalidRecord { line, message } => {
                write!(formatter, "invalid VCF record at line {line}: {message}")
            }
        }
    }
}

impl Error for VariantTransformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Vcf(error) => Some(error),
            Self::Fasta(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for VariantTransformError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<VcfError> for VariantTransformError {
    fn from(error: VcfError) -> Self {
        Self::Vcf(error)
    }
}

impl From<SequenceTransformError> for VariantTransformError {
    fn from(error: SequenceTransformError) -> Self {
        Self::Fasta(error)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VariantFilterOptions {
    pub min_qual: Option<f64>,
    pub require_pass: bool,
    pub contigs: Vec<String>,
    pub min_info_dp: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantFilterSummary {
    pub input_records: u64,
    pub output_records: u64,
    pub rejected_by_qual: u64,
    pub rejected_by_filter: u64,
    pub rejected_by_contig: u64,
    pub rejected_by_info_dp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum VariantComparisonStatus {
    Shared,
    LeftOnly,
    RightOnly,
}

impl VariantComparisonStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::LeftOnly => "left-only",
            Self::RightOnly => "right-only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VariantAlleleKey {
    chrom: String,
    position: u64,
    reference: String,
    alternate: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantComparisonRow {
    pub chrom: String,
    pub position: u64,
    pub reference: String,
    pub alternate: String,
    pub status: VariantComparisonStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantComparisonResult {
    pub shared_count: u64,
    pub left_only_count: u64,
    pub right_only_count: u64,
    pub sample_genotypes_compared: bool,
    pub variants: Vec<VariantComparisonRow>,
}

pub fn compare_vcf_paths(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
) -> Result<VariantComparisonResult, VariantTransformError> {
    let left = collect_variant_alleles(left.as_ref())?;
    let right = collect_variant_alleles(right.as_ref())?;
    let mut shared_count = 0_u64;
    let mut left_only_count = 0_u64;
    let mut right_only_count = 0_u64;
    let mut variants = Vec::with_capacity(left.len().saturating_add(right.len()));

    for key in left.union(&right) {
        let status = match (left.contains(key), right.contains(key)) {
            (true, true) => {
                shared_count += 1;
                VariantComparisonStatus::Shared
            }
            (true, false) => {
                left_only_count += 1;
                VariantComparisonStatus::LeftOnly
            }
            (false, true) => {
                right_only_count += 1;
                VariantComparisonStatus::RightOnly
            }
            (false, false) => unreachable!("set union contains a key from at least one input"),
        };
        variants.push(VariantComparisonRow {
            chrom: key.chrom.clone(),
            position: key.position,
            reference: key.reference.clone(),
            alternate: key.alternate.clone(),
            status,
        });
    }

    Ok(VariantComparisonResult {
        shared_count,
        left_only_count,
        right_only_count,
        sample_genotypes_compared: false,
        variants,
    })
}

fn collect_variant_alleles(
    input: &Path,
) -> Result<BTreeSet<VariantAlleleKey>, VariantTransformError> {
    vcf_stats_path(input)?;
    let mut reader = open_vcf_reader(input)?;
    let mut variants = BTreeSet::new();
    let mut buffer = String::new();
    let mut line_number = 0_usize;
    loop {
        buffer.clear();
        if reader.read_line(&mut buffer)? == 0 {
            break;
        }
        line_number += 1;
        if buffer.starts_with('#') {
            continue;
        }
        let columns = buffer
            .trim_end_matches(['\r', '\n'])
            .split('\t')
            .collect::<Vec<_>>();
        let position =
            columns[1]
                .parse::<u64>()
                .map_err(|_| VariantTransformError::InvalidRecord {
                    line: line_number,
                    message: "POS does not fit the comparison key".to_owned(),
                })?;
        if columns[4] == "." {
            continue;
        }
        for alternate in columns[4].split(',') {
            variants.insert(canonical_variant_key(
                columns[0],
                position,
                columns[3],
                alternate,
                line_number,
            )?);
        }
    }
    Ok(variants)
}

fn canonical_variant_key(
    chrom: &str,
    mut position: u64,
    reference: &str,
    alternate: &str,
    line: usize,
) -> Result<VariantAlleleKey, VariantTransformError> {
    let mut reference = normalize_allele(reference, line)?;
    if is_symbolic_allele(alternate) {
        return Ok(VariantAlleleKey {
            chrom: chrom.to_owned(),
            position,
            reference: String::from_utf8(reference)
                .expect("validated reference alleles contain ASCII nucleotides"),
            alternate: alternate.to_owned(),
        });
    }
    let mut alternate = normalize_allele(alternate, line)?;
    trim_comparison_alleles(&mut position, &mut reference, &mut alternate, line)?;
    if reference == alternate {
        return Err(VariantTransformError::InvalidRecord {
            line,
            message: "REF and ALT are identical after comparison normalization".to_owned(),
        });
    }
    Ok(VariantAlleleKey {
        chrom: chrom.to_owned(),
        position,
        reference: String::from_utf8(reference)
            .expect("validated reference alleles contain ASCII nucleotides"),
        alternate: String::from_utf8(alternate)
            .expect("validated alternate alleles contain ASCII nucleotides"),
    })
}

fn trim_comparison_alleles(
    position: &mut u64,
    reference: &mut Vec<u8>,
    alternate: &mut Vec<u8>,
    line: usize,
) -> Result<(), VariantTransformError> {
    while reference.len() > 1 && alternate.len() > 1 && reference.last() == alternate.last() {
        reference.pop();
        alternate.pop();
    }
    let mut prefix = 0_usize;
    while reference.len() - prefix > 1
        && alternate.len() - prefix > 1
        && reference[prefix] == alternate[prefix]
    {
        prefix += 1;
    }
    if prefix > 0 {
        reference.drain(..prefix);
        alternate.drain(..prefix);
        *position = position
            .checked_add(u64::try_from(prefix).expect("allele length fits in u64"))
            .ok_or_else(|| VariantTransformError::InvalidRecord {
                line,
                message: "normalized POS overflows the comparison key".to_owned(),
            })?;
    }
    Ok(())
}

pub fn filter_vcf_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &VariantFilterOptions,
) -> Result<VariantFilterSummary, VariantTransformError> {
    if options.min_qual.is_some_and(|value| !value.is_finite()) {
        return Err(VariantTransformError::InvalidOption(
            "minimum QUAL must be finite".to_owned(),
        ));
    }
    let input = input.as_ref();
    vcf_stats_path(input)?;
    let allowed_contigs = options.contigs.iter().cloned().collect::<BTreeSet<_>>();
    if allowed_contigs.contains("") {
        return Err(VariantTransformError::InvalidOption(
            "contig filters must not be empty".to_owned(),
        ));
    }
    let mut summary = VariantFilterSummary {
        input_records: 0,
        output_records: 0,
        rejected_by_qual: 0,
        rejected_by_filter: 0,
        rejected_by_contig: 0,
        rejected_by_info_dp: 0,
    };

    with_new_vcf_output(output.as_ref(), |writer| {
        let mut reader = open_vcf_reader(input)?;
        let mut buffer = String::new();
        let mut line_number = 0_usize;
        loop {
            buffer.clear();
            if reader.read_line(&mut buffer)? == 0 {
                break;
            }
            line_number += 1;
            if buffer.starts_with('#') {
                writer.write_all(buffer.as_bytes())?;
                continue;
            }
            let line = buffer.trim_end_matches(['\r', '\n']);
            let columns = line.split('\t').collect::<Vec<_>>();
            summary.input_records += 1;

            if let Some(minimum) = options.min_qual {
                let passes = columns[5] != "."
                    && columns[5]
                        .parse::<f64>()
                        .is_ok_and(|quality| quality.is_finite() && quality >= minimum);
                if !passes {
                    summary.rejected_by_qual += 1;
                    continue;
                }
            }
            if options.require_pass && columns[6] != "PASS" {
                summary.rejected_by_filter += 1;
                continue;
            }
            if !allowed_contigs.is_empty() && !allowed_contigs.contains(columns[0]) {
                summary.rejected_by_contig += 1;
                continue;
            }
            if let Some(minimum) = options.min_info_dp {
                let depth = info_integer(columns[7], "DP", line_number)?;
                if depth.is_none_or(|depth| depth < minimum) {
                    summary.rejected_by_info_dp += 1;
                    continue;
                }
            }
            writer.write_all(buffer.as_bytes())?;
            if !buffer.ends_with('\n') {
                writer.write_all(b"\n")?;
            }
            summary.output_records += 1;
        }
        Ok(())
    })?;
    Ok(summary)
}

fn info_integer(info: &str, key: &str, line: usize) -> Result<Option<u64>, VariantTransformError> {
    for field in info.split(';') {
        let Some((field_key, value)) = field.split_once('=') else {
            continue;
        };
        if field_key == key {
            return value.parse::<u64>().map(Some).map_err(|_| {
                VariantTransformError::InvalidRecord {
                    line,
                    message: format!("INFO/{key} must be a non-negative integer"),
                }
            });
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariantNormalizeSummary {
    pub input_records: u64,
    pub output_records: u64,
    pub changed_records: u64,
    pub left_aligned_records: u64,
    pub reference_validated_records: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct VcfToTableSummary {
    pub input_record_count: u64,
    pub output_record_count: u64,
    pub sample_count: u64,
    pub warnings: Vec<String>,
}

pub fn normalize_vcf_path(
    input: impl AsRef<Path>,
    reference: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<VariantNormalizeSummary, VariantTransformError> {
    let input = input.as_ref();
    vcf_stats_path(input)?;
    let reference_sequences = load_reference(reference.as_ref())?;
    let mut summary = VariantNormalizeSummary {
        input_records: 0,
        output_records: 0,
        changed_records: 0,
        left_aligned_records: 0,
        reference_validated_records: 0,
    };

    with_new_vcf_output(output.as_ref(), |writer| {
        let mut reader = open_vcf_reader(input)?;
        let mut buffer = String::new();
        let mut line_number = 0_usize;
        loop {
            buffer.clear();
            if reader.read_line(&mut buffer)? == 0 {
                break;
            }
            line_number += 1;
            if buffer.starts_with('#') {
                writer.write_all(buffer.as_bytes())?;
                continue;
            }
            let line = buffer.trim_end_matches(['\r', '\n']);
            let mut columns = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
            summary.input_records += 1;
            let contig = reference_sequences.get(&columns[0]).ok_or_else(|| {
                VariantTransformError::InvalidRecord {
                    line: line_number,
                    message: format!("reference FASTA has no contig {:?}", columns[0]),
                }
            })?;
            if columns[4] == "." || columns[4].contains(',') {
                return Err(VariantTransformError::InvalidRecord {
                    line: line_number,
                    message: "normalization requires exactly one ALT allele".to_owned(),
                });
            }
            if is_symbolic_allele(&columns[3]) || is_symbolic_allele(&columns[4]) {
                return Err(VariantTransformError::InvalidRecord {
                    line: line_number,
                    message: "symbolic, spanning-deletion, and breakend alleles are unsupported"
                        .to_owned(),
                });
            }
            let position =
                columns[1]
                    .parse::<usize>()
                    .map_err(|_| VariantTransformError::InvalidRecord {
                        line: line_number,
                        message: "POS does not fit this platform".to_owned(),
                    })?;
            let reference_allele = normalize_allele(&columns[3], line_number)?;
            let alternate_allele = normalize_allele(&columns[4], line_number)?;
            validate_reference(contig, position, &reference_allele, line_number)?;
            summary.reference_validated_records += 1;
            let normalized = normalize_small_variant(
                contig,
                position,
                reference_allele,
                alternate_allele,
                line_number,
            )?;
            if normalized.position != position
                || normalized.reference != columns[3].as_bytes()
                || normalized.alternate != columns[4].as_bytes()
            {
                summary.changed_records += 1;
            }
            if normalized.position < position {
                summary.left_aligned_records += 1;
            }
            columns[1] = normalized.position.to_string();
            columns[3] = String::from_utf8(normalized.reference)
                .expect("validated alleles contain ASCII nucleotides");
            columns[4] = String::from_utf8(normalized.alternate)
                .expect("validated alleles contain ASCII nucleotides");
            writeln!(writer, "{}", columns.join("\t"))?;
            summary.output_records += 1;
        }
        Ok(())
    })?;
    Ok(summary)
}

struct NormalizedVariant {
    position: usize,
    reference: Vec<u8>,
    alternate: Vec<u8>,
}

fn normalize_small_variant(
    genome: &[u8],
    mut position: usize,
    mut reference: Vec<u8>,
    mut alternate: Vec<u8>,
    line: usize,
) -> Result<NormalizedVariant, VariantTransformError> {
    trim_common(&mut position, &mut reference, &mut alternate);
    if reference == alternate {
        return Err(VariantTransformError::InvalidRecord {
            line,
            message: "REF and ALT are identical after normalization".to_owned(),
        });
    }
    if reference.len() != alternate.len() {
        loop {
            if position <= 1 {
                break;
            }
            let previous = genome[position - 2];
            let (insertion, payload) =
                if reference.len() == 1 && alternate.first() == reference.first() {
                    (true, &alternate[1..])
                } else if alternate.len() == 1 && reference.first() == alternate.first() {
                    (false, &reference[1..])
                } else {
                    break;
                };
            if payload.last() != Some(&previous) {
                break;
            }
            let mut rotated = Vec::with_capacity(payload.len());
            rotated.push(previous);
            rotated.extend_from_slice(&payload[..payload.len() - 1]);
            position -= 1;
            let anchor = genome[position - 1];
            if insertion {
                reference = vec![anchor];
                alternate = vec![anchor];
                alternate.extend_from_slice(&rotated);
            } else {
                reference = vec![anchor];
                reference.extend_from_slice(&rotated);
                alternate = vec![anchor];
            }
        }
    }
    Ok(NormalizedVariant {
        position,
        reference,
        alternate,
    })
}

fn trim_common(position: &mut usize, reference: &mut Vec<u8>, alternate: &mut Vec<u8>) {
    while reference.len() > 1 && alternate.len() > 1 && reference.last() == alternate.last() {
        reference.pop();
        alternate.pop();
    }
    let mut prefix = 0_usize;
    while reference.len() - prefix > 1
        && alternate.len() - prefix > 1
        && reference[prefix] == alternate[prefix]
    {
        prefix += 1;
    }
    if prefix > 0 {
        reference.drain(..prefix);
        alternate.drain(..prefix);
        *position += prefix;
    }
}

fn normalize_allele(value: &str, line: usize) -> Result<Vec<u8>, VariantTransformError> {
    if value.is_empty() {
        return Err(VariantTransformError::InvalidRecord {
            line,
            message: "alleles must not be empty".to_owned(),
        });
    }
    value
        .bytes()
        .map(|byte| match byte.to_ascii_uppercase() {
            b'A' | b'C' | b'G' | b'T' | b'N' => Ok(byte.to_ascii_uppercase()),
            symbol => Err(VariantTransformError::InvalidRecord {
                line,
                message: format!("unsupported allele byte 0x{symbol:02x}"),
            }),
        })
        .collect()
}

fn validate_reference(
    genome: &[u8],
    position: usize,
    reference: &[u8],
    line: usize,
) -> Result<(), VariantTransformError> {
    let start = position
        .checked_sub(1)
        .ok_or_else(|| VariantTransformError::InvalidRecord {
            line,
            message: "POS must be positive".to_owned(),
        })?;
    let end =
        start
            .checked_add(reference.len())
            .ok_or_else(|| VariantTransformError::InvalidRecord {
                line,
                message: "REF interval overflows this platform".to_owned(),
            })?;
    if genome.get(start..end) != Some(reference) {
        return Err(VariantTransformError::InvalidRecord {
            line,
            message: "REF does not match the supplied reference FASTA".to_owned(),
        });
    }
    Ok(())
}

fn is_symbolic_allele(allele: &str) -> bool {
    allele == "*"
        || allele.starts_with('<')
        || allele.ends_with('>')
        || allele.contains('[')
        || allele.contains(']')
}

fn load_reference(path: &Path) -> Result<HashMap<String, Vec<u8>>, VariantTransformError> {
    let mut references = HashMap::new();
    visit_fasta_path(path, |record| {
        let sequence = record
            .sequence
            .iter()
            .copied()
            .enumerate()
            .map(|(index, byte)| match byte.to_ascii_uppercase() {
                b'A' | b'C' | b'G' | b'T' | b'N' => Ok(byte.to_ascii_uppercase()),
                symbol => Err(SequenceTransformError::InvalidNucleotide {
                    identifier: record.identifier.clone(),
                    position: index + 1,
                    symbol,
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if references
            .insert(record.identifier.clone(), sequence)
            .is_some()
        {
            return Err(SequenceTransformError::InvalidOption(format!(
                "reference FASTA contains duplicate identifier {:?}",
                record.identifier
            )));
        }
        Ok(())
    })?;
    Ok(references)
}

fn open_vcf_reader(path: &Path) -> Result<Box<dyn BufRead>, VariantTransformError> {
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    Ok(Box::new(BufReader::new(input)))
}

fn with_new_vcf_output(
    output: &Path,
    operation: impl FnOnce(&mut BufWriter<File>) -> Result<(), VariantTransformError>,
) -> Result<(), VariantTransformError> {
    if output.exists() {
        return Err(VariantTransformError::OutputAlreadyExists(
            output.to_owned(),
        ));
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut writer = BufWriter::new(file);
    match operation(&mut writer).and_then(|()| {
        writer.flush()?;
        Ok(())
    }) {
        Ok(()) => Ok(()),
        Err(error) => {
            drop(writer);
            let _ = fs::remove_file(output);
            Err(error)
        }
    }
}

pub fn vcf_to_table_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<VcfToTableSummary, VcfError> {
    let input = input.as_ref();
    let output = output.as_ref();
    if output.exists() {
        return Err(VcfError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "refusing to overwrite existing output: {}",
                output.display()
            ),
        )));
    }

    let mut magic = [0_u8; 2];
    let magic_length = File::open(input)?.read(&mut magic)?;
    let input_reader: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(input)?))
    } else {
        Box::new(File::open(input)?)
    };

    let mut reader = BufReader::new(input_reader);
    let mut buffer = String::new();
    let mut line_number = 0_usize;
    let mut sample_names: Vec<String> = Vec::new();
    let mut header_found = false;
    let mut saw_file_format = false;

    // Read header lines
    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read = reader
            .read_line(&mut buffer)
            .map_err(|source| VcfError::ReadLine {
                line: line_number,
                source,
            })?;
        if bytes_read == 0 {
            break;
        }

        let line = buffer.trim_end_matches(['\r', '\n']);
        if line_number == 1 {
            if !line.starts_with("##fileformat=VCFv") {
                return Err(VcfError::InvalidHeader {
                    line: 1,
                    message: "the first line must be a ##fileformat=VCFv... declaration".to_owned(),
                });
            }
            saw_file_format = true;
            continue;
        }
        if line.starts_with("##") {
            continue;
        }
        if line.starts_with('#') {
            if !saw_file_format {
                return Err(VcfError::InvalidHeader {
                    line: line_number,
                    message: "missing fileformat declaration".to_owned(),
                });
            }
            let columns: Vec<&str> = line.split('\t').collect();
            const REQUIRED: [&str; 8] = [
                "#CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER", "INFO",
            ];
            if columns.len() < REQUIRED.len() {
                return Err(VcfError::InvalidHeader {
                    line: line_number,
                    message: format!(
                        "expected at least 8 tab-separated columns, found {}",
                        columns.len()
                    ),
                });
            }
            for (index, expected) in REQUIRED.iter().enumerate() {
                if columns[index] != *expected {
                    return Err(VcfError::InvalidHeader {
                        line: line_number,
                        message: format!(
                            "column {} must be {expected}, found {:?}",
                            index + 1,
                            columns[index]
                        ),
                    });
                }
            }
            if columns.len() > REQUIRED.len() && columns[8] != "FORMAT" {
                return Err(VcfError::InvalidHeader {
                    line: line_number,
                    message: format!("column 9 must be FORMAT, found {:?}", columns[8]),
                });
            }
            sample_names = columns
                .get(9..)
                .unwrap_or_default()
                .iter()
                .map(|s| s.to_string())
                .collect();
            header_found = true;
            break;
        }
        return Err(VcfError::InvalidHeader {
            line: line_number,
            message: "record data appears before the #CHROM column header".to_owned(),
        });
    }

    if !header_found {
        return Err(VcfError::MissingHeader);
    }

    let sample_count = u64::try_from(sample_names.len()).expect("sample count fits in u64");

    // Write output
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut writer = BufWriter::new(file);

    let result = (|| -> Result<VcfToTableSummary, VcfError> {
        // Write header
        let mut header_parts = vec![
            "CHROM".to_owned(),
            "POS".to_owned(),
            "ID".to_owned(),
            "REF".to_owned(),
            "ALT".to_owned(),
            "QUAL".to_owned(),
            "FILTER".to_owned(),
            "INFO".to_owned(),
        ];
        for name in &sample_names {
            header_parts.push(name.clone());
        }
        writeln!(writer, "{}", header_parts.join("\t")).map_err(VcfError::Io)?;

        let mut input_record_count = 0_u64;
        let mut warnings = Vec::new();

        // Read records
        loop {
            buffer.clear();
            let bytes_read =
                reader
                    .read_line(&mut buffer)
                    .map_err(|source| VcfError::ReadLine {
                        line: line_number + 1,
                        source,
                    })?;
            if bytes_read == 0 {
                break;
            }
            line_number += 1;

            let line = buffer.trim_end_matches(['\r', '\n']);
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let columns: Vec<&str> = line.split('\t').collect();
            if columns.len() < 8 {
                continue;
            }

            input_record_count += 1;

            let mut row_parts = Vec::with_capacity(8 + sample_names.len());
            for column in columns.iter().take(8) {
                row_parts.push(column.to_string());
            }
            for _ in 0..sample_names.len() {
                row_parts.push(".".to_owned());
            }

            if sample_names.is_empty() {
                // no samples
            } else if columns.len() > 8 {
                let sample_cols = &columns[9..];
                for (i, sample_col) in sample_cols.iter().enumerate() {
                    if i < sample_names.len() {
                        row_parts[8 + i] = sample_col.to_string();
                    }
                }
            }

            writeln!(writer, "{}", row_parts.join("\t")).map_err(VcfError::Io)?;
        }

        if input_record_count == 0 {
            warnings.push("VCF contains no variant records".to_owned());
        }

        Ok(VcfToTableSummary {
            input_record_count,
            output_record_count: input_record_count,
            sample_count,
            warnings,
        })
    })();

    match result {
        Ok(summary) => {
            writer.flush().map_err(VcfError::Io)?;
            Ok(summary)
        }
        Err(error) => {
            drop(writer);
            let _ = fs::remove_file(output);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("linxira-{nonce}-{name}"))
    }

    fn comparison_fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/variant-compare")
            .join(name)
    }

    fn write_vcf(path: &Path, column_header: &str, records: &str) {
        fs::write(
            path,
            format!("##fileformat=VCFv4.2\n{column_header}\n{records}"),
        )
        .unwrap();
    }

    #[test]
    fn compares_fixture_alleles_in_stable_order() {
        let left = comparison_fixture("left.vcf");
        let right = comparison_fixture("right.vcf");
        let result = compare_vcf_paths(&left, &right).unwrap();

        assert_eq!(result.shared_count, 3);
        assert_eq!(result.left_only_count, 2);
        assert_eq!(result.right_only_count, 1);
        assert!(!result.sample_genotypes_compared);
        assert_eq!(
            result.variants,
            vec![
                VariantComparisonRow {
                    chrom: "chr1".to_owned(),
                    position: 5,
                    reference: "G".to_owned(),
                    alternate: "A".to_owned(),
                    status: VariantComparisonStatus::LeftOnly,
                },
                VariantComparisonRow {
                    chrom: "chr1".to_owned(),
                    position: 6,
                    reference: "C".to_owned(),
                    alternate: "G".to_owned(),
                    status: VariantComparisonStatus::RightOnly,
                },
                VariantComparisonRow {
                    chrom: "chr1".to_owned(),
                    position: 11,
                    reference: "C".to_owned(),
                    alternate: "T".to_owned(),
                    status: VariantComparisonStatus::Shared,
                },
                VariantComparisonRow {
                    chrom: "chr2".to_owned(),
                    position: 20,
                    reference: "A".to_owned(),
                    alternate: "C".to_owned(),
                    status: VariantComparisonStatus::Shared,
                },
                VariantComparisonRow {
                    chrom: "chr2".to_owned(),
                    position: 20,
                    reference: "A".to_owned(),
                    alternate: "G".to_owned(),
                    status: VariantComparisonStatus::LeftOnly,
                },
                VariantComparisonRow {
                    chrom: "chr3".to_owned(),
                    position: 7,
                    reference: "T".to_owned(),
                    alternate: "<DEL>".to_owned(),
                    status: VariantComparisonStatus::Shared,
                },
            ]
        );
        assert_eq!(result, compare_vcf_paths(left, right).unwrap());
    }

    #[test]
    fn splits_multiallelic_records_and_collapses_duplicate_alleles() {
        let left = temp_path("compare-multi-left.vcf");
        let right = temp_path("compare-multi-right.vcf");
        let header = "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO";
        write_vcf(
            &left,
            header,
            "chr1\t2\t.\tA\tC,G\t50\tPASS\t.\nchr1\t2\tduplicate\ta\tg\t50\tPASS\t.\n",
        );
        write_vcf(&right, header, "chr1\t2\t.\tA\tG\t50\tPASS\t.\n");

        let result = compare_vcf_paths(&left, &right).unwrap();
        assert_eq!(result.shared_count, 1);
        assert_eq!(result.left_only_count, 1);
        assert_eq!(result.right_only_count, 0);
        assert_eq!(result.variants.len(), 2);
        assert_eq!(result.variants[0].alternate, "C");
        assert_eq!(result.variants[1].alternate, "G");

        let _ = fs::remove_file(left);
        let _ = fs::remove_file(right);
    }

    #[test]
    fn compares_alleles_without_claiming_genotype_concordance() {
        let left = temp_path("compare-gt-left.vcf");
        let right = temp_path("compare-gt-right.vcf");
        let header = "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample";
        write_vcf(&left, header, "chr1\t2\t.\tA\tG\t50\tPASS\t.\tGT\t0/1\n");
        write_vcf(&right, header, "chr1\t2\t.\tA\tG\t50\tPASS\t.\tGT\t1/1\n");

        let result = compare_vcf_paths(&left, &right).unwrap();
        assert_eq!(result.shared_count, 1);
        assert_eq!(result.variants[0].status, VariantComparisonStatus::Shared);
        assert!(!result.sample_genotypes_compared);

        let _ = fs::remove_file(left);
        let _ = fs::remove_file(right);
    }

    #[test]
    fn rejects_invalid_alleles_during_comparison() {
        let valid = temp_path("compare-valid.vcf");
        let invalid_alt_list = temp_path("compare-invalid-alt-list.vcf");
        let unsupported_allele = temp_path("compare-unsupported-allele.vcf");
        let identical_allele = temp_path("compare-identical-allele.vcf");
        let header = "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO";
        write_vcf(&valid, header, "chr1\t2\t.\tA\tG\t50\tPASS\t.\n");
        write_vcf(
            &invalid_alt_list,
            header,
            "chr1\t2\t.\tA\tA,,G\t50\tPASS\t.\n",
        );
        write_vcf(
            &unsupported_allele,
            header,
            "chr1\t2\t.\tA\tU\t50\tPASS\t.\n",
        );
        write_vcf(&identical_allele, header, "chr1\t2\t.\ta\tA\t50\tPASS\t.\n");

        assert!(matches!(
            compare_vcf_paths(&invalid_alt_list, &valid),
            Err(VariantTransformError::Vcf(VcfError::MalformedRecord { .. }))
        ));
        assert!(matches!(
            compare_vcf_paths(&unsupported_allele, &valid),
            Err(VariantTransformError::InvalidRecord { .. })
        ));
        assert!(matches!(
            compare_vcf_paths(&identical_allele, &valid),
            Err(VariantTransformError::InvalidRecord { .. })
        ));

        let _ = fs::remove_file(valid);
        let _ = fs::remove_file(invalid_alt_list);
        let _ = fs::remove_file(unsupported_allele);
        let _ = fs::remove_file(identical_allele);
    }

    #[test]
    fn rejects_malformed_headers_and_positions_during_comparison() {
        let valid = temp_path("compare-valid-structure.vcf");
        let malformed_header = temp_path("compare-malformed-header.vcf");
        let malformed_position = temp_path("compare-malformed-position.vcf");
        let header = "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO";
        write_vcf(&valid, header, "chr1\t2\t.\tA\tG\t50\tPASS\t.\n");
        write_vcf(
            &malformed_header,
            "#CHROM\tSTART\tID\tREF\tALT\tQUAL\tFILTER\tINFO",
            "chr1\t2\t.\tA\tG\t50\tPASS\t.\n",
        );
        write_vcf(
            &malformed_position,
            header,
            "chr1\tzero\t.\tA\tG\t50\tPASS\t.\n",
        );

        assert!(matches!(
            compare_vcf_paths(&malformed_header, &valid),
            Err(VariantTransformError::Vcf(VcfError::InvalidHeader { .. }))
        ));
        assert!(matches!(
            compare_vcf_paths(&malformed_position, &valid),
            Err(VariantTransformError::Vcf(VcfError::MalformedRecord { .. }))
        ));

        let _ = fs::remove_file(valid);
        let _ = fs::remove_file(malformed_header);
        let _ = fs::remove_file(malformed_position);
    }

    #[test]
    fn filters_vcf_by_quality_pass_contig_and_depth() {
        let input = temp_path("filter.vcf");
        let output = temp_path("filtered.vcf");
        fs::write(
            &input,
            "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t2\t.\tA\tG\t50\tPASS\tDP=20\nchr2\t2\t.\tA\tT\t10\tq10\tDP=2\n",
        )
        .unwrap();
        let summary = filter_vcf_path(
            &input,
            &output,
            &VariantFilterOptions {
                min_qual: Some(20.0),
                require_pass: true,
                contigs: vec!["chr1".to_owned()],
                min_info_dp: Some(10),
            },
        )
        .unwrap();
        assert_eq!(summary.output_records, 1);
        assert_eq!(summary.rejected_by_qual, 1);
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn validates_reference_and_left_aligns_homopolymer_indel() {
        let fasta = temp_path("reference.fa");
        let input = temp_path("normalize.vcf");
        let output = temp_path("normalized.vcf");
        fs::write(&fasta, ">chr1\nAAAAAC\n").unwrap();
        fs::write(
            &input,
            "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t4\t.\tA\tAA\t50\tPASS\t.\n",
        )
        .unwrap();
        let summary = normalize_vcf_path(&input, &fasta, &output).unwrap();
        assert_eq!(summary.changed_records, 1);
        assert_eq!(summary.left_aligned_records, 1);
        assert!(
            fs::read_to_string(&output)
                .unwrap()
                .contains("chr1\t1\t.\tA\tAA")
        );
        let _ = fs::remove_file(fasta);
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn rejects_multiallelic_normalization_without_creating_output() {
        let fasta = temp_path("reference-multi.fa");
        let input = temp_path("normalize-multi.vcf");
        let output = temp_path("normalized-multi.vcf");
        fs::write(&fasta, ">chr1\nAAAAAC\n").unwrap();
        fs::write(
            &input,
            "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t2\t.\tA\tC,G\t50\tPASS\t.\n",
        )
        .unwrap();
        assert!(normalize_vcf_path(&input, &fasta, &output).is_err());
        assert!(!output.exists());
        let _ = fs::remove_file(fasta);
        let _ = fs::remove_file(input);
    }

    #[test]
    fn converts_vcf_to_tsv_table() {
        let input = temp_path("to-table.vcf");
        let output = temp_path("to-table.tsv");
        fs::write(
            &input,
            concat!(
                "##fileformat=VCFv4.3\n",
                "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tsample1\tsample2\n",
                "chr1\t100\trs1\tA\tG\t50\tPASS\tDP=20\tGT:DP\t0/1:10\t1/1:15\n",
                "chr2\t200\t.\tC\tT,<DEL>\t.\t.\t.\tGT\t0/0\t./.\n",
            ),
        )
        .unwrap();

        let summary = vcf_to_table_path(&input, &output).unwrap();
        assert_eq!(summary.input_record_count, 2);
        assert_eq!(summary.output_record_count, 2);
        assert_eq!(summary.sample_count, 2);
        assert!(summary.warnings.is_empty());

        let content = fs::read_to_string(&output).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3);
        // Header
        assert_eq!(
            lines[0],
            "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tsample1\tsample2"
        );
        // First record
        assert!(lines[1].starts_with("chr1\t100\trs1\tA\tG\t50\tPASS\tDP=20\t"));
        assert!(lines[1].contains("0/1:10"));
        assert!(lines[1].contains("1/1:15"));
        // Second record
        assert!(lines[2].starts_with("chr2\t200\t.\tC\tT,<DEL>\t.\t.\t.\t"));
        assert!(lines[2].contains("0/0"));
        assert!(lines[2].contains("./."));

        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn converts_vcf_without_samples_to_tsv() {
        let input = temp_path("to-table-nosamples.vcf");
        let output = temp_path("to-table-nosamples.tsv");
        fs::write(
            &input,
            concat!(
                "##fileformat=VCFv4.3\n",
                "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n",
                "chr1\t100\t.\tA\tG\t50\tPASS\t.\n",
            ),
        )
        .unwrap();

        let summary = vcf_to_table_path(&input, &output).unwrap();
        assert_eq!(summary.input_record_count, 1);
        assert_eq!(summary.sample_count, 0);

        let content = fs::read_to_string(&output).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO");

        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn refuses_to_overwrite_existing_output() {
        let input = temp_path("to-table-exists.vcf");
        let output = temp_path("to-table-exists.tsv");
        fs::write(
            &input,
            concat!(
                "##fileformat=VCFv4.3\n",
                "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n",
                "chr1\t100\t.\tA\tG\t50\tPASS\t.\n",
            ),
        )
        .unwrap();
        fs::write(&output, "existing\n").unwrap();

        let result = vcf_to_table_path(&input, &output);
        assert!(result.is_err());

        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn reads_gzip_vcf_for_to_table() {
        let output = temp_path("to-table-gzip.tsv");
        let vcf_content = concat!(
            "##fileformat=VCFv4.3\n",
            "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n",
            "chr1\t1\t.\tA\tG\t.\tPASS\t.\n",
        );

        use flate2::{Compression, write::GzEncoder};
        let input = temp_path("to-table-gzip.vcf.gz");
        let mut encoder = GzEncoder::new(fs::File::create(&input).unwrap(), Compression::default());
        encoder.write_all(vcf_content.as_bytes()).unwrap();
        encoder.finish().unwrap();

        let summary = vcf_to_table_path(&input, &output).unwrap();
        assert_eq!(summary.input_record_count, 1);

        let content = fs::read_to_string(&output).unwrap();
        assert!(content.contains("chr1\t1\t.\tA\tG\t.\tPASS\t."));

        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }
}
