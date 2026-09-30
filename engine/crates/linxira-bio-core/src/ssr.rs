//! Simple sequence repeat (SSR / microsatellite) detection with MISA-aligned
//! semantics: shortest-period-first scanning, consumed intervals (records
//! never overlap), and compound grouping by interruption distance.

use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

const MAX_MOTIF_LENGTH: usize = 6;
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsrParameters {
    /// Minimum repeat count indexed by motif length (index 1..=6; index 0 is
    /// unused). Defaults follow MISA: mono 10, di 6, tri-penta-hexa 5.
    pub min_repeats: [u32; 7],
    /// Maximum interruption (bp) between two SSRs in the same sequence for
    /// both to be counted as one compound SSR group. MISA default is 100.
    pub compound_max_distance: u64,
}

impl Default for SsrParameters {
    fn default() -> Self {
        Self {
            min_repeats: [0, 10, 6, 5, 5, 5, 5],
            compound_max_distance: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrRecord {
    pub sequence_id: String,
    /// 1-based inclusive start on the forward strand.
    pub start: u64,
    /// 1-based inclusive end.
    pub end: u64,
    pub motif_length: u8,
    pub motif: String,
    pub repeats: u32,
    pub size: u64,
    /// True when this record belongs to a compound SSR group.
    pub compound: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SsrSequenceSummary {
    pub sequence_id: String,
    pub length: u64,
    pub ssr_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SsrSummary {
    pub sequence_count: u64,
    pub total_bases: u64,
    pub ssr_count: u64,
    pub compound_group_count: u64,
    pub motif_length_counts: BTreeMap<u8, u64>,
    pub sequences: Vec<SsrSequenceSummary>,
}

#[derive(Debug)]
pub enum SsrError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MalformedRecord { line: usize, message: String },
    NoRecords,
    InvalidParameter(String),
}

impl Display for SsrError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read FASTA: {error}"),
            Self::ReadLine { line, source } => {
                write!(formatter, "failed to read FASTA at line {line}: {source}")
            }
            Self::MalformedRecord { line, message } => {
                write!(formatter, "malformed FASTA at line {line}: {message}")
            }
            Self::NoRecords => formatter.write_str("FASTA contains no sequence records"),
            Self::InvalidParameter(message) => formatter.write_str(message),
        }
    }
}

impl Error for SsrError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) | Self::ReadLine { source: error, .. } => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SsrError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Parse a `1:10,2:6,3:5` style override; unspecified lengths keep the
/// default from `SsrParameters::default()`.
pub fn parse_min_repeats(spec: &str) -> Result<SsrParameters, SsrError> {
    let mut parameters = SsrParameters::default();
    for entry in spec.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (length, count) = entry.split_once(':').ok_or_else(|| {
            SsrError::InvalidParameter(format!(
                "min-repeats entries must be <motif-length>:<count>, got {entry:?}"
            ))
        })?;
        let length = length
            .trim()
            .parse::<usize>()
            .map_err(|_| SsrError::InvalidParameter(format!("invalid motif length {length:?}")))?;
        if !(1..=MAX_MOTIF_LENGTH).contains(&length) {
            return Err(SsrError::InvalidParameter(format!(
                "motif length must be between 1 and {MAX_MOTIF_LENGTH}"
            )));
        }
        let count = count
            .trim()
            .parse::<u32>()
            .map_err(|_| SsrError::InvalidParameter(format!("invalid repeat count {count:?}")))?;
        if count < 2 {
            return Err(SsrError::InvalidParameter(
                "repeat counts must be at least 2".to_owned(),
            ));
        }
        parameters.min_repeats[length] = count;
    }
    Ok(parameters)
}

/// Detect perfect SSRs in one uppercase DNA sequence.
///
/// At each position the shortest motif length whose tandem repeat meets the
/// configured minimum wins (MISA semantics) and the whole repeat interval is
/// consumed, so records never overlap. Motif windows containing bases outside
/// ACGT (for example N) cannot extend.
pub fn find_ssrs(sequence: &[u8], parameters: &SsrParameters) -> Vec<SsrRecord> {
    let mut records = Vec::new();
    let sequence = uppercase_acgt(sequence);
    let length = sequence.len();
    let mut position = 0_usize;
    while position < length {
        let mut matched: Option<(usize, u32)> = None;
        for motif_length in 1..=MAX_MOTIF_LENGTH {
            if position + motif_length * 2 > length {
                break;
            }
            let unit = &sequence[position..position + motif_length];
            if !unit
                .iter()
                .all(|base| matches!(base, b'A' | b'C' | b'G' | b'T'))
            {
                continue;
            }
            let mut repeats = 1_u32;
            while position + (repeats as usize + 1) * motif_length <= length
                && &sequence[position + repeats as usize * motif_length
                    ..position + (repeats as usize + 1) * motif_length]
                    == unit
            {
                repeats += 1;
            }
            if repeats >= parameters.min_repeats[motif_length] {
                matched = Some((motif_length, repeats));
                break;
            }
        }
        match matched {
            Some((motif_length, repeats)) => {
                let size = motif_length as u64 * u64::from(repeats);
                records.push(SsrRecord {
                    sequence_id: String::new(),
                    start: (position + 1) as u64,
                    end: (position + 1) as u64 + size - 1,
                    motif_length: motif_length as u8,
                    motif: String::from_utf8_lossy(&sequence[position..position + motif_length])
                        .into_owned(),
                    repeats,
                    size,
                    compound: false,
                });
                position += size as usize;
            }
            None => position += 1,
        }
    }
    records
}

/// Mark compound SSR groups: consecutive records whose interruption
/// (bp between end and next start) is at most `compound_max_distance`.
/// Returns the number of compound groups found.
fn mark_compound_groups(records: &mut [SsrRecord], compound_max_distance: u64) -> u64 {
    let mut groups = 0_u64;
    let mut run: Option<(usize, usize)> = None;
    for index in 0..records.len() {
        let joins_previous = index > 0 && {
            let gap = records[index].start - records[index - 1].end - 1;
            gap <= compound_max_distance
        };
        if joins_previous {
            let (start, _) = run.map_or((index - 1, index - 1), |bounds| bounds);
            run = Some((start, index));
        } else {
            close_compound_run(run.take(), records, &mut groups);
            run = Some((index, index));
        }
    }
    close_compound_run(run.take(), records, &mut groups);
    groups
}

/// Mark one finished run as compound when it covers more than one record.
fn close_compound_run(run: Option<(usize, usize)>, records: &mut [SsrRecord], groups: &mut u64) {
    if let Some((start, end)) = run
        && end > start
    {
        for record in &mut records[start..=end] {
            record.compound = true;
        }
        *groups += 1;
    }
}

fn uppercase_acgt(sequence: &[u8]) -> Vec<u8> {
    sequence.to_ascii_uppercase()
}

/// Scan a FASTA file (optionally gzipped) and return every SSR record plus a
/// per-file summary. Sequence identifiers are the first whitespace-delimited
/// token of each `>` header.
pub fn ssr_scan_fasta_path(
    path: impl AsRef<Path>,
    parameters: &SsrParameters,
) -> Result<(Vec<SsrRecord>, SsrSummary), SsrError> {
    let path = path.as_ref();
    let mut prefix = [0_u8; 2];
    let prefix_length = File::open(path)?.read(&mut prefix)?;
    let input: Box<dyn Read> = if prefix_length == 2 && prefix == GZIP_MAGIC {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    ssr_scan_fasta(BufReader::new(input), parameters)
}

fn ssr_scan_fasta(
    mut reader: impl BufRead,
    parameters: &SsrParameters,
) -> Result<(Vec<SsrRecord>, SsrSummary), SsrError> {
    let mut records: Vec<SsrRecord> = Vec::new();
    let mut compound_group_total = 0_u64;
    let mut sequences = Vec::new();
    let mut motif_length_counts: BTreeMap<u8, u64> = BTreeMap::new();
    let mut total_bases = 0_u64;
    let mut line_number = 0_usize;
    let mut buffer = String::new();
    let mut current_id: Option<String> = None;
    let mut current_sequence: Vec<u8> = Vec::new();
    let mut saw_record = false;

    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read = reader
            .read_line(&mut buffer)
            .map_err(|source| SsrError::ReadLine {
                line: line_number,
                source,
            })?;
        if bytes_read == 0 {
            break;
        }
        let line = buffer.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('>') {
            if let Some(id) = current_id.take() {
                compound_group_total += flush_sequence(
                    &id,
                    &mut current_sequence,
                    parameters,
                    &mut records,
                    &mut sequences,
                    &mut motif_length_counts,
                    &mut total_bases,
                );
            }
            let id = header
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            if id.is_empty() {
                return Err(SsrError::MalformedRecord {
                    line: line_number,
                    message: "FASTA header has no identifier".to_owned(),
                });
            }
            current_id = Some(id);
            current_sequence.clear();
            saw_record = true;
        } else {
            if !saw_record {
                return Err(SsrError::MalformedRecord {
                    line: line_number,
                    message: "sequence data appears before any FASTA header".to_owned(),
                });
            }
            current_sequence.extend_from_slice(line.as_bytes());
        }
    }
    if let Some(id) = current_id.take() {
        compound_group_total += flush_sequence(
            &id,
            &mut current_sequence,
            parameters,
            &mut records,
            &mut sequences,
            &mut motif_length_counts,
            &mut total_bases,
        );
    }
    if !saw_record {
        return Err(SsrError::NoRecords);
    }

    let ssr_count = records.len() as u64;
    let summary = SsrSummary {
        sequence_count: sequences.len() as u64,
        total_bases,
        ssr_count,
        compound_group_count: compound_group_total,
        motif_length_counts,
        sequences,
    };
    Ok((records, summary))
}

#[allow(clippy::too_many_arguments)]
fn flush_sequence(
    id: &str,
    sequence: &mut [u8],
    parameters: &SsrParameters,
    records: &mut Vec<SsrRecord>,
    sequences: &mut Vec<SsrSequenceSummary>,
    motif_length_counts: &mut BTreeMap<u8, u64>,
    total_bases: &mut u64,
) -> u64 {
    let sequence_length = sequence.len() as u64;
    *total_bases += sequence_length;
    let mut found = find_ssrs(sequence, parameters);
    for record in &mut found {
        record.sequence_id = id.to_owned();
        *motif_length_counts.entry(record.motif_length).or_insert(0) += 1;
    }
    let compound_group_count = mark_compound_groups(&mut found, parameters.compound_max_distance);
    sequences.push(SsrSequenceSummary {
        sequence_id: id.to_owned(),
        length: sequence_length,
        ssr_count: found.len() as u64,
    });
    records.append(&mut found);
    compound_group_count
}

/// Write the SSR table as TSV with a header line.
pub fn write_ssr_tsv(
    mut writer: impl std::io::Write,
    records: &[SsrRecord],
) -> std::io::Result<()> {
    writeln!(
        writer,
        "sequence_id\tmotif_length\tmotif\trepeats\tsize\tstart\tend\tcompound"
    )?;
    for record in records {
        writeln!(
            writer,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            record.sequence_id,
            record.motif_length,
            record.motif,
            record.repeats,
            record.size,
            record.start,
            record.end,
            u8::from(record.compound)
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SsrParameters, find_ssrs, mark_compound_groups, parse_min_repeats};

    #[test]
    fn finds_mono_and_di_repeats_with_misa_thresholds() {
        let records = find_ssrs(b"AAAAAAAAAATATATATATATAT", &SsrParameters::default());
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].motif, "A");
        assert_eq!(records[0].repeats, 10);
        assert_eq!(records[0].start, 1);
        assert_eq!(records[0].end, 10);
        // The di-nucleotide scan starts on the T, so the reported unit is
        // the repeat as it occurs, "TA", not its rotation "AT".
        assert_eq!(records[1].motif, "TA");
        assert_eq!(records[1].repeats, 6);
        assert_eq!(records[1].start, 11);
        assert_eq!(records[1].end, 22);
    }

    #[test]
    fn shortest_motif_wins_and_interval_is_consumed() {
        // A 12 bp poly-A run: mono reaches the threshold first; the di/tri
        // interpretations of the same stretch are never emitted.
        let records = find_ssrs(b"AAAAAAAAAAAA", &SsrParameters::default());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].motif_length, 1);
        assert_eq!(records[0].size, 12);
    }

    #[test]
    fn below_threshold_repeats_are_not_reported() {
        let records = find_ssrs(b"ATATATA", &SsrParameters::default());
        assert!(records.is_empty());
    }

    #[test]
    fn n_breaks_repeats_and_input_is_case_insensitive() {
        let records = find_ssrs(b"aaaaaaaaaNATATATATATATATAT", &SsrParameters::default());
        // The N interrupts the poly-A run before it reaches 10; the AT run
        // that follows still qualifies.
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].motif, "AT");
    }

    #[test]
    fn longer_motifs_are_detected() {
        let records = find_ssrs(b"ATCATCATCATCATC", &SsrParameters::default());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].motif, "ATC");
        assert_eq!(records[0].repeats, 5);
        assert_eq!(records[0].size, 15);
    }

    #[test]
    fn compound_groups_mark_close_records() {
        let mut records = super::find_ssrs(
            b"AAAAAAAAAAGGGGGGGGGGATATATATATATAT",
            &SsrParameters {
                compound_max_distance: 2,
                ..SsrParameters::default()
            },
        );
        let groups = mark_compound_groups(&mut records, 2);
        // Three qualifying SSRs (A10, G10, AT8) with no interruption form
        // one compound group covering all three records.
        assert_eq!(records.len(), 3);
        assert_eq!(groups, 1);
        assert!(records.iter().all(|record| record.compound));
    }

    #[test]
    fn distant_records_stay_uncompounded() {
        let mut records = super::find_ssrs(
            b"AAAAAAAAAAACGTACGTATATATATATATAT",
            &SsrParameters {
                compound_max_distance: 2,
                ..SsrParameters::default()
            },
        );
        let groups = mark_compound_groups(&mut records, 2);
        assert_eq!(records.len(), 2);
        assert_eq!(groups, 0);
        assert!(records.iter().all(|record| !record.compound));
    }

    #[test]
    fn parses_partial_min_repeats_override() {
        let parameters = parse_min_repeats("2:8").expect("valid override");
        assert_eq!(parameters.min_repeats[1], 10);
        assert_eq!(parameters.min_repeats[2], 8);
        assert_eq!(parameters.min_repeats[6], 5);
        assert!(parse_min_repeats("7:3").is_err());
        assert!(parse_min_repeats("1:1").is_err());
        assert!(parse_min_repeats("3").is_err());
    }
}
