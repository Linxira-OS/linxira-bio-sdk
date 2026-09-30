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

/// One fixed genomic window of per-base depth, 1-based inclusive coordinates.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WindowDepthRow {
    pub reference: String,
    pub start: u64,
    pub end: u64,
    pub mean_depth: f64,
    pub min_depth: u64,
    pub max_depth: u64,
    pub covered_bases: u64,
    pub breadth_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WindowDepthReferenceSummary {
    pub reference: String,
    pub length: u64,
    pub window_count: u64,
    pub covered_bases: u64,
    pub breadth_percent: f64,
    pub mean_depth: f64,
    pub max_depth: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WindowDepthSummary {
    pub window_size: u64,
    pub reference_count: u64,
    pub window_count: u64,
    pub total_bases: u64,
    pub covered_bases: u64,
    pub breadth_percent: Option<f64>,
    pub mean_depth: Option<f64>,
    pub max_depth: u64,
    pub references: Vec<WindowDepthReferenceSummary>,
}

/// Aggregate a `samtools depth -aa` stream (reference, position, depth per
/// line) into fixed windows. Depth sums are exact `u64` integers; means and
/// breadth percentages divide once per window, so no accumulation error can
/// grow with genome size.
pub fn aggregate_window_depth(
    mut reader: impl BufRead,
    window_size: u64,
) -> Result<(Vec<WindowDepthRow>, WindowDepthSummary), SamError> {
    if window_size == 0 {
        return Err(SamError::MalformedRecord {
            line: 0,
            message: "window size must be positive".to_owned(),
        });
    }
    let mut rows = Vec::new();
    let mut references: Vec<WindowDepthReferenceSummary> = Vec::new();
    let mut current_reference: Option<ReferenceAccumulator> = None;
    let mut current_window: Option<WindowAccumulator> = None;
    let mut total_bases = 0_u64;
    let mut total_covered = 0_u64;
    let mut total_depth = 0_u64;
    let mut overall_max = 0_u64;
    let mut line_number = 0_usize;
    let mut buffer = String::new();

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
            return malformed(line_number, "blank lines are not valid depth records");
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 3 {
            return malformed(
                line_number,
                format!(
                    "expected 3 tab-separated depth fields, found {}",
                    fields.len()
                ),
            );
        }
        if fields[0].is_empty() {
            return malformed(line_number, "reference name must not be empty");
        }
        let position = fields[1]
            .parse::<u64>()
            .map_err(|_| SamError::MalformedRecord {
                line: line_number,
                message: format!("invalid position value {:?}", fields[1]),
            })?;
        if position == 0 {
            return malformed(line_number, "positions are 1-based and must be positive");
        }
        let depth = fields[2]
            .parse::<u64>()
            .map_err(|_| SamError::MalformedRecord {
                line: line_number,
                message: format!("invalid depth value {:?}", fields[2]),
            })?;

        if current_reference
            .as_ref()
            .is_none_or(|reference| reference.name != fields[0])
        {
            if references
                .iter()
                .any(|summary| summary.reference == fields[0])
            {
                return malformed(
                    line_number,
                    format!(
                        "reference {} appears more than once; depth input must be grouped by reference",
                        fields[0]
                    ),
                );
            }
            if let Some(mut finished) = current_reference.take() {
                if let Some(window) = current_window.take() {
                    flush_window(&mut rows, window, &mut finished, window_size, false);
                }
                references.push(finished.into_summary());
            }
            current_reference = Some(ReferenceAccumulator::new(fields[0].to_owned()));
        }

        let index = (position - 1) / window_size;
        match current_window.as_mut() {
            None => current_window = Some(WindowAccumulator::new(index)),
            Some(window) if window.index == index => {}
            Some(window) if index > window.index => {
                let contiguous = index == window.index + 1;
                let finished = std::mem::replace(window, WindowAccumulator::new(index));
                flush_window(
                    &mut rows,
                    finished,
                    current_reference
                        .as_mut()
                        .expect("reference present after the switch check"),
                    window_size,
                    contiguous,
                );
            }
            Some(_) => {
                return malformed(
                    line_number,
                    "positions must be ascending within a reference",
                );
            }
        }
        current_window
            .as_mut()
            .expect("window is present after the index match")
            .push(position, depth, line_number)?;
        let reference = current_reference
            .as_mut()
            .expect("reference is present after the switch check");
        reference.push(position, depth, line_number)?;

        total_bases += 1;
        if depth > 0 {
            total_covered += 1;
        }
        total_depth = total_depth
            .checked_add(depth)
            .ok_or_else(|| SamError::MalformedRecord {
                line: line_number,
                message: "depth sum exceeds supported range".to_owned(),
            })?;
        overall_max = overall_max.max(depth);
    }

    if let Some(mut finished) = current_reference.take() {
        if let Some(window) = current_window.take() {
            flush_window(&mut rows, window, &mut finished, window_size, false);
        }
        references.push(finished.into_summary());
    }

    let summary = WindowDepthSummary {
        window_size,
        reference_count: references.len() as u64,
        window_count: rows.len() as u64,
        total_bases,
        covered_bases: total_covered,
        breadth_percent: percent(total_covered, total_bases),
        mean_depth: ratio(total_depth, total_bases),
        max_depth: overall_max,
        references,
    };
    Ok((rows, summary))
}

/// Write the per-window depth table as a TSV with a header line.
pub fn write_window_depth_tsv(
    mut writer: impl std::io::Write,
    rows: &[WindowDepthRow],
) -> std::io::Result<()> {
    writeln!(
        writer,
        "reference\tstart\tend\tmean_depth\tmin_depth\tmax_depth\tcovered_bases\tbreadth_percent"
    )?;
    for row in rows {
        writeln!(
            writer,
            "{}\t{}\t{}\t{:.4}\t{}\t{}\t{}\t{:.4}",
            row.reference,
            row.start,
            row.end,
            row.mean_depth,
            row.min_depth,
            row.max_depth,
            row.covered_bases,
            row.breadth_percent
        )?;
    }
    Ok(())
}

fn flush_window(
    rows: &mut Vec<WindowDepthRow>,
    window: WindowAccumulator,
    reference: &mut ReferenceAccumulator,
    window_size: u64,
    full: bool,
) {
    rows.push(window.into_row(&reference.name, window_size, full));
    reference.window_count += 1;
}

struct WindowAccumulator {
    index: u64,
    last_position: u64,
    depth_sum: u64,
    min_depth: u64,
    max_depth: u64,
    covered: u64,
    count: u64,
}

impl WindowAccumulator {
    fn new(index: u64) -> Self {
        Self {
            index,
            last_position: 0,
            depth_sum: 0,
            min_depth: u64::MAX,
            max_depth: 0,
            covered: 0,
            count: 0,
        }
    }

    fn push(&mut self, position: u64, depth: u64, line: usize) -> Result<(), SamError> {
        self.depth_sum =
            self.depth_sum
                .checked_add(depth)
                .ok_or_else(|| SamError::MalformedRecord {
                    line,
                    message: "depth sum exceeds supported range".to_owned(),
                })?;
        self.last_position = position;
        self.min_depth = self.min_depth.min(depth);
        self.max_depth = self.max_depth.max(depth);
        if depth > 0 {
            self.covered += 1;
        }
        self.count += 1;
        Ok(())
    }

    /// `full` marks a window followed by a contiguous next window; its end is
    /// then the full window span instead of the last observed position.
    fn into_row(self, reference: &str, window_size: u64, full: bool) -> WindowDepthRow {
        let start = self.index * window_size + 1;
        let end = if full {
            (self.index + 1) * window_size
        } else {
            self.last_position
        };
        WindowDepthRow {
            reference: reference.to_owned(),
            start,
            end,
            mean_depth: self.depth_sum as f64 / self.count as f64,
            min_depth: self.min_depth,
            max_depth: self.max_depth,
            covered_bases: self.covered,
            breadth_percent: self.covered as f64 * 100.0 / self.count as f64,
        }
    }
}

struct ReferenceAccumulator {
    name: String,
    length: u64,
    depth_sum: u64,
    covered: u64,
    max_depth: u64,
    window_count: u64,
}

impl ReferenceAccumulator {
    fn new(name: String) -> Self {
        Self {
            name,
            length: 0,
            depth_sum: 0,
            covered: 0,
            max_depth: 0,
            window_count: 0,
        }
    }

    fn push(&mut self, position: u64, depth: u64, line: usize) -> Result<(), SamError> {
        self.depth_sum =
            self.depth_sum
                .checked_add(depth)
                .ok_or_else(|| SamError::MalformedRecord {
                    line,
                    message: "depth sum exceeds supported range".to_owned(),
                })?;
        self.length = position;
        if depth > 0 {
            self.covered += 1;
        }
        self.max_depth = self.max_depth.max(depth);
        Ok(())
    }

    fn into_summary(self) -> WindowDepthReferenceSummary {
        WindowDepthReferenceSummary {
            reference: self.name,
            length: self.length,
            window_count: self.window_count,
            covered_bases: self.covered,
            breadth_percent: self.covered as f64 * 100.0 / self.length as f64,
            mean_depth: self.depth_sum as f64 / self.length as f64,
            max_depth: self.max_depth,
        }
    }
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
    use super::{SamError, WindowDepthRow, aggregate_window_depth, sam_qc, write_window_depth_tsv};
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

    #[test]
    fn aggregates_complete_depth_stream_into_fixed_windows() {
        let input = "chr1\t1\t0\nchr1\t2\t1\nchr1\t3\t2\nchr1\t4\t3\nchr1\t5\t4\nchr1\t6\t0\nchr1\t7\t0\nchr1\t8\t0\nchr1\t9\t5\nchr1\t10\t0\n";
        let (rows, summary) =
            aggregate_window_depth(Cursor::new(input), 4).expect("valid depth stream");

        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[0],
            WindowDepthRow {
                reference: "chr1".to_owned(),
                start: 1,
                end: 4,
                mean_depth: 1.5,
                min_depth: 0,
                max_depth: 3,
                covered_bases: 3,
                breadth_percent: 75.0,
            }
        );
        assert_eq!(rows[1].start, 5);
        assert_eq!(rows[1].end, 8);
        assert_eq!(rows[1].mean_depth, 1.0);
        assert_eq!(rows[1].breadth_percent, 25.0);
        // The final window is partial: it ends at the last position, not at
        // the full window span.
        assert_eq!(rows[2].start, 9);
        assert_eq!(rows[2].end, 10);
        assert_eq!(rows[2].mean_depth, 2.5);

        assert_eq!(summary.window_size, 4);
        assert_eq!(summary.reference_count, 1);
        assert_eq!(summary.window_count, 3);
        assert_eq!(summary.total_bases, 10);
        assert_eq!(summary.covered_bases, 5);
        assert_eq!(summary.breadth_percent, Some(50.0));
        assert_eq!(summary.mean_depth, Some(1.5));
        assert_eq!(summary.max_depth, 5);
        let reference = &summary.references[0];
        assert_eq!(reference.reference, "chr1");
        assert_eq!(reference.length, 10);
        assert_eq!(reference.window_count, 3);
        assert_eq!(reference.covered_bases, 5);
        assert_eq!(reference.mean_depth, 1.5);
        assert_eq!(reference.max_depth, 5);
    }

    #[test]
    fn aggregates_multiple_references_and_rejects_revisits() {
        let input = "chr1\t1\t1\nchr1\t2\t1\nchr2\t1\t0\nchr2\t2\t4\nchr1\t1\t1\n";
        let error = aggregate_window_depth(Cursor::new(input), 2).expect_err("chr1 revisited");
        assert!(error.to_string().contains("appears more than once"));

        let input = "chr1\t1\t1\nchr1\t2\t1\nchr2\t1\t0\nchr2\t2\t4\n";
        let (rows, summary) =
            aggregate_window_depth(Cursor::new(input), 2).expect("grouped references");
        assert_eq!(rows.len(), 2);
        assert_eq!(summary.reference_count, 2);
        assert_eq!(summary.total_bases, 4);
        assert_eq!(summary.covered_bases, 3);
        assert_eq!(summary.references[1].max_depth, 4);
    }

    #[test]
    fn rejects_descending_positions_and_bad_records() {
        let error = aggregate_window_depth(Cursor::new(b"chr1\t5\t1\nchr1\t4\t1\n"), 2)
            .expect_err("descending positions");
        assert!(error.to_string().contains("ascending"));

        let error =
            aggregate_window_depth(Cursor::new(b"chr1\t0\t1\n"), 2).expect_err("zero position");
        assert!(error.to_string().contains("1-based"));

        let error =
            aggregate_window_depth(Cursor::new(b"chr1\t1\n"), 2).expect_err("missing depth column");
        assert!(error.to_string().contains("expected 3"));

        let error =
            aggregate_window_depth(Cursor::new(b"chr1\t1\tx\n"), 2).expect_err("non-numeric depth");
        assert!(error.to_string().contains("invalid depth"));

        let error =
            aggregate_window_depth(Cursor::new(b"chr1\t1\t1\n"), 0).expect_err("zero window size");
        assert!(error.to_string().contains("window size must be positive"));
    }

    #[test]
    fn empty_depth_stream_yields_empty_summary() {
        let (rows, summary) = aggregate_window_depth(Cursor::new(b""), 100).expect("empty stream");
        assert!(rows.is_empty());
        assert_eq!(summary.reference_count, 0);
        assert_eq!(summary.total_bases, 0);
        assert_eq!(summary.breadth_percent, None);
        assert_eq!(summary.mean_depth, None);
    }

    #[test]
    fn writes_window_depth_tsv_with_header() {
        let input = "chr1\t1\t2\nchr1\t2\t0\n";
        let (rows, _summary) =
            aggregate_window_depth(Cursor::new(input), 2).expect("valid depth stream");
        let mut buffer = Vec::new();
        write_window_depth_tsv(&mut buffer, &rows).expect("TSV written");
        let text = String::from_utf8(buffer).expect("UTF-8 TSV");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            "reference\tstart\tend\tmean_depth\tmin_depth\tmax_depth\tcovered_bases\tbreadth_percent"
        );
        assert_eq!(lines[1], "chr1\t1\t2\t1.0000\t0\t2\t1\t50.0000");
    }
}
