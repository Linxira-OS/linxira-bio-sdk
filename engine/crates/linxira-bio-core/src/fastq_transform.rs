use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_TRIM_QUALITY: u8 = 20;
pub const DEFAULT_MIN_LENGTH: usize = 20;
pub const DEFAULT_ADAPTER_MIN_OVERLAP: usize = 8;
pub const DEFAULT_ADAPTER: &str = "AGATCGGAAGAGC";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FastqTransformQualityEncoding {
    #[default]
    Phred33,
    Phred64,
}

impl FastqTransformQualityEncoding {
    pub fn offset(self) -> u8 {
        match self {
            Self::Phred33 => 33,
            Self::Phred64 => 64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastqTrimOptions {
    pub min_quality: u8,
    pub min_length: usize,
    pub quality_encoding: FastqTransformQualityEncoding,
}

impl Default for FastqTrimOptions {
    fn default() -> Self {
        Self {
            min_quality: DEFAULT_TRIM_QUALITY,
            min_length: DEFAULT_MIN_LENGTH,
            quality_encoding: FastqTransformQualityEncoding::Phred33,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastqAdapterOptions {
    pub adapters: Vec<String>,
    pub min_overlap: usize,
    pub min_length: usize,
}

impl Default for FastqAdapterOptions {
    fn default() -> Self {
        Self {
            adapters: vec![DEFAULT_ADAPTER.to_owned()],
            min_overlap: DEFAULT_ADAPTER_MIN_OVERLAP,
            min_length: DEFAULT_MIN_LENGTH,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct FastqTransformSummary {
    pub input_read_count: u64,
    pub output_read_count: u64,
    pub discarded_read_count: u64,
    pub trimmed_read_count: u64,
    pub input_bases: u64,
    pub output_bases: u64,
    pub quality_trimmed_bases: u64,
    pub adapter_trimmed_bases: u64,
    pub min_length: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum FastqTransformError {
    Io(io::Error),
    OutputAlreadyExists(PathBuf),
    NoRecords,
    InvalidOption(String),
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

impl Display for FastqTransformError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to process FASTQ: {error}"),
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::NoRecords => formatter.write_str("FASTQ contains no records"),
            Self::InvalidOption(message) => formatter.write_str(message),
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

impl Error for FastqTransformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FastqTransformError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug)]
struct FastqRecord {
    header: Vec<u8>,
    sequence: Vec<u8>,
    quality: Vec<u8>,
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

    fn next_line(&mut self, buffer: &mut Vec<u8>) -> Result<Option<u64>, FastqTransformError> {
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

pub fn fastq_trim_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &FastqTrimOptions,
) -> Result<FastqTransformSummary, FastqTransformError> {
    validate_trim_options(options)?;
    transform_path(
        input.as_ref(),
        output.as_ref(),
        |record, summary| {
            let original_len = record.sequence.len();
            let retained_len = trim_end_by_quality(&record.quality, options);
            record.sequence.truncate(retained_len);
            record.quality.truncate(retained_len);
            let trimmed = original_len - retained_len;
            summary.quality_trimmed_bases += trimmed as u64;
            Ok(trimmed > 0)
        },
        options.min_length,
    )
}

pub fn fastq_adapter_trim_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &FastqAdapterOptions,
) -> Result<FastqTransformSummary, FastqTransformError> {
    let adapters = normalize_adapters(options)?;
    transform_path(
        input.as_ref(),
        output.as_ref(),
        |record, summary| {
            let original_len = record.sequence.len();
            if let Some(cut) = first_adapter_cut(&record.sequence, &adapters, options.min_overlap) {
                record.sequence.truncate(cut);
                record.quality.truncate(cut);
            }
            let trimmed = original_len - record.sequence.len();
            summary.adapter_trimmed_bases += trimmed as u64;
            Ok(trimmed > 0)
        },
        options.min_length,
    )
}

fn transform_path(
    input: &Path,
    output: &Path,
    mut transform: impl FnMut(
        &mut FastqRecord,
        &mut FastqTransformSummary,
    ) -> Result<bool, FastqTransformError>,
    min_length: usize,
) -> Result<FastqTransformSummary, FastqTransformError> {
    if output.exists() {
        return Err(FastqTransformError::OutputAlreadyExists(output.to_owned()));
    }
    if min_length == 0 {
        return Err(FastqTransformError::InvalidOption(
            "min_length must be at least 1".to_owned(),
        ));
    }
    let temporary = temporary_output_path(output);
    let result = (|| {
        let mut reader = open_fastq(input)?;
        let mut writer = BufWriter::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?,
        );
        let mut summary = FastqTransformSummary {
            min_length,
            ..FastqTransformSummary::default()
        };
        let mut line = Vec::new();
        while let Some(record) = read_record(&mut reader, &mut line, summary.input_read_count + 1)?
        {
            summary.input_read_count += 1;
            summary.input_bases += record.sequence.len() as u64;
            let mut record = record;
            let changed = transform(&mut record, &mut summary)?;
            if changed {
                summary.trimmed_read_count += 1;
            }
            if record.sequence.len() < min_length {
                summary.discarded_read_count += 1;
                continue;
            }
            summary.output_read_count += 1;
            summary.output_bases += record.sequence.len() as u64;
            write_record(&mut writer, &record)?;
        }
        if summary.input_read_count == 0 {
            return Err(FastqTransformError::NoRecords);
        }
        if summary.output_read_count == 0 {
            summary
                .warnings
                .push("all reads were discarded by min_length".to_owned());
        }
        writer.flush()?;
        drop(writer);
        fs::rename(&temporary, output)?;
        Ok(summary)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn open_fastq(path: &Path) -> Result<FastqLineReader<Box<dyn BufRead>>, FastqTransformError> {
    let file = File::open(path)?;
    let mut input = BufReader::new(file);
    let is_gzip = input.fill_buf()?.starts_with(&[0x1f, 0x8b]);
    let reader: Box<dyn BufRead> = if is_gzip {
        Box::new(BufReader::new(MultiGzDecoder::new(input)))
    } else {
        Box::new(input)
    };
    Ok(FastqLineReader::new(reader))
}

fn read_record<R: BufRead>(
    reader: &mut FastqLineReader<R>,
    line: &mut Vec<u8>,
    record: u64,
) -> Result<Option<FastqRecord>, FastqTransformError> {
    let Some(header_line) = reader.next_line(line)? else {
        return Ok(None);
    };
    let header = parse_header(line, record, header_line)?.to_vec();
    let identifier = first_ascii_field(&header[1..])
        .ok_or_else(|| malformed(record, header_line, "header has no identifier"))?
        .to_vec();
    let mut sequence = Vec::new();
    loop {
        let Some(line_number) = reader.next_line(line)? else {
            return Err(FastqTransformError::TruncatedRecord {
                record,
                line: reader.next_expected_line(),
                expected: "a '+' separator line",
            });
        };
        if let Some(separator) = line.strip_prefix(b"+") {
            if sequence.is_empty() {
                return Err(malformed(record, line_number, "sequence is empty"));
            }
            validate_separator(separator, &identifier, record, line_number)?;
            break;
        }
        if line.is_empty() {
            return Err(malformed(record, line_number, "sequence line is empty"));
        }
        validate_sequence_line(line, record, line_number)?;
        sequence.extend_from_slice(line);
    }
    let mut quality = Vec::with_capacity(sequence.len());
    while quality.len() < sequence.len() {
        let Some(line_number) = reader.next_line(line)? else {
            return Err(FastqTransformError::TruncatedQuality {
                record,
                line: reader.next_expected_line(),
                sequence_length: sequence.len() as u64,
                quality_length: quality.len() as u64,
            });
        };
        if line.is_empty() {
            return Err(malformed(record, line_number, "quality line is empty"));
        }
        validate_quality_line(line, record, line_number)?;
        quality.extend_from_slice(line);
        if quality.len() > sequence.len() {
            return Err(FastqTransformError::SequenceQualityLengthMismatch {
                record,
                line: line_number,
                sequence_length: sequence.len() as u64,
                quality_length: quality.len() as u64,
            });
        }
    }
    Ok(Some(FastqRecord {
        header,
        sequence,
        quality,
    }))
}

fn write_record(writer: &mut impl Write, record: &FastqRecord) -> Result<(), FastqTransformError> {
    writer.write_all(&record.header)?;
    writer.write_all(b"\n")?;
    writer.write_all(&record.sequence)?;
    writer.write_all(b"\n+\n")?;
    writer.write_all(&record.quality)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn parse_header(line: &[u8], record: u64, line_number: u64) -> Result<&[u8], FastqTransformError> {
    if !line.starts_with(b"@") {
        return Err(malformed(
            record,
            line_number,
            "expected a header beginning with '@'",
        ));
    }
    if first_ascii_field(&line[1..]).is_none() {
        return Err(malformed(record, line_number, "header has no identifier"));
    }
    Ok(line)
}

fn validate_separator(
    separator: &[u8],
    identifier: &[u8],
    record: u64,
    line: u64,
) -> Result<(), FastqTransformError> {
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

fn validate_sequence_line(
    line: &[u8],
    record: u64,
    line_number: u64,
) -> Result<(), FastqTransformError> {
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
    }
    Ok(())
}

fn validate_quality_line(
    line: &[u8],
    record: u64,
    line_number: u64,
) -> Result<(), FastqTransformError> {
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
    }
    Ok(())
}

fn validate_trim_options(options: &FastqTrimOptions) -> Result<(), FastqTransformError> {
    if options.min_length == 0 {
        return Err(FastqTransformError::InvalidOption(
            "min_length must be at least 1".to_owned(),
        ));
    }
    Ok(())
}

fn trim_end_by_quality(record_quality: &[u8], options: &FastqTrimOptions) -> usize {
    let offset = options.quality_encoding.offset();
    let threshold = u16::from(offset) + u16::from(options.min_quality);
    record_quality
        .iter()
        .rposition(|&quality| u16::from(quality) >= threshold)
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn normalize_adapters(options: &FastqAdapterOptions) -> Result<Vec<Vec<u8>>, FastqTransformError> {
    if options.min_length == 0 {
        return Err(FastqTransformError::InvalidOption(
            "min_length must be at least 1".to_owned(),
        ));
    }
    if options.min_overlap == 0 {
        return Err(FastqTransformError::InvalidOption(
            "min_overlap must be at least 1".to_owned(),
        ));
    }
    if options.adapters.is_empty() {
        return Err(FastqTransformError::InvalidOption(
            "at least one adapter sequence is required".to_owned(),
        ));
    }
    options
        .adapters
        .iter()
        .enumerate()
        .map(|(index, adapter)| {
            let adapter = adapter.trim().as_bytes().to_vec();
            if adapter.is_empty() {
                return Err(FastqTransformError::InvalidOption(format!(
                    "adapters[{index}] must not be empty"
                )));
            }
            if adapter.len() < options.min_overlap {
                return Err(FastqTransformError::InvalidOption(format!(
                    "adapters[{index}] is shorter than min_overlap"
                )));
            }
            if !adapter.iter().all(u8::is_ascii_alphabetic) {
                return Err(FastqTransformError::InvalidOption(format!(
                    "adapters[{index}] must contain sequence letters only"
                )));
            }
            Ok(adapter
                .into_iter()
                .map(|base| base.to_ascii_uppercase())
                .collect())
        })
        .collect()
}

fn first_adapter_cut(sequence: &[u8], adapters: &[Vec<u8>], min_overlap: usize) -> Option<usize> {
    (0..sequence.len()).find(|&position| {
        adapters.iter().any(|adapter| {
            let overlap = adapter.len().min(sequence.len() - position);
            overlap >= min_overlap
                && sequence[position..position + overlap]
                    .iter()
                    .zip(adapter.iter())
                    .all(|(&base, &adapter_base)| base.to_ascii_uppercase() == adapter_base)
        })
    })
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

fn malformed(record: u64, line: u64, message: impl Into<String>) -> FastqTransformError {
    FastqTransformError::MalformedRecord {
        record,
        line,
        message: message.into(),
    }
}

fn temporary_output_path(output: &Path) -> PathBuf {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("fastq-output");
    let mut temporary = output.to_owned();
    temporary.set_file_name(format!(".{file_name}.linxira-tmp-{}", std::process::id()));
    temporary
}

#[cfg(test)]
mod tests {
    use super::{
        FastqAdapterOptions, FastqTransformQualityEncoding, FastqTrimOptions,
        fastq_adapter_trim_path, fastq_trim_path,
    };
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn trims_trailing_low_quality_bases_and_discards_short_reads() {
        let input = temporary_path("trim-input.fastq");
        let output = temporary_path("trim-output.fastq");
        fs::write(&input, b"@keep\nACGTAC\n+\nIIII!!\n@drop\nACGT\n+\n!!!!\n")
            .expect("write trim fixture");

        let summary = fastq_trim_path(
            &input,
            &output,
            &FastqTrimOptions {
                min_quality: 20,
                min_length: 4,
                quality_encoding: FastqTransformQualityEncoding::Phred33,
            },
        )
        .expect("trim FASTQ");

        assert_eq!(summary.input_read_count, 2);
        assert_eq!(summary.output_read_count, 1);
        assert_eq!(summary.discarded_read_count, 1);
        assert_eq!(summary.quality_trimmed_bases, 6);
        assert_eq!(
            fs::read_to_string(&output).expect("trimmed FASTQ"),
            "@keep\nACGT\n+\nIIII\n"
        );
        cleanup(&[input, output]);
    }

    #[test]
    fn removes_full_and_partial_adapter_matches() {
        let input = temporary_path("adapter-input.fastq");
        let output = temporary_path("adapter-output.fastq");
        fs::write(
            &input,
            b"@full\nACGTAGATCGGA\n+\nIIIIIIIIIIII\n@partial\nTTTTAGAT\n+\nIIIIIIII\n",
        )
        .expect("write adapter fixture");

        let summary = fastq_adapter_trim_path(
            &input,
            &output,
            &FastqAdapterOptions {
                adapters: vec!["AGATCGGA".to_owned()],
                min_overlap: 4,
                min_length: 1,
            },
        )
        .expect("adapter-trim FASTQ");

        assert_eq!(summary.input_read_count, 2);
        assert_eq!(summary.output_read_count, 2);
        assert_eq!(summary.trimmed_read_count, 2);
        assert_eq!(summary.adapter_trimmed_bases, 12);
        assert_eq!(
            fs::read_to_string(&output).expect("adapter FASTQ"),
            "@full\nACGT\n+\nIIII\n@partial\nTTTT\n+\nIIII\n"
        );
        cleanup(&[input, output]);
    }

    #[test]
    fn reads_gzip_input_by_magic_bytes() {
        let input = temporary_path("adapter-gzip.data");
        let output = temporary_path("adapter-gzip-output.fastq");
        let mut gzip = GzEncoder::new(
            fs::File::create(&input).expect("create gzip fixture"),
            Compression::default(),
        );
        gzip.write_all(b"@read\nACGTAGAT\n+\nIIIIIIII\n")
            .expect("write gzip payload");
        gzip.finish().expect("finish gzip fixture");

        let summary = fastq_adapter_trim_path(
            &input,
            &output,
            &FastqAdapterOptions {
                adapters: vec!["AGAT".to_owned()],
                min_overlap: 4,
                min_length: 1,
            },
        )
        .expect("adapter trim gzip input");

        assert_eq!(summary.output_bases, 4);
        assert_eq!(
            fs::read_to_string(&output).expect("gzip adapter output"),
            "@read\nACGT\n+\nIIII\n"
        );
        cleanup(&[input, output]);
    }

    #[test]
    fn refuses_to_overwrite_outputs() {
        let input = temporary_path("overwrite-input.fastq");
        let output = temporary_path("overwrite-output.fastq");
        fs::write(&input, b"@read\nACGT\n+\nIIII\n").expect("write input");
        fs::write(&output, b"protected\n").expect("write protected output");

        let error = fastq_trim_path(&input, &output, &FastqTrimOptions::default())
            .expect_err("existing output must fail");

        assert!(error.to_string().contains("refusing to overwrite"));
        assert_eq!(
            fs::read_to_string(&output).expect("protected output"),
            "protected\n"
        );
        cleanup(&[input, output]);
    }

    fn temporary_path(suffix: &str) -> PathBuf {
        let count = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-fastq-transform-{}-{count}-{suffix}",
            std::process::id()
        ))
    }

    fn cleanup(paths: &[PathBuf]) {
        for path in paths {
            let _ = fs::remove_file(path);
        }
    }
}
