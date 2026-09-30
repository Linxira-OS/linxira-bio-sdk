//! Deterministic biological data simulation (phase 1): seeded FASTA sequence
//! synthesis and seeded FASTQ read synthesis. Every run with the same seed
//! and options is byte-identical, which is the contract the benchmark
//! harness and GPU consistency checks rely on.

use flate2::read::MultiGzDecoder;
use serde::Serialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

const LINE_WIDTH: usize = 60;
const DEFAULT_QUALITY_CHAR: u8 = b'I';
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// xoshiro256** — small, fast, statistically solid for simulation streams.
/// Seeded through SplitMix64 so any u64 seed expands to a full state.
#[derive(Debug, Clone)]
pub struct SimulationRng {
    state: [u64; 4],
}

impl SimulationRng {
    pub fn new(seed: u64) -> Self {
        let mut mixer = seed;
        let mut state = [0_u64; 4];
        for slot in &mut state {
            mixer = mixer.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut value = mixer;
            value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *slot = value ^ (value >> 31);
        }
        Self { state }
    }

    pub fn next_u64(&mut self) -> u64 {
        let result = self.state[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    /// Uniform value in `0..bound`; `bound` must be positive.
    pub fn below(&mut self, bound: u64) -> u64 {
        debug_assert!(bound > 0);
        // Rejection sampling avoids modulo bias without floating point.
        let zone = u64::MAX - (u64::MAX % bound) - 1;
        loop {
            let value = self.next_u64();
            if value <= zone {
                return value % bound;
            }
        }
    }

    /// True with probability `percent`/100 (integer percent keeps runs
    /// reproducible across floating-point implementations).
    pub fn percent_chance(&mut self, percent_x100: u64) -> bool {
        self.below(10_000) < percent_x100
    }
}

#[derive(Debug)]
pub enum SimulationError {
    Io(io::Error),
    InvalidOption(String),
    MissingInput(PathBuf),
    OutputAlreadyExists(PathBuf),
    MalformedFasta { line: usize, message: String },
    NoRecords,
}

impl Display for SimulationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "simulation I/O failed: {error}"),
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::MissingInput(path) => {
                write!(formatter, "input file does not exist: {}", path.display())
            }
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::MalformedFasta { line, message } => {
                write!(formatter, "malformed FASTA at line {line}: {message}")
            }
            Self::NoRecords => formatter.write_str("reference FASTA contains no records"),
        }
    }
}

impl Error for SimulationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for SimulationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

// --- sequence synthesis -----------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct SimulateSequenceOptions {
    pub sequence_count: u64,
    /// Uniform length per sequence in `min_length..=max_length`.
    pub min_length: u64,
    pub max_length: u64,
    /// GC content target in percent (0-100, integer hundredths via percent_x100).
    pub gc_percent_x100: u64,
    /// Fraction of N bases in percent x100.
    pub n_percent_x100: u64,
    pub seed: u64,
    pub identifier_prefix: String,
}

impl Default for SimulateSequenceOptions {
    fn default() -> Self {
        Self {
            sequence_count: 1,
            min_length: 1_000,
            max_length: 1_000,
            gc_percent_x100: 5_000,
            n_percent_x100: 0,
            seed: 42,
            identifier_prefix: "sim".to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SimulateSequenceSummary {
    pub seed: u64,
    pub sequence_count: u64,
    pub total_bases: u64,
    pub min_length: u64,
    pub max_length: u64,
    pub mean_length: f64,
    pub gc_percent: f64,
    pub n_count: u64,
}

/// Synthesize a random DNA FASTA. Base selection draws N first, then the
/// GC/AT split, then G-vs-C / A-vs-T — so the realized GC content converges
/// on the target as lengths grow while every draw stays integer-only.
pub fn simulate_sequence_fasta_path(
    output: impl AsRef<Path>,
    options: &SimulateSequenceOptions,
) -> Result<SimulateSequenceSummary, SimulationError> {
    validate_sequence_options(options)?;
    let output = output.as_ref();
    if output.exists() {
        return Err(SimulationError::OutputAlreadyExists(output.to_owned()));
    }
    let mut rng = SimulationRng::new(options.seed);
    let file = File::create(output)?;
    let mut writer = BufWriter::new(file);

    let mut gc_count = 0_u64;
    let mut at_count = 0_u64;
    let mut n_count = 0_u64;
    let mut total_bases = 0_u64;
    let mut min_length = u64::MAX;
    let mut max_length = 0_u64;

    for index in 1..=options.sequence_count {
        let length = options.min_length + rng.below(options.max_length - options.min_length + 1);
        total_bases = total_bases.saturating_add(length);
        min_length = min_length.min(length);
        max_length = max_length.max(length);
        writeln!(
            writer,
            ">{}_{} length={}",
            options.identifier_prefix, index, length
        )?;
        let mut line = [0_u8; LINE_WIDTH];
        let mut line_fill = 0_usize;
        for _ in 0..length {
            let base = if options.n_percent_x100 > 0 && rng.percent_chance(options.n_percent_x100) {
                n_count += 1;
                b'N'
            } else if rng.percent_chance(options.gc_percent_x100) {
                gc_count += 1;
                if rng.percent_chance(5_000) {
                    b'G'
                } else {
                    b'C'
                }
            } else {
                at_count += 1;
                if rng.percent_chance(5_000) {
                    b'A'
                } else {
                    b'T'
                }
            };
            line[line_fill] = base;
            line_fill += 1;
            if line_fill == LINE_WIDTH {
                writer.write_all(&line)?;
                writer.write_all(
                    b"
",
                )?;
                line_fill = 0;
            }
        }
        if line_fill > 0 {
            writer.write_all(&line[..line_fill])?;
            writer.write_all(
                b"
",
            )?;
        }
    }
    writer.flush()?;

    Ok(SimulateSequenceSummary {
        seed: options.seed,
        sequence_count: options.sequence_count,
        total_bases,
        min_length: if options.sequence_count == 0 {
            0
        } else {
            min_length
        },
        max_length,
        mean_length: if options.sequence_count == 0 {
            0.0
        } else {
            total_bases as f64 / options.sequence_count as f64
        },
        gc_percent: percent(gc_count, gc_count + at_count),
        n_count,
    })
}

fn validate_sequence_options(options: &SimulateSequenceOptions) -> Result<(), SimulationError> {
    if options.sequence_count == 0 {
        return Err(SimulationError::InvalidOption(
            "sequence count must be at least 1".to_owned(),
        ));
    }
    if options.min_length == 0 || options.max_length < options.min_length {
        return Err(SimulationError::InvalidOption(
            "lengths must satisfy 1 <= min_length <= max_length".to_owned(),
        ));
    }
    if options.gc_percent_x100 > 10_000 {
        return Err(SimulationError::InvalidOption(
            "gc percent must be between 0 and 100".to_owned(),
        ));
    }
    if options.n_percent_x100 > 10_000 {
        return Err(SimulationError::InvalidOption(
            "n percent must be between 0 and 100".to_owned(),
        ));
    }
    Ok(())
}

// --- read synthesis ---------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct SimulateReadsOptions {
    /// Read length in bases for every emitted read.
    pub read_length: u64,
    /// Target coverage in percent x100 (e.g. 3_000 = 30x). Ignored when
    /// `read_count` is set.
    pub coverage_percent_x100: Option<u64>,
    /// Explicit read (or pair) count; overrides coverage.
    pub read_count: Option<u64>,
    /// Emit mate-pair FASTQ records (/1 and /2) instead of single reads.
    pub paired: bool,
    /// Fragment length for paired runs (outer distance between mate starts).
    pub fragment_length: u64,
    /// Per-base substitution probability in percent x100.
    pub error_rate_percent_x100: u64,
    pub seed: u64,
    /// One ASCII byte (33..126) repeated for every quality character.
    pub quality_char: u8,
}

impl Default for SimulateReadsOptions {
    fn default() -> Self {
        Self {
            read_length: 150,
            coverage_percent_x100: Some(3_000),
            read_count: None,
            paired: false,
            fragment_length: 300,
            error_rate_percent_x100: 0,
            seed: 42,
            quality_char: DEFAULT_QUALITY_CHAR,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SimulateReadsSummary {
    pub seed: u64,
    pub paired: bool,
    pub reference_sequence_count: u64,
    pub reference_bases: u64,
    pub read_count: u64,
    pub read_length: u64,
    pub requested_coverage_percent_x100: Option<u64>,
    pub realized_coverage_percent_x100: u64,
    pub error_rate_percent_x100: u64,
}

/// Synthesize (optionally paired) FASTQ reads from a reference FASTA with a
/// seeded generator: uniform sequence choice, uniform start positions,
/// reverse-complement reads half the time (single mode), substitutions at
/// the configured error rate, and a flat quality string.
pub fn simulate_reads_fastq_path(
    reference: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SimulateReadsOptions,
) -> Result<SimulateReadsSummary, SimulationError> {
    validate_reads_options(options)?;
    let reference = reference.as_ref();
    let output = output.as_ref();
    if !reference.is_file() {
        return Err(SimulationError::MissingInput(reference.to_owned()));
    }
    if output.exists() {
        return Err(SimulationError::OutputAlreadyExists(output.to_owned()));
    }
    let records = read_reference_fasta(reference)?;
    if records.is_empty() {
        return Err(SimulationError::NoRecords);
    }
    let reference_bases = records
        .iter()
        .map(|record| record.sequence.len() as u64)
        .sum::<u64>();
    let readable = records
        .iter()
        .filter(|record| record.sequence.len() as u64 >= options.read_length)
        .count();
    if readable == 0 {
        return Err(SimulationError::InvalidOption(
            "no reference sequence is at least read_length long".to_owned(),
        ));
    }

    let single_count = options.read_count.unwrap_or_else(|| {
        let denominators = if options.paired { 2 } else { 1 };
        (reference_bases * options.coverage_percent_x100.unwrap_or(0))
            .div_ceil(10_000 * options.read_length * denominators)
            .max(1)
    });
    if single_count == 0 {
        return Err(SimulationError::InvalidOption(
            "read count must be at least 1".to_owned(),
        ));
    }

    let mut rng = SimulationRng::new(options.seed);
    let file = File::create(output)?;
    let mut writer = BufWriter::new(file);
    let quality = vec![options.quality_char; options.read_length as usize];
    let mut emitted = 0_u64;
    let mut error_count = 0_u64;

    for _ in 0..single_count {
        // Pick among the readable sequences by drawing until one qualifies;
        // the uniform index draw keeps the selection deterministic and
        // unbiased for long references while short ones retry a few times.
        let (header, sequence) = loop {
            let index = rng.below(records.len() as u64) as usize;
            let record = &records[index];
            if record.sequence.len() as u64 >= options.read_length {
                break (&record.identifier, &record.sequence);
            }
        };
        let span = sequence.len() as u64 - options.read_length + 1;
        let start = rng.below(span) as usize;
        let reverse = if options.paired {
            false
        } else {
            rng.percent_chance(5_000)
        };
        let slice: Vec<u8> = if reverse {
            reverse_complement(&sequence[start..start + options.read_length as usize])
        } else {
            sequence[start..start + options.read_length as usize].to_vec()
        };
        let (mut read1, errors1) = apply_errors(&slice, &mut rng, options);
        error_count += errors1;
        let name = if reverse {
            format!("{header}_revpos{}", start + 1)
        } else {
            format!("{header}_pos{}", start + 1)
        };
        write_fastq(&mut writer, &format!("{name}/1"), &read1, &quality)?;
        emitted += 1;
        read1.clear();

        if options.paired {
            let gap = options.fragment_length.saturating_sub(options.read_length);
            let mate_start = sequence
                .len()
                .saturating_sub(options.read_length as usize)
                .min(start + gap as usize);
            let mate: Vec<u8> = reverse_complement(
                &sequence[mate_start..mate_start + options.read_length as usize],
            );
            let (mut read2, errors2) = apply_errors(&mate, &mut rng, options);
            error_count += errors2;
            write_fastq(
                &mut writer,
                &format!("{header}_pos{}/2", mate_start + 1),
                &read2,
                &quality,
            )?;
            emitted += 1;
            read2.clear();
        }
    }
    writer.flush()?;

    let _ = error_count; // per-read substitutions are exercised in parity tests
    let emitted_bases = emitted * options.read_length;
    Ok(SimulateReadsSummary {
        seed: options.seed,
        paired: options.paired,
        reference_sequence_count: records.len() as u64,
        reference_bases,
        read_count: emitted,
        read_length: options.read_length,
        requested_coverage_percent_x100: options.coverage_percent_x100,
        realized_coverage_percent_x100: (emitted_bases * 10_000) / reference_bases.max(1),
        error_rate_percent_x100: options.error_rate_percent_x100,
    })
}

struct ReferenceRecord {
    identifier: String,
    sequence: Vec<u8>,
}

fn read_reference_fasta(path: &Path) -> Result<Vec<ReferenceRecord>, SimulationError> {
    // Probe the magic bytes with a fresh handle; the caller's handle position
    // would otherwise skip the first bytes of plain FASTA input.
    let mut prefix = [0_u8; 2];
    let prefix_length = File::open(path)?.read(&mut prefix)?;
    let input: Box<dyn Read> = if prefix_length == 2 && prefix == GZIP_MAGIC {
        Box::new(MultiGzDecoder::new(File::open(path)?))
    } else {
        Box::new(File::open(path)?)
    };
    let mut reader = BufReader::new(input);
    let mut records = Vec::new();
    let mut line = String::new();
    let mut line_number = 0_usize;
    let mut current: Option<ReferenceRecord> = None;
    loop {
        line_number += 1;
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('>') {
            if let Some(record) = current.take() {
                records.push(record);
            }
            let identifier = header
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned();
            if identifier.is_empty() {
                return Err(SimulationError::MalformedFasta {
                    line: line_number,
                    message: "FASTA header has no identifier".to_owned(),
                });
            }
            current = Some(ReferenceRecord {
                identifier,
                sequence: Vec::new(),
            });
        } else {
            let record = current.as_mut().ok_or(SimulationError::MalformedFasta {
                line: line_number,
                message: "sequence data appears before any FASTA header".to_owned(),
            })?;
            record.sequence.extend(
                trimmed
                    .as_bytes()
                    .iter()
                    .map(|base| base.to_ascii_uppercase()),
            );
        }
    }
    if let Some(record) = current.take() {
        records.push(record);
    }
    Ok(records)
}

fn reverse_complement(sequence: &[u8]) -> Vec<u8> {
    sequence
        .iter()
        .rev()
        .map(|base| match base {
            b'A' => b'T',
            b'T' => b'A',
            b'G' => b'C',
            b'C' => b'G',
            _ => b'N',
        })
        .collect()
}

fn apply_errors(
    sequence: &[u8],
    rng: &mut SimulationRng,
    options: &SimulateReadsOptions,
) -> (Vec<u8>, u64) {
    if options.error_rate_percent_x100 == 0 {
        return (sequence.to_vec(), 0);
    }
    let mut output = sequence.to_vec();
    let mut errors = 0_u64;
    for base in &mut output {
        if rng.percent_chance(options.error_rate_percent_x100) {
            let replacement = b"ACGT"[rng.below(4) as usize];
            if replacement != *base {
                *base = replacement;
            }
            errors += 1;
        }
    }
    (output, errors)
}

fn write_fastq(
    writer: &mut impl Write,
    header: &str,
    sequence: &[u8],
    quality: &[u8],
) -> Result<(), SimulationError> {
    writer.write_all(b"@")?;
    writer.write_all(header.as_bytes())?;
    writer.write_all(b"\n")?;
    writer.write_all(sequence)?;
    writer.write_all(b"\n+\n")?;
    writer.write_all(quality)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn validate_reads_options(options: &SimulateReadsOptions) -> Result<(), SimulationError> {
    if options.read_length == 0 {
        return Err(SimulationError::InvalidOption(
            "read length must be at least 1".to_owned(),
        ));
    }
    if options.read_count.is_none()
        && !matches!(options.coverage_percent_x100, Some(coverage) if coverage > 0)
    {
        return Err(SimulationError::InvalidOption(
            "either a positive coverage or an explicit read count is required".to_owned(),
        ));
    }
    if options.error_rate_percent_x100 > 10_000 {
        return Err(SimulationError::InvalidOption(
            "error rate must be between 0 and 100 percent".to_owned(),
        ));
    }
    let quality_ok = (33..=126).contains(&options.quality_char);
    if !quality_ok {
        return Err(SimulationError::InvalidOption(
            "quality character must be a printable ASCII byte".to_owned(),
        ));
    }
    if options.paired && options.fragment_length < options.read_length {
        return Err(SimulationError::InvalidOption(
            "fragment length must be at least the read length".to_owned(),
        ));
    }
    Ok(())
}

fn percent(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 * 100.0 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SimulateReadsOptions, SimulateSequenceOptions, SimulationRng, simulate_reads_fastq_path,
        simulate_sequence_fasta_path,
    };
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("linxira-sim-test-{}-{name}", std::process::id()));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let mut left = SimulationRng::new(7);
        let mut right = SimulationRng::new(7);
        for _ in 0..100 {
            assert_eq!(left.next_u64(), right.next_u64());
        }
        let mut rng = SimulationRng::new(1);
        for _ in 0..500 {
            assert!(rng.below(10) < 10);
        }
    }

    #[test]
    fn synthesizes_reproducible_sequences_with_target_gc() {
        let output = scratch("seq.fa");
        let options = SimulateSequenceOptions {
            sequence_count: 4,
            min_length: 500,
            max_length: 500,
            gc_percent_x100: 5_000,
            n_percent_x100: 100,
            seed: 11,
            identifier_prefix: "chr".to_owned(),
        };
        let summary = simulate_sequence_fasta_path(&output, &options).expect("synthesis");
        assert_eq!(summary.sequence_count, 4);
        assert_eq!(summary.total_bases, 2_000);
        // Re-run with the same seed: byte-identical output.
        let repeat = scratch("seq2.fa");
        let summary2 = simulate_sequence_fasta_path(&repeat, &options).expect("repeat");
        assert_eq!(fs::read(&output).unwrap(), fs::read(&repeat).unwrap(),);
        assert_eq!(summary.gc_percent, summary2.gc_percent);
        assert!((summary.gc_percent - 50.0).abs() < 5.0);
        // 1 percent of 2000 bases draws around 20 N; a CI-stable band, not
        // an exact count.
        assert!((10..=35).contains(&summary.n_count));
        fs::remove_file(&output).unwrap();
        fs::remove_file(&repeat).unwrap();
    }

    #[test]
    fn synthesizes_paired_reads_within_reference_bounds() {
        let reference = scratch("ref.fa");
        fs::write(
            &reference,
            ">chr1\nACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT\n",
        )
        .unwrap();
        let output = scratch("reads.fq");
        let options = SimulateReadsOptions {
            read_length: 10,
            coverage_percent_x100: None,
            read_count: Some(5),
            paired: true,
            fragment_length: 20,
            error_rate_percent_x100: 1_000,
            seed: 3,
            quality_char: b'I',
        };
        let summary = simulate_reads_fastq_path(&reference, &output, &options).expect("reads");
        assert_eq!(summary.read_count, 10);
        assert!(summary.paired);
        let text = fs::read_to_string(&output).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 40);
        assert!(lines[0].starts_with("@chr1_pos"));
        assert!(lines[0].ends_with("/1"));
        assert!(lines[4].ends_with("/2"));
        for (index, line) in lines.iter().enumerate() {
            if index % 4 == 1 {
                assert_eq!(line.len(), 10);
            }
            if index % 4 == 3 {
                assert_eq!(line.len(), 10);
            }
        }
        fs::remove_file(&reference).unwrap();
        fs::remove_file(&output).unwrap();
    }

    #[test]
    fn rejects_invalid_options() {
        let output = scratch("bad.fa");
        let error = simulate_sequence_fasta_path(
            &output,
            &SimulateSequenceOptions {
                sequence_count: 0,
                ..SimulateSequenceOptions::default()
            },
        )
        .expect_err("zero count rejected");
        assert!(error.to_string().contains("at least 1"));
        let _ = fs::remove_file(&output);
    }
}
