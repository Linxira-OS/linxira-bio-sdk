use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

const NO_HEADER_WARNING: &str = "SAM has no header; reference metadata is unavailable";
const NO_RECORDS_WARNING: &str = "SAM contains no alignment records";

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct SamQcMetrics {
    pub header_line_count: u64,
    pub record_count: u64,
    pub primary_record_count: u64,
    pub secondary_record_count: u64,
    pub supplementary_record_count: u64,
    pub mapped_record_count: u64,
    pub unmapped_record_count: u64,
    pub mapped_percent: Option<f64>,
    pub paired_record_count: u64,
    pub proper_pair_record_count: u64,
    pub read1_record_count: u64,
    pub read2_record_count: u64,
    pub duplicate_record_count: u64,
    pub qc_fail_record_count: u64,
    pub zero_mapq_record_count: u64,
    pub mean_mapq: Option<f64>,
    pub reference_counts: BTreeMap<String, u64>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum SamError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MalformedRecord { line: usize, message: String },
}

impl Display for SamError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read SAM: {error}"),
            Self::ReadLine { line, source } => {
                write!(formatter, "failed to read SAM at line {line}: {source}")
            }
            Self::MalformedRecord { line, message } => {
                write!(formatter, "malformed SAM record at line {line}: {message}")
            }
        }
    }
}

impl Error for SamError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            Self::MalformedRecord { .. } => None,
        }
    }
}

impl From<io::Error> for SamError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn sam_qc_path(path: impl AsRef<Path>) -> Result<SamQcMetrics, SamError> {
    let path = path.as_ref();
    let mut magic = [0_u8; 2];
    let magic_length = File::open(path)?.read(&mut magic)?;
    let input: Box<dyn Read> = if magic_length == magic.len() && magic == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    sam_qc(BufReader::new(input))
}

fn sam_qc(mut reader: impl BufRead) -> Result<SamQcMetrics, SamError> {
    let mut metrics = SamQcMetrics::default();
    let mut line_number = 0_usize;
    let mut buffer = String::new();
    let mut saw_record = false;
    let mut mapq_total = 0_u64;
    let mut known_mapq_record_count = 0_u64;

    loop {
        line_number += 1;
        buffer.clear();
        let bytes_read = reader
            .read_line(&mut buffer)
            .map_err(|source| SamError::ReadLine {
                line: line_number,
                source,
            })?;
        if bytes_read == 0 {
            break;
        }

        let line = buffer.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return malformed(line_number, "blank lines are not valid SAM records");
        }
        if line.starts_with('@') {
            if saw_record {
                return malformed(line_number, "header line appears after alignment records");
            }
            metrics.header_line_count += 1;
            continue;
        }
        saw_record = true;
        parse_record(
            line,
            line_number,
            &mut metrics,
            &mut mapq_total,
            &mut known_mapq_record_count,
        )?;
    }

    metrics.mapped_percent = percent(metrics.mapped_record_count, metrics.record_count);
    metrics.mean_mapq = ratio(mapq_total, known_mapq_record_count);
    if metrics.header_line_count == 0 {
        metrics.warnings.push(NO_HEADER_WARNING.to_owned());
    }
    if metrics.record_count == 0 {
        metrics.warnings.push(NO_RECORDS_WARNING.to_owned());
    }
    Ok(metrics)
}

fn parse_record(
    line: &str,
    line_number: usize,
    metrics: &mut SamQcMetrics,
    mapq_total: &mut u64,
    known_mapq_record_count: &mut u64,
) -> Result<(), SamError> {
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() < 11 {
        return malformed(
            line_number,
            format!(
                "expected at least 11 tab-separated fields, found {}",
                fields.len()
            ),
        );
    }
    if fields[0].is_empty() {
        return malformed(line_number, "QNAME must not be empty");
    }
    let flag = fields[1]
        .parse::<u16>()
        .map_err(|_| SamError::MalformedRecord {
            line: line_number,
            message: format!("invalid FLAG value {:?}", fields[1]),
        })?;
    let position = fields[3]
        .parse::<u64>()
        .map_err(|_| SamError::MalformedRecord {
            line: line_number,
            message: format!("invalid POS value {:?}", fields[3]),
        })?;
    let mapq = fields[4]
        .parse::<u8>()
        .map_err(|_| SamError::MalformedRecord {
            line: line_number,
            message: format!("invalid MAPQ value {:?}", fields[4]),
        })?;
    fields[7]
        .parse::<u64>()
        .map_err(|_| SamError::MalformedRecord {
            line: line_number,
            message: format!("invalid PNEXT value {:?}", fields[7]),
        })?;
    fields[8]
        .parse::<i64>()
        .map_err(|_| SamError::MalformedRecord {
            line: line_number,
            message: format!("invalid TLEN value {:?}", fields[8]),
        })?;
    if fields[9] == "*" && fields[10] != "*" {
        return malformed(line_number, "QUAL must be '*' when SEQ is '*'");
    }
    if fields[9] != "*" && fields[10] != "*" && fields[9].len() != fields[10].len() {
        return malformed(line_number, "SEQ and QUAL lengths differ");
    }

    const PAIRED: u16 = 0x1;
    const PROPER_PAIR: u16 = 0x2;
    const UNMAPPED: u16 = 0x4;
    const READ1: u16 = 0x40;
    const READ2: u16 = 0x80;
    const SECONDARY: u16 = 0x100;
    const QC_FAIL: u16 = 0x200;
    const DUPLICATE: u16 = 0x400;
    const SUPPLEMENTARY: u16 = 0x800;

    let unmapped = flag & UNMAPPED != 0;
    if !unmapped && (fields[2] == "*" || position == 0 || fields[5] == "*") {
        return malformed(
            line_number,
            "mapped records require RNAME, positive POS, and CIGAR",
        );
    }

    metrics.record_count += 1;
    if flag & SECONDARY != 0 {
        metrics.secondary_record_count += 1;
    }
    if flag & SUPPLEMENTARY != 0 {
        metrics.supplementary_record_count += 1;
    }
    if flag & (SECONDARY | SUPPLEMENTARY) == 0 {
        metrics.primary_record_count += 1;
    }
    if unmapped {
        metrics.unmapped_record_count += 1;
    } else {
        metrics.mapped_record_count += 1;
        if mapq == 0 {
            metrics.zero_mapq_record_count += 1;
        }
        // SAM reserves 255 for "mapping quality unavailable". Keep the
        // alignment in mapped counts, but do not treat that sentinel as a
        // numeric observation when computing the mean.
        if mapq != 255 {
            *mapq_total = mapq_total.checked_add(u64::from(mapq)).ok_or_else(|| {
                SamError::MalformedRecord {
                    line: line_number,
                    message: "mapping-quality total exceeds supported range".to_owned(),
                }
            })?;
            *known_mapq_record_count += 1;
        }
        *metrics
            .reference_counts
            .entry(fields[2].to_owned())
            .or_default() += 1;
    }
    for (mask, counter) in [
        (PAIRED, &mut metrics.paired_record_count),
        (PROPER_PAIR, &mut metrics.proper_pair_record_count),
        (READ1, &mut metrics.read1_record_count),
        (READ2, &mut metrics.read2_record_count),
        (DUPLICATE, &mut metrics.duplicate_record_count),
        (QC_FAIL, &mut metrics.qc_fail_record_count),
    ] {
        if flag & mask != 0 {
            *counter += 1;
        }
    }
    Ok(())
}

fn ratio(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator != 0).then_some(numerator as f64 / denominator as f64)
}

fn percent(numerator: u64, denominator: u64) -> Option<f64> {
    ratio(numerator, denominator).map(|value| value * 100.0)
}

fn malformed<T>(line: usize, message: impl Into<String>) -> Result<T, SamError> {
    Err(SamError::MalformedRecord {
        line,
        message: message.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::{SamError, sam_qc};
    use std::io::Cursor;

    #[test]
    fn summarizes_sam_flags_and_mapping_quality() {
        let input = b"@HD\tVN:1.6\tSO:coordinate\n@SQ\tSN:chr1\tLN:1000\nr1\t99\tchr1\t10\t60\t4M\t=\t30\t24\tACGT\tIIII\nr1\t147\tchr1\t30\t30\t4M\t=\t10\t-24\tTGCA\tIIII\nr2\t4\t*\t0\t0\t*\t*\t0\t0\tNN\t!!\nr3\t1280\tchr1\t50\t0\t4M\t*\t0\t0\tAAAA\tIIII\n";
        let metrics = sam_qc(Cursor::new(input)).expect("valid SAM");

        assert_eq!(metrics.record_count, 4);
        assert_eq!(metrics.mapped_record_count, 3);
        assert_eq!(metrics.unmapped_record_count, 1);
        assert_eq!(metrics.primary_record_count, 3);
        assert_eq!(metrics.secondary_record_count, 1);
        assert_eq!(metrics.supplementary_record_count, 0);
        assert_eq!(metrics.paired_record_count, 2);
        assert_eq!(metrics.proper_pair_record_count, 2);
        assert_eq!(metrics.zero_mapq_record_count, 1);
        assert_eq!(metrics.mean_mapq, Some(30.0));
        assert_eq!(metrics.reference_counts["chr1"], 3);
        assert!(metrics.warnings.is_empty());
    }

    #[test]
    fn rejects_mapped_record_without_reference_coordinates() {
        let error = sam_qc(Cursor::new(b"r1\t0\t*\t0\t10\t*\t*\t0\t0\tA\tI\n"))
            .expect_err("invalid mapped record");
        assert!(matches!(error, SamError::MalformedRecord { line: 1, .. }));
    }

    #[test]
    fn rejects_quality_without_sequence() {
        let error = sam_qc(Cursor::new(b"r1\t4\t*\t0\t0\t*\t*\t0\t0\t*\tI\n"))
            .expect_err("quality without sequence");
        assert!(matches!(error, SamError::MalformedRecord { line: 1, .. }));
        assert!(error.to_string().contains("QUAL must be '*'"));
    }

    #[test]
    fn excludes_unknown_mapq_from_mean_but_counts_zero() {
        let input = b"unknown\t0\tchr1\t1\t255\t1M\t*\t0\t0\tA\tI\nzero\t0\tchr1\t2\t0\t1M\t*\t0\t0\tA\tI\nknown\t0\tchr1\t3\t60\t1M\t*\t0\t0\tA\tI\n";
        let metrics = sam_qc(Cursor::new(input)).expect("valid SAM");

        assert_eq!(metrics.mapped_record_count, 3);
        assert_eq!(metrics.zero_mapq_record_count, 1);
        assert_eq!(metrics.mean_mapq, Some(30.0));
    }

    #[test]
    fn warns_for_headerless_empty_sam() {
        let metrics = sam_qc(Cursor::new([])).expect("empty SAM summary");
        assert_eq!(metrics.warnings.len(), 2);
    }
}
