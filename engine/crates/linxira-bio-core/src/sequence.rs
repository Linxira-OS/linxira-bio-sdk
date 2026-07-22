use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SequenceStats {
    pub sequence_count: u64,
    pub total_bases: u64,
    pub min_length: u64,
    pub max_length: u64,
    pub mean_length: f64,
    pub n50: u64,
    pub l50: u64,
    pub au_n: f64,
    pub gc_percent: f64,
    pub n_count: u64,
    pub n_percent: f64,
}

#[derive(Debug)]
pub enum FastaError {
    Io(io::Error),
    EmptyIdentifier { line: usize },
    SequenceBeforeHeader { line: usize },
    NoRecords,
}

impl Display for FastaError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "failed to read FASTA: {error}"),
            Self::EmptyIdentifier { line } => {
                write!(formatter, "FASTA header at line {line} has no identifier")
            }
            Self::SequenceBeforeHeader { line } => {
                write!(
                    formatter,
                    "sequence data appears before a FASTA header at line {line}"
                )
            }
            Self::NoRecords => write!(formatter, "FASTA contains no records"),
        }
    }
}

impl Error for FastaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for FastaError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn fasta_stats(reader: impl BufRead) -> Result<SequenceStats, FastaError> {
    let mut lengths = Vec::new();
    let mut current_length = None;
    let mut gc_bases = 0_u64;
    let mut canonical_bases = 0_u64;
    let mut n_count = 0_u64;

    for (line_index, line) in reader.lines().enumerate() {
        let line_number = line_index + 1;
        let line = line?;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if let Some(header) = trimmed.strip_prefix('>') {
            if header.split_whitespace().next().is_none() {
                return Err(FastaError::EmptyIdentifier { line: line_number });
            }
            if let Some(length) = current_length.replace(0) {
                lengths.push(length);
            }
            continue;
        }

        let length = current_length
            .as_mut()
            .ok_or(FastaError::SequenceBeforeHeader { line: line_number })?;

        for base in trimmed.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            *length += 1;
            match base.to_ascii_uppercase() {
                b'G' | b'C' => {
                    gc_bases += 1;
                    canonical_bases += 1;
                }
                b'A' | b'T' | b'U' => canonical_bases += 1,
                b'N' => n_count += 1,
                _ => {}
            }
        }
    }

    if let Some(length) = current_length {
        lengths.push(length);
    }
    if lengths.is_empty() {
        return Err(FastaError::NoRecords);
    }

    let sequence_count = u64::try_from(lengths.len()).expect("sequence count fits in u64");
    let total_bases = lengths.iter().sum::<u64>();
    let min_length = lengths.iter().copied().min().unwrap_or(0);
    let max_length = lengths.iter().copied().max().unwrap_or(0);
    let mean_length = ratio(total_bases, sequence_count);

    lengths.sort_unstable_by(|left, right| right.cmp(left));
    let threshold = total_bases.saturating_add(1) / 2;
    let mut cumulative = 0_u64;
    let mut n50 = 0_u64;
    let mut l50 = 0_u64;
    if threshold > 0 {
        for (index, length) in lengths.iter().copied().enumerate() {
            cumulative += length;
            if cumulative >= threshold {
                n50 = length;
                l50 = u64::try_from(index + 1).expect("sequence count fits in u64");
                break;
            }
        }
    }

    let squared_length_sum = lengths
        .iter()
        .map(|length| (*length as f64) * (*length as f64))
        .sum::<f64>();
    let au_n = if total_bases == 0 {
        0.0
    } else {
        squared_length_sum / total_bases as f64
    };

    Ok(SequenceStats {
        sequence_count,
        total_bases,
        min_length,
        max_length,
        mean_length,
        n50,
        l50,
        au_n,
        gc_percent: percent(gc_bases, canonical_bases),
        n_count,
        n_percent: percent(n_count, total_bases),
    })
}

pub fn fasta_stats_path(path: impl AsRef<Path>) -> Result<SequenceStats, FastaError> {
    let path = path.as_ref();
    let mut prefix = [0_u8; 2];
    let prefix_length = File::open(path)?.read(&mut prefix)?;
    let input: Box<dyn Read> = if prefix_length == 2 && prefix == [0x1f, 0x8b] {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    fasta_stats(BufReader::new(input))
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

#[cfg(test)]
mod tests {
    use super::{FastaError, fasta_stats, fasta_stats_path};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs;
    use std::io::{Cursor, Write};

    #[test]
    fn calculates_expected_statistics() {
        let input = b">one\nACGTNN\n>two\nGGGG\n>three\nAT\n";
        let stats = fasta_stats(Cursor::new(input)).expect("valid FASTA");

        assert_eq!(stats.sequence_count, 3);
        assert_eq!(stats.total_bases, 12);
        assert_eq!(stats.min_length, 2);
        assert_eq!(stats.max_length, 6);
        assert_eq!(stats.n50, 6);
        assert_eq!(stats.l50, 1);
        assert!((stats.au_n - 4.666_666_666_7).abs() < 1e-9);
        assert!((stats.gc_percent - 60.0).abs() < 1e-9);
        assert_eq!(stats.n_count, 2);
    }

    #[test]
    fn rejects_sequence_before_header() {
        let error =
            fasta_stats(Cursor::new(b"ACGT\n")).expect_err("sequence without a header must fail");

        assert!(matches!(
            error,
            FastaError::SequenceBeforeHeader { line: 1 }
        ));
    }

    #[test]
    fn rejects_empty_input() {
        let error = fasta_stats(Cursor::new(b"\n")).expect_err("empty input must fail");

        assert!(matches!(error, FastaError::NoRecords));
    }

    #[test]
    fn reads_gzip_compressed_fasta_by_magic_bytes() {
        let path = std::env::temp_dir().join(format!(
            "linxira-sequence-stats-{}.data",
            std::process::id()
        ));
        let mut encoder = GzEncoder::new(
            fs::File::create(&path).expect("create fixture"),
            Compression::default(),
        );
        encoder
            .write_all(b">one\nACGT\n>two\nNN\n")
            .expect("write fixture");
        encoder.finish().expect("finish gzip stream");

        let stats = fasta_stats_path(&path).expect("read compressed FASTA");
        fs::remove_file(path).expect("remove fixture");

        assert_eq!(stats.sequence_count, 2);
        assert_eq!(stats.total_bases, 6);
    }
}
