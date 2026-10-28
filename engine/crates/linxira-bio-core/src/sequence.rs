use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Per-byte classification, one table lookup instead of a branch per base.
///
/// Bit 0: canonical base (A/T/U/G/C), bit 1: G/C, bit 2: N, bit 3: counted as a
/// sequence byte (every non-whitespace byte, matching the previous
/// `bytes().filter(|b| !b.is_ascii_whitespace())`).
///
/// The previous loop matched on `base.to_ascii_uppercase()` per base. On random
/// sequence that branch is unpredictable every time, and branch misprediction - not
/// I/O and not the counting - was the cost: measured on a 101.7 MB, 1.67 M-line FASTA,
/// the branchy loop took ~642 ms on random ACGT against 168-184 ms on an all-A or
/// ACGT-cyclic file of the same size, while the same scan with this table takes
/// ~124 ms (read-only I/O: ~10 ms). The table is built with `to_ascii_uppercase`, so
/// it is case-equivalent by construction rather than by a second hand-written mapping.
///
/// `fastq.rs` reads bits 1 and 2 to classify bases in `fastq.qc`, so a change to those
/// two bits changes both kernels; they are pinned by a test there.
pub(crate) const BASE_CLASS: [u8; 256] = {
    let mut table = [0_u8; 256];
    let mut byte = 0_usize;
    while byte < 256 {
        let value = byte as u8;
        if !value.is_ascii_whitespace() {
            let upper = value.to_ascii_uppercase();
            let mut class = 8_u8;
            if matches!(upper, b'A' | b'T' | b'U' | b'G' | b'C') {
                class |= 1;
            }
            if matches!(upper, b'G' | b'C') {
                class |= 2;
            }
            if upper == b'N' {
                class |= 4;
            }
            table[byte] = class;
        }
        byte += 1;
    }
    table
};

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

        for &base in trimmed.as_bytes() {
            let class = BASE_CLASS[base as usize];
            *length += u64::from(class >> 3 & 1);
            canonical_bases += u64::from(class & 1);
            gc_bases += u64::from(class >> 1 & 1);
            n_count += u64::from(class >> 2 & 1);
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
    fn counts_bases_case_insensitively_and_treats_u_as_canonical() {
        let stats = fasta_stats(Cursor::new(b">mixed\nACGTUNacgtun\n")).expect("valid FASTA");

        assert_eq!(stats.total_bases, 12);
        assert_eq!(stats.n_count, 2, "one N and one n");
        // 10 canonical bases (A/C/G/T/U in either case), 4 of them G or C.
        assert!((stats.gc_percent - 40.0).abs() < 1e-9);
        assert!((stats.n_percent - (2.0 / 12.0) * 100.0).abs() < 1e-9);
    }

    #[test]
    fn interior_whitespace_is_not_a_sequence_byte_but_other_bytes_are() {
        let spaced = fasta_stats(Cursor::new(b">gapped\nAC GT\n")).expect("valid FASTA");
        assert_eq!(spaced.total_bases, 4, "interior whitespace is skipped");
        assert!((spaced.gc_percent - 50.0).abs() < 1e-9);

        // A gap character is counted as a sequence byte (length) but classified as
        // neither canonical nor N, which is what the previous match arm `_ => {}` did.
        let gapped = fasta_stats(Cursor::new(b">gap\nAC-GT\n")).expect("valid FASTA");
        assert_eq!(gapped.total_bases, 5);
        assert_eq!(gapped.n_count, 0);
        assert!(
            (gapped.gc_percent - 50.0).abs() < 1e-9,
            "2 of 4 canonical bases"
        );
    }

    #[test]
    fn uppercase_and_lowercase_input_agree() {
        let upper = fasta_stats(Cursor::new(b">a\nACGTACGTNN\n")).expect("valid FASTA");
        let lower = fasta_stats(Cursor::new(b">a\nacgtacgtnn\n")).expect("valid FASTA");

        assert_eq!(upper.total_bases, lower.total_bases);
        assert_eq!(upper.n_count, lower.n_count);
        assert!((upper.gc_percent - lower.gc_percent).abs() < 1e-12);
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
