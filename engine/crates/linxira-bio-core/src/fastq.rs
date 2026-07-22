use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

pub const DEFAULT_MAX_CYCLES: usize = 500;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityEncodingMode {
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "phred+33")]
    Phred33,
    #[serde(rename = "phred+64")]
    Phred64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastqQcOptions {
    pub max_cycles: usize,
    pub quality_encoding: QualityEncodingMode,
}

impl Default for FastqQcOptions {
    fn default() -> Self {
        Self {
            max_cycles: DEFAULT_MAX_CYCLES,
            quality_encoding: QualityEncodingMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityEncoding {
    #[serde(rename = "phred+33")]
    Phred33,
    #[serde(rename = "phred+64")]
    Phred64,
    #[serde(rename = "ambiguous")]
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerCycleQc {
    pub cycle: u64,
    pub base_count: u64,
    pub gc_percent: f64,
    pub n_percent: f64,
    pub mean_quality: f64,
    pub q20_percent: f64,
    pub q30_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FastqQcMetrics {
    pub read_count: u64,
    pub total_bases: u64,
    pub min_length: u64,
    pub max_length: u64,
    pub mean_length: f64,
    pub gc_percent: f64,
    pub n_percent: f64,
    pub mean_quality: f64,
    pub q20_percent: f64,
    pub q30_percent: f64,
    pub quality_encoding: QualityEncoding,
    pub applied_quality_offset: u8,
    pub per_cycle: Vec<PerCycleQc>,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum FastqError {
    Io(io::Error),
    NoRecords,
    MalformedRecord {
        record: u64,
        line: u64,
        message: String,
    },
    TruncatedRecord {
        record: u64,
        line: u64,
        expected: &'static str,
    },
    TruncatedQuality {
        record: u64,
        line: u64,
        sequence_length: u64,
        quality_length: u64,
    },
    SequenceQualityLengthMismatch {
        record: u64,
        line: u64,
        sequence_length: u64,
        quality_length: u64,
    },
}

impl Display for FastqError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read FASTQ: {error}"),
            Self::NoRecords => formatter.write_str("FASTQ contains no records"),
            Self::MalformedRecord {
                record,
                line,
                message,
            } => write!(
                formatter,
                "malformed FASTQ record {record} at line {line}: {message}"
            ),
            Self::TruncatedRecord {
                record,
                line,
                expected,
            } => write!(
                formatter,
                "truncated FASTQ record {record} at line {line}: expected {expected}"
            ),
            Self::TruncatedQuality {
                record,
                line,
                sequence_length,
                quality_length,
            } => write!(
                formatter,
                "truncated FASTQ record {record} at line {line}: sequence length is \
                 {sequence_length}, but only {quality_length} quality values were present"
            ),
            Self::SequenceQualityLengthMismatch {
                record,
                line,
                sequence_length,
                quality_length,
            } => write!(
                formatter,
                "malformed FASTQ record {record} at line {line}: sequence length is \
                 {sequence_length}, but quality length is {quality_length}"
            ),
        }
    }
}

impl Error for FastqError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FastqError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Default)]
struct CycleAccumulator {
    base_count: u64,
    gc_count: u64,
    n_count: u64,
    quality_ascii_sum: u128,
    q20_phred33_count: u64,
    q30_phred33_count: u64,
    q20_phred64_count: u64,
    q30_phred64_count: u64,
}

#[derive(Debug)]
struct QcAccumulator {
    read_count: u64,
    total_bases: u64,
    min_length: u64,
    max_length: u64,
    gc_count: u64,
    n_count: u64,
    quality_ascii_sum: u128,
    minimum_quality_ascii: u8,
    q20_phred33_count: u64,
    q30_phred33_count: u64,
    q20_phred64_count: u64,
    q30_phred64_count: u64,
    cycles: Vec<CycleAccumulator>,
    cycles_truncated: bool,
}

impl Default for QcAccumulator {
    fn default() -> Self {
        Self {
            read_count: 0,
            total_bases: 0,
            min_length: u64::MAX,
            max_length: 0,
            gc_count: 0,
            n_count: 0,
            quality_ascii_sum: 0,
            minimum_quality_ascii: u8::MAX,
            q20_phred33_count: 0,
            q30_phred33_count: 0,
            q20_phred64_count: 0,
            q30_phred64_count: 0,
            cycles: Vec::new(),
            cycles_truncated: false,
        }
    }
}

struct FastqLineReader<R> {
    inner: R,
    line_number: u64,
}

impl<R: BufRead> FastqLineReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            line_number: 0,
        }
    }

    fn next_line(&mut self, buffer: &mut Vec<u8>) -> Result<Option<u64>, FastqError> {
        buffer.clear();
        if self.inner.read_until(b'\n', buffer)? == 0 {
            return Ok(None);
        }

        self.line_number += 1;
        if buffer.last() == Some(&b'\n') {
            buffer.pop();
        }
        if buffer.last() == Some(&b'\r') {
            buffer.pop();
        }
        Ok(Some(self.line_number))
    }

    fn next_expected_line(&self) -> u64 {
        self.line_number + 1
    }
}

pub fn fastq_qc_path(
    path: impl AsRef<Path>,
    options: FastqQcOptions,
) -> Result<FastqQcMetrics, FastqError> {
    let file = File::open(path)?;
    let mut input = BufReader::new(file);
    let is_gzip = input.fill_buf()?.starts_with(&[0x1f, 0x8b]);

    if is_gzip {
        fastq_qc(BufReader::new(MultiGzDecoder::new(input)), options)
    } else {
        fastq_qc(input, options)
    }
}

pub fn fastq_qc(
    reader: impl BufRead,
    options: FastqQcOptions,
) -> Result<FastqQcMetrics, FastqError> {
    let mut reader = FastqLineReader::new(reader);
    let mut line = Vec::new();
    let mut accumulator = QcAccumulator::default();

    while let Some(header_line) = reader.next_line(&mut line)? {
        let record = accumulator.read_count + 1;
        let identifier = parse_header(&line, record, header_line)?.to_vec();
        let sequence_length = parse_sequence(
            &mut reader,
            &mut line,
            record,
            &identifier,
            options.max_cycles,
            &mut accumulator,
        )?;
        parse_quality(
            &mut reader,
            &mut line,
            record,
            sequence_length,
            options,
            &mut accumulator,
        )?;

        accumulator.read_count += 1;
        accumulator.total_bases = accumulator.total_bases.checked_add(sequence_length).ok_or(
            FastqError::MalformedRecord {
                record,
                line: reader.line_number,
                message: "total base count exceeds the supported range".to_owned(),
            },
        )?;
        accumulator.min_length = accumulator.min_length.min(sequence_length);
        accumulator.max_length = accumulator.max_length.max(sequence_length);
        if sequence_length > options.max_cycles as u64 {
            accumulator.cycles_truncated = true;
        }
    }

    finish_metrics(accumulator, options)
}

fn parse_header(line: &[u8], record: u64, line_number: u64) -> Result<&[u8], FastqError> {
    let Some(header) = line.strip_prefix(b"@") else {
        return Err(malformed(
            record,
            line_number,
            "expected a header beginning with '@'",
        ));
    };
    let Some(identifier) = first_ascii_field(header) else {
        return Err(malformed(record, line_number, "header has no identifier"));
    };
    Ok(identifier)
}

fn parse_sequence<R: BufRead>(
    reader: &mut FastqLineReader<R>,
    line: &mut Vec<u8>,
    record: u64,
    identifier: &[u8],
    max_cycles: usize,
    accumulator: &mut QcAccumulator,
) -> Result<u64, FastqError> {
    let mut sequence_length = 0_u64;

    loop {
        let Some(line_number) = reader.next_line(line)? else {
            return Err(FastqError::TruncatedRecord {
                record,
                line: reader.next_expected_line(),
                expected: "a '+' separator line",
            });
        };

        if let Some(separator) = line.strip_prefix(b"+") {
            if sequence_length == 0 {
                return Err(malformed(record, line_number, "sequence is empty"));
            }
            validate_separator(separator, identifier, record, line_number)?;
            return Ok(sequence_length);
        }

        if line.is_empty() {
            return Err(malformed(record, line_number, "sequence line is empty"));
        }

        for (column, &base) in line.iter().enumerate() {
            if !base.is_ascii_graphic() {
                return Err(malformed(
                    record,
                    line_number,
                    format!(
                        "invalid sequence byte 0x{base:02x} at column {}",
                        column + 1
                    ),
                ));
            }
            add_base(
                accumulator,
                sequence_length,
                base,
                max_cycles,
                record,
                line_number,
            )?;
            sequence_length = sequence_length.checked_add(1).ok_or_else(|| {
                malformed(
                    record,
                    line_number,
                    "sequence length exceeds the supported range",
                )
            })?;
        }
    }
}

fn validate_separator(
    separator: &[u8],
    identifier: &[u8],
    record: u64,
    line: u64,
) -> Result<(), FastqError> {
    if let Some(separator_identifier) = first_ascii_field(separator)
        && separator_identifier != identifier
    {
        return Err(malformed(
            record,
            line,
            "separator identifier does not match the header identifier",
        ));
    }
    Ok(())
}

fn parse_quality<R: BufRead>(
    reader: &mut FastqLineReader<R>,
    line: &mut Vec<u8>,
    record: u64,
    sequence_length: u64,
    options: FastqQcOptions,
    accumulator: &mut QcAccumulator,
) -> Result<(), FastqError> {
    let mut quality_length = 0_u64;

    while quality_length < sequence_length {
        let Some(line_number) = reader.next_line(line)? else {
            return Err(FastqError::TruncatedQuality {
                record,
                line: reader.next_expected_line(),
                sequence_length,
                quality_length,
            });
        };
        if line.is_empty() {
            return Err(malformed(record, line_number, "quality line is empty"));
        }

        let line_length = u64::try_from(line.len()).map_err(|_| {
            malformed(
                record,
                line_number,
                "quality line length exceeds the supported range",
            )
        })?;
        let resulting_length = quality_length.checked_add(line_length).ok_or_else(|| {
            malformed(
                record,
                line_number,
                "quality length exceeds the supported range",
            )
        })?;
        if resulting_length > sequence_length {
            return Err(FastqError::SequenceQualityLengthMismatch {
                record,
                line: line_number,
                sequence_length,
                quality_length: resulting_length,
            });
        }

        for (column, &quality) in line.iter().enumerate() {
            if !(33..=126).contains(&quality) {
                return Err(malformed(
                    record,
                    line_number,
                    format!(
                        "invalid quality byte 0x{quality:02x} at column {}",
                        column + 1
                    ),
                ));
            }
            if options.quality_encoding == QualityEncodingMode::Phred64 && quality < 64 {
                return Err(malformed(
                    record,
                    line_number,
                    format!(
                        "quality byte 0x{quality:02x} at column {} is below the Phred+64 offset",
                        column + 1
                    ),
                ));
            }

            let cycle = quality_length + column as u64;
            add_quality(accumulator, cycle, quality, options.max_cycles);
        }
        quality_length = resulting_length;
    }

    Ok(())
}

fn add_base(
    accumulator: &mut QcAccumulator,
    cycle: u64,
    base: u8,
    max_cycles: usize,
    record: u64,
    line: u64,
) -> Result<(), FastqError> {
    let upper = base.to_ascii_uppercase();
    if matches!(upper, b'G' | b'C') {
        accumulator.gc_count = accumulator
            .gc_count
            .checked_add(1)
            .ok_or_else(|| malformed(record, line, "GC base count exceeds the supported range"))?;
    } else if upper == b'N' {
        accumulator.n_count = accumulator
            .n_count
            .checked_add(1)
            .ok_or_else(|| malformed(record, line, "N base count exceeds the supported range"))?;
    }

    let Some(cycle) = cycle_index(cycle, max_cycles) else {
        return Ok(());
    };
    ensure_cycle(&mut accumulator.cycles, cycle);
    let cycle_accumulator = &mut accumulator.cycles[cycle];
    cycle_accumulator.base_count += 1;
    if matches!(upper, b'G' | b'C') {
        cycle_accumulator.gc_count += 1;
    } else if upper == b'N' {
        cycle_accumulator.n_count += 1;
    }
    Ok(())
}

fn add_quality(accumulator: &mut QcAccumulator, cycle: u64, quality: u8, max_cycles: usize) {
    accumulator.quality_ascii_sum += u128::from(quality);
    accumulator.minimum_quality_ascii = accumulator.minimum_quality_ascii.min(quality);
    if quality >= 53 {
        accumulator.q20_phred33_count += 1;
    }
    if quality >= 63 {
        accumulator.q30_phred33_count += 1;
    }
    if quality >= 84 {
        accumulator.q20_phred64_count += 1;
    }
    if quality >= 94 {
        accumulator.q30_phred64_count += 1;
    }

    let Some(cycle) = cycle_index(cycle, max_cycles) else {
        return;
    };
    ensure_cycle(&mut accumulator.cycles, cycle);
    let cycle_accumulator = &mut accumulator.cycles[cycle];
    cycle_accumulator.quality_ascii_sum += u128::from(quality);
    if quality >= 53 {
        cycle_accumulator.q20_phred33_count += 1;
    }
    if quality >= 63 {
        cycle_accumulator.q30_phred33_count += 1;
    }
    if quality >= 84 {
        cycle_accumulator.q20_phred64_count += 1;
    }
    if quality >= 94 {
        cycle_accumulator.q30_phred64_count += 1;
    }
}

fn finish_metrics(
    accumulator: QcAccumulator,
    options: FastqQcOptions,
) -> Result<FastqQcMetrics, FastqError> {
    if accumulator.read_count == 0 {
        return Err(FastqError::NoRecords);
    }

    let (quality_encoding, applied_quality_offset) = match options.quality_encoding {
        QualityEncodingMode::Phred33 => (QualityEncoding::Phred33, 33),
        QualityEncodingMode::Phred64 => (QualityEncoding::Phred64, 64),
        QualityEncodingMode::Auto if accumulator.minimum_quality_ascii < 59 => {
            (QualityEncoding::Phred33, 33)
        }
        QualityEncodingMode::Auto => (QualityEncoding::Ambiguous, 33),
    };

    let mut warnings = Vec::new();
    if quality_encoding == QualityEncoding::Ambiguous {
        warnings.push(
            "quality bytes are compatible with both Phred+33 and legacy Phred+64/Solexa \
             encodings; metrics use Phred+33 unless quality_encoding is explicitly overridden"
                .to_owned(),
        );
    }
    if accumulator.cycles_truncated {
        warnings.push(format!(
            "per-cycle metrics are capped at {} cycle(s)",
            options.max_cycles
        ));
    }

    let use_phred64 = applied_quality_offset == 64;
    let q20_count = if use_phred64 {
        accumulator.q20_phred64_count
    } else {
        accumulator.q20_phred33_count
    };
    let q30_count = if use_phred64 {
        accumulator.q30_phred64_count
    } else {
        accumulator.q30_phred33_count
    };
    let per_cycle = accumulator
        .cycles
        .iter()
        .enumerate()
        .map(|(index, cycle)| cycle_metrics(index, cycle, applied_quality_offset))
        .collect();

    Ok(FastqQcMetrics {
        read_count: accumulator.read_count,
        total_bases: accumulator.total_bases,
        min_length: accumulator.min_length,
        max_length: accumulator.max_length,
        mean_length: ratio(accumulator.total_bases, accumulator.read_count),
        gc_percent: percent(accumulator.gc_count, accumulator.total_bases),
        n_percent: percent(accumulator.n_count, accumulator.total_bases),
        mean_quality: mean_quality(
            accumulator.quality_ascii_sum,
            accumulator.total_bases,
            applied_quality_offset,
        ),
        q20_percent: percent(q20_count, accumulator.total_bases),
        q30_percent: percent(q30_count, accumulator.total_bases),
        quality_encoding,
        applied_quality_offset,
        per_cycle,
        warnings,
    })
}

fn cycle_metrics(index: usize, cycle: &CycleAccumulator, quality_offset: u8) -> PerCycleQc {
    let use_phred64 = quality_offset == 64;
    let q20_count = if use_phred64 {
        cycle.q20_phred64_count
    } else {
        cycle.q20_phred33_count
    };
    let q30_count = if use_phred64 {
        cycle.q30_phred64_count
    } else {
        cycle.q30_phred33_count
    };

    PerCycleQc {
        cycle: index as u64 + 1,
        base_count: cycle.base_count,
        gc_percent: percent(cycle.gc_count, cycle.base_count),
        n_percent: percent(cycle.n_count, cycle.base_count),
        mean_quality: mean_quality(cycle.quality_ascii_sum, cycle.base_count, quality_offset),
        q20_percent: percent(q20_count, cycle.base_count),
        q30_percent: percent(q30_count, cycle.base_count),
    }
}

fn cycle_index(cycle: u64, max_cycles: usize) -> Option<usize> {
    usize::try_from(cycle)
        .ok()
        .filter(|&cycle| cycle < max_cycles)
}

fn ensure_cycle(cycles: &mut Vec<CycleAccumulator>, cycle: usize) {
    if cycles.len() <= cycle {
        cycles.resize_with(cycle + 1, CycleAccumulator::default);
    }
}

fn first_ascii_field(bytes: &[u8]) -> Option<&[u8]> {
    let bytes = trim_ascii(bytes);
    let end = bytes
        .iter()
        .position(|byte| byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    (end > 0).then_some(&bytes[..end])
}

fn trim_ascii(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(u8::is_ascii_whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}

fn malformed(record: u64, line: u64, message: impl Into<String>) -> FastqError {
    FastqError::MalformedRecord {
        record,
        line,
        message: message.into(),
    }
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn percent(numerator: u64, denominator: u64) -> f64 {
    ratio(numerator, denominator) * 100.0
}

fn mean_quality(quality_ascii_sum: u128, base_count: u64, quality_offset: u8) -> f64 {
    if base_count == 0 {
        0.0
    } else {
        quality_ascii_sum as f64 / base_count as f64 - f64::from(quality_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FastqError, FastqQcOptions, QualityEncoding, QualityEncodingMode, fastq_qc, fastq_qc_path,
    };
    use flate2::Compression;
    use flate2::write::{DeflateEncoder, GzEncoder};
    use std::fs;
    use std::io::{Cursor, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn calculates_expected_metrics_from_fixture() {
        let input = fixture("valid.fastq");
        let metrics =
            fastq_qc(Cursor::new(input), FastqQcOptions::default()).expect("valid FASTQ fixture");

        assert_eq!(metrics.read_count, 2);
        assert_eq!(metrics.total_bases, 9);
        assert_eq!(metrics.min_length, 4);
        assert_eq!(metrics.max_length, 5);
        assert_close(metrics.mean_length, 4.5);
        assert_close(metrics.gc_percent, 100.0 * 6.0 / 9.0);
        assert_close(metrics.n_percent, 100.0 / 9.0);
        assert_close(metrics.mean_quality, 280.0 / 9.0);
        assert_close(metrics.q20_percent, 100.0);
        assert_close(metrics.q30_percent, 100.0 * 5.0 / 9.0);
        assert_eq!(metrics.quality_encoding, QualityEncoding::Phred33);
        assert_eq!(metrics.applied_quality_offset, 33);
        assert!(metrics.warnings.is_empty());
    }

    #[test]
    fn supports_wrapped_records_and_caps_per_cycle_metrics() {
        let input = fixture("wrapped.fastq");
        let options = FastqQcOptions {
            max_cycles: 3,
            ..FastqQcOptions::default()
        };
        let metrics = fastq_qc(Cursor::new(input), options).expect("wrapped FASTQ fixture");

        assert_eq!(metrics.read_count, 2);
        assert_eq!(metrics.total_bases, 7);
        assert_eq!(metrics.per_cycle.len(), 3);
        assert_eq!(metrics.per_cycle[0].cycle, 1);
        assert_eq!(metrics.per_cycle[0].base_count, 2);
        assert_close(metrics.per_cycle[0].mean_quality, 20.0);
        assert_close(metrics.per_cycle[0].q30_percent, 50.0);
        assert_close(metrics.per_cycle[1].gc_percent, 50.0);
        assert_eq!(metrics.warnings.len(), 1);
        assert!(metrics.warnings[0].contains("capped at 3"));
    }

    #[test]
    fn reports_ambiguous_auto_encoding_and_allows_legacy_override() {
        let input = b"@read\nAC\n+\nII\n";
        let automatic =
            fastq_qc(Cursor::new(input), FastqQcOptions::default()).expect("ambiguous FASTQ");
        assert_eq!(automatic.quality_encoding, QualityEncoding::Ambiguous);
        assert_eq!(automatic.applied_quality_offset, 33);
        assert_close(automatic.mean_quality, 40.0);
        assert_eq!(automatic.warnings.len(), 1);

        let legacy = fastq_qc(
            Cursor::new(input),
            FastqQcOptions {
                quality_encoding: QualityEncodingMode::Phred64,
                ..FastqQcOptions::default()
            },
        )
        .expect("forced Phred+64 FASTQ");
        assert_eq!(legacy.quality_encoding, QualityEncoding::Phred64);
        assert_eq!(legacy.applied_quality_offset, 64);
        assert_close(legacy.mean_quality, 9.0);
        assert_close(legacy.q20_percent, 0.0);
    }

    #[test]
    fn treats_solexa_range_as_ambiguous() {
        let input = b"@read\nA\n+\n;\n";
        let metrics = fastq_qc(Cursor::new(input), FastqQcOptions::default())
            .expect("quality in the overlapping Solexa range");

        assert_eq!(metrics.quality_encoding, QualityEncoding::Ambiguous);
        assert_eq!(metrics.applied_quality_offset, 33);
        assert_close(metrics.mean_quality, 26.0);
        assert_eq!(metrics.warnings.len(), 1);
    }

    #[test]
    fn rejects_truncated_quality_with_record_and_line_context() {
        let error = fastq_qc(
            Cursor::new(fixture("truncated.fastq")),
            FastqQcOptions::default(),
        )
        .expect_err("truncated FASTQ must fail");

        assert!(matches!(
            error,
            FastqError::TruncatedQuality {
                record: 1,
                line: 5,
                sequence_length: 4,
                quality_length: 3,
            }
        ));
    }

    #[test]
    fn rejects_quality_length_overrun_with_record_and_line_context() {
        let error = fastq_qc(
            Cursor::new(fixture("length-mismatch.fastq")),
            FastqQcOptions::default(),
        )
        .expect_err("quality overrun must fail");

        assert!(matches!(
            error,
            FastqError::SequenceQualityLengthMismatch {
                record: 1,
                line: 4,
                sequence_length: 2,
                quality_length: 3,
            }
        ));
    }

    #[test]
    fn rejects_malformed_next_header_with_record_and_line_context() {
        let error = fastq_qc(
            Cursor::new(b"@one\nA\n+\n!\nnot-a-header\n"),
            FastqQcOptions::default(),
        )
        .expect_err("malformed header must fail");

        assert!(matches!(
            error,
            FastqError::MalformedRecord {
                record: 2,
                line: 5,
                ..
            }
        ));
    }

    #[test]
    fn detects_gzip_and_bgzf_from_magic_bytes() {
        let payload = b"@read\nACGT\n+\n!!!!\n";
        let gzip_path = temporary_path("gzip.data");
        let mut gzip = GzEncoder::new(
            fs::File::create(&gzip_path).expect("create gzip fixture"),
            Compression::default(),
        );
        gzip.write_all(payload).expect("write gzip fixture");
        gzip.finish().expect("finish gzip fixture");

        let bgzf_path = temporary_path("bgzf.data");
        fs::write(&bgzf_path, bgzf_block(payload)).expect("write BGZF fixture");

        let gzip_metrics =
            fastq_qc_path(&gzip_path, FastqQcOptions::default()).expect("read gzip by magic bytes");
        let bgzf_metrics =
            fastq_qc_path(&bgzf_path, FastqQcOptions::default()).expect("read BGZF by magic bytes");
        fs::remove_file(gzip_path).expect("remove gzip fixture");
        fs::remove_file(bgzf_path).expect("remove BGZF fixture");

        assert_eq!(gzip_metrics.total_bases, 4);
        assert_eq!(bgzf_metrics.total_bases, 4);
    }

    fn fixture(name: &str) -> Vec<u8> {
        fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/fixtures/fastq-qc")
                .join(name),
        )
        .expect("read FASTQ fixture")
    }

    fn temporary_path(suffix: &str) -> PathBuf {
        let count = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-fastq-qc-{}-{count}-{suffix}",
            std::process::id()
        ))
    }

    fn bgzf_block(payload: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload).expect("deflate BGZF payload");
        let compressed = encoder.finish().expect("finish BGZF deflate stream");
        let block_size = 18 + compressed.len() + 8;
        let block_size_minus_one = u16::try_from(block_size - 1).expect("small BGZF test block");

        let mut block = vec![
            0x1f, 0x8b, 0x08, 0x04, 0, 0, 0, 0, 0, 0xff, 6, 0, b'B', b'C', 2, 0,
        ];
        block.extend_from_slice(&block_size_minus_one.to_le_bytes());
        block.extend_from_slice(&compressed);
        block.extend_from_slice(&crc32(payload).to_le_bytes());
        block.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        block
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }
}
