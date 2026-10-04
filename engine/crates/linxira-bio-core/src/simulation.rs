//! Deterministic biological data simulation (phase 1): seeded FASTA sequence
//! synthesis and seeded FASTQ read synthesis. Every run with the same seed
//! and options is byte-identical, which is the contract the benchmark
//! harness and GPU consistency checks rely on.

use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
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

// --- mechanistic cell simulation ---------------------------------------------
//
// Minimal whole-cell-style lineage simulator: every gene runs a telegraph
// (two-state promoter) model, translation is throttled by a Monod factor of
// the remaining substrate, synthesized protein accrues biomass, and reaching
// the division threshold splits the cell with binomial molecule partitioning.
// The Gillespie direct method drives every reaction from the shared
// xoshiro256** stream, so a run is byte-reproducible from
// (seed, model, options) alone — the contract the benchmark harness and
// cross-machine rental ledgers rely on.

/// Reaction rates for one gene, all in events per minute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellGeneParams {
    pub id: String,
    /// Promoter activation rate (OFF -> ON).
    pub kon: f64,
    /// Promoter deactivation rate (ON -> OFF).
    pub koff: f64,
    /// Transcription rate while the promoter is ON.
    pub ks: f64,
    /// mRNA decay rate.
    pub kdm: f64,
    /// Translation rate per mRNA molecule.
    pub ktl: f64,
    /// Protein decay rate.
    pub kdp: f64,
    /// Biomass units contributed by each synthesized protein molecule.
    pub yield_per_protein: f64,
}

/// Mechanistic cell model consumed by `simulate cell` and echoed in the
/// summary so every run is self-describing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellModel {
    /// Substrate half-saturation constant of the Monod factor.
    pub monod_constant: f64,
    /// Initial substrate units; integer units partition binomially at division.
    pub substrate_initial: u64,
    /// Biomass threshold that triggers division.
    pub division_biomass: f64,
    pub genes: Vec<CellGeneParams>,
}

impl CellGeneParams {
    fn builtin(index: usize) -> Self {
        // Hand-picked varied rates: every gene bursts, none dominates the
        // biomass budget.
        const TABLE: [[f64; 7]; 12] = [
            [1.00, 0.50, 1.5, 0.12, 3.0, 0.020, 0.15],
            [0.80, 0.60, 1.2, 0.10, 2.5, 0.020, 0.15],
            [1.20, 0.40, 1.0, 0.15, 3.5, 0.025, 0.15],
            [0.60, 0.80, 1.8, 0.11, 2.0, 0.020, 0.15],
            [0.90, 0.50, 1.4, 0.13, 3.2, 0.022, 0.15],
            [0.70, 0.70, 1.1, 0.14, 2.8, 0.020, 0.15],
            [1.10, 0.45, 1.6, 0.12, 3.1, 0.024, 0.15],
            [0.50, 0.90, 0.9, 0.16, 2.2, 0.020, 0.15],
            [1.00, 0.55, 1.3, 0.12, 2.9, 0.021, 0.15],
            [0.85, 0.60, 1.5, 0.13, 3.3, 0.023, 0.15],
            [0.75, 0.65, 1.0, 0.15, 2.6, 0.020, 0.15],
            [0.95, 0.50, 1.7, 0.11, 3.4, 0.025, 0.15],
        ];
        let row = TABLE[index % TABLE.len()];
        Self {
            id: format!("gene{:02}", index + 1),
            kon: row[0],
            koff: row[1],
            ks: row[2],
            kdm: row[3],
            ktl: row[4],
            kdp: row[5],
            yield_per_protein: row[6],
        }
    }
}

impl CellModel {
    /// Built-in 12-gene panel: bursting expression, Monod-coupled growth, and
    /// substrate exhaustion (visible growth slowdown) after roughly a dozen
    /// divisions on a default-length lineage.
    pub fn builtin() -> Self {
        Self {
            monod_constant: 1000.0,
            substrate_initial: 100_000,
            division_biomass: 150.0,
            genes: (0..12).map(CellGeneParams::builtin).collect(),
        }
    }

    fn validate(&self) -> Result<(), SimulationError> {
        if self.genes.is_empty() {
            return Err(SimulationError::InvalidOption(
                "cell model needs at least 1 gene".to_owned(),
            ));
        }
        if self.genes.len() > 4096 {
            return Err(SimulationError::InvalidOption(
                "cell model supports at most 4096 genes".to_owned(),
            ));
        }
        if !self.monod_constant.is_finite() || self.monod_constant < 0.0 {
            return Err(SimulationError::InvalidOption(
                "monod_constant must be finite and non-negative".to_owned(),
            ));
        }
        if !self.division_biomass.is_finite() || self.division_biomass <= 0.0 {
            return Err(SimulationError::InvalidOption(
                "division_biomass must be finite and positive".to_owned(),
            ));
        }
        for gene in &self.genes {
            if gene.id.is_empty() {
                return Err(SimulationError::InvalidOption(
                    "every gene needs a non-empty id".to_owned(),
                ));
            }
            for (name, rate) in [
                ("kon", gene.kon),
                ("koff", gene.koff),
                ("ks", gene.ks),
                ("kdm", gene.kdm),
                ("ktl", gene.ktl),
                ("kdp", gene.kdp),
                ("yield_per_protein", gene.yield_per_protein),
            ] {
                if !rate.is_finite() || rate < 0.0 {
                    return Err(SimulationError::InvalidOption(format!(
                        "gene {} rate {name} must be finite and non-negative",
                        gene.id
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Options for `simulate cell`; `model: None` selects the built-in panel.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulateCellOptions {
    /// Optional model JSON path; `None` uses `CellModel::builtin`.
    pub model: Option<PathBuf>,
    /// Simulated minutes for the lineage run.
    pub end_time: f64,
    pub seed: u64,
    /// Equal-width time-series bins for the TSV output.
    pub bins: u32,
    /// Upper bound on Gillespie events (guards runaway rate sets).
    pub max_events: u64,
}

impl Default for SimulateCellOptions {
    fn default() -> Self {
        Self {
            model: None,
            // Two simulated hours: the built-in panel ramps up, divides fast,
            // exhausts the medium (Monod collapse), and enters stationary
            // phase — the full arc without minutes of dead tail.
            end_time: 120.0,
            seed: 42,
            bins: 100,
            max_events: 20_000_000,
        }
    }
}

/// Time-averaged per-gene expression over the whole run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CellGeneSummary {
    pub id: String,
    pub mean_mrna: f64,
    pub mean_protein: f64,
    pub final_mrna: u64,
    pub final_protein: u64,
    pub transcription_events: u64,
    pub translation_events: u64,
}

/// Summary of one lineage run; byte-identical for identical seed/model/options.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SimulateCellSummary {
    pub seed: u64,
    /// "builtin" or the model path as given on the command line.
    pub model_source: String,
    pub model: CellModel,
    pub end_time: f64,
    pub bins: u32,
    pub event_count: u64,
    pub transcription_events: u64,
    pub translation_events: u64,
    pub division_count: u64,
    /// Generations completed by the tracked lineage.
    pub lineage_depth: u64,
    pub initial_substrate: u64,
    pub final_substrate: u64,
    pub final_biomass: f64,
    /// Mean minutes between the first and last division; 0.0 with <2 divisions.
    pub mean_generation_time: f64,
    pub per_gene: Vec<CellGeneSummary>,
}

const CHANNELS_PER_GENE: usize = 5;

#[derive(Debug)]
struct LineageState {
    promoter_on: Vec<bool>,
    mrna: Vec<u64>,
    protein: Vec<u64>,
    substrate: u64,
    biomass: f64,
    time: f64,
    depth: u64,
}

#[derive(Debug, Default, Clone)]
struct BinAccumulator {
    dt: f64,
    mrna: f64,
    protein: f64,
    biomass: f64,
    monod: f64,
    divisions: u64,
}

#[derive(Debug)]
struct LineageOutputs {
    bins: Vec<BinAccumulator>,
    transcription: Vec<u64>,
    translation: Vec<u64>,
    transcription_total: u64,
    translation_total: u64,
    gene_mrna_integral: Vec<f64>,
    gene_protein_integral: Vec<f64>,
    division_times: Vec<f64>,
    event_count: u64,
    final_state: LineageState,
}

fn validate_cell_options(options: &SimulateCellOptions) -> Result<(), SimulationError> {
    if !options.end_time.is_finite() || options.end_time <= 0.0 {
        return Err(SimulationError::InvalidOption(
            "end time must be finite and positive".to_owned(),
        ));
    }
    if options.bins == 0 || options.bins > 100_000 {
        return Err(SimulationError::InvalidOption(
            "bins must be between 1 and 100000".to_owned(),
        ));
    }
    if options.max_events == 0 {
        return Err(SimulationError::InvalidOption(
            "max events must be at least 1".to_owned(),
        ));
    }
    Ok(())
}

/// 53 random mantissa bits -> uniform in `[0, 1)`.
fn uniform_unit_interval(rng: &mut SimulationRng) -> f64 {
    (rng.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

fn exponential_waiting_time(rng: &mut SimulationRng, total_rate: f64) -> f64 {
    -(1.0 - uniform_unit_interval(rng)).ln() / total_rate
}

/// Independent fair coin flips; n can be large (substrate units), so this is
/// the only part of the run whose cost scales with molecule counts.
fn binomial_half(rng: &mut SimulationRng, count: u64) -> u64 {
    (0..count).filter(|_| rng.below(2) == 0).count() as u64
}

fn run_lineage(
    model: &CellModel,
    options: &SimulateCellOptions,
) -> Result<LineageOutputs, SimulationError> {
    let gene_count = model.genes.len();
    let mut state = LineageState {
        promoter_on: vec![false; gene_count],
        mrna: vec![0; gene_count],
        protein: vec![0; gene_count],
        substrate: model.substrate_initial,
        biomass: 0.0,
        time: 0.0,
        depth: 0,
    };
    let bin_width = options.end_time / options.bins as f64;
    let mut bins = vec![BinAccumulator::default(); options.bins as usize];
    let mut gene_mrna_integral = vec![0.0; gene_count];
    let mut gene_protein_integral = vec![0.0; gene_count];
    let mut transcription: Vec<u64> = vec![0; gene_count];
    let mut translation: Vec<u64> = vec![0; gene_count];
    let mut division_times: Vec<f64> = Vec::new();
    let mut rng = SimulationRng::new(options.seed);
    let mut propensities = vec![0.0_f64; gene_count * CHANNELS_PER_GENE];
    let mut event_count: u64 = 0;

    // Accumulate `elapsed` minutes of the current (constant) state into the
    // binned time series, splitting at bin boundaries. Bins advance by
    // arithmetic index, never by re-walking a floating-point cursor, so the
    // loop always terminates.
    let accumulate = |state: &LineageState,
                      monod: f64,
                      start: f64,
                      elapsed: f64,
                      bins: &mut Vec<BinAccumulator>,
                      gene_mrna_integral: &mut [f64],
                      gene_protein_integral: &mut [f64]| {
        if elapsed <= 0.0 {
            return;
        }
        let total_mrna = state.mrna.iter().sum::<u64>() as f64;
        let total_protein = state.protein.iter().sum::<u64>() as f64;
        let first_index = ((start / bin_width) as usize).min(bins.len() - 1);
        let last_index = (((start + elapsed) / bin_width) as usize).min(bins.len() - 1);
        for index in first_index..=last_index {
            let bin_start = bin_width * index as f64;
            let bin_end = if index + 1 == bins.len() {
                options.end_time
            } else {
                bin_width * (index + 1) as f64
            };
            let lo = if index == first_index {
                start
            } else {
                bin_start
            };
            let hi = if index == last_index {
                start + elapsed
            } else {
                bin_end
            };
            let chunk = hi - lo;
            if chunk <= 0.0 {
                continue;
            }
            let bin = &mut bins[index];
            bin.dt += chunk;
            bin.mrna += total_mrna * chunk;
            bin.protein += total_protein * chunk;
            bin.biomass += state.biomass * chunk;
            bin.monod += monod * chunk;
            for slot in 0..gene_count {
                gene_mrna_integral[slot] += state.mrna[slot] as f64 * chunk;
                gene_protein_integral[slot] += state.protein[slot] as f64 * chunk;
            }
        }
    };

    while state.time < options.end_time {
        let current_monod = if state.substrate == 0 {
            0.0
        } else {
            let substrate = state.substrate as f64;
            substrate / (model.monod_constant + substrate)
        };
        let mut total_rate = 0.0;
        for (index, gene) in model.genes.iter().enumerate() {
            let on = state.promoter_on[index];
            let mrna = state.mrna[index] as f64;
            let protein = state.protein[index] as f64;
            let rates = [
                if on { gene.koff } else { gene.kon },
                if on { gene.ks } else { 0.0 },
                gene.kdm * mrna,
                gene.ktl * current_monod * mrna,
                gene.kdp * protein,
            ];
            for (slot, rate) in rates.iter().enumerate() {
                propensities[index * CHANNELS_PER_GENE + slot] = *rate;
            }
            total_rate += rates[0] + rates[1] + rates[2] + rates[3] + rates[4];
        }
        if total_rate <= 0.0 {
            // Absorbing state: nothing can ever fire again.
            let elapsed = options.end_time - state.time;
            accumulate(
                &state,
                current_monod,
                state.time,
                elapsed,
                &mut bins,
                &mut gene_mrna_integral,
                &mut gene_protein_integral,
            );
            state.time = options.end_time;
            break;
        }
        let waiting = exponential_waiting_time(&mut rng, total_rate);
        let next_time = state.time + waiting;
        let elapsed = if next_time >= options.end_time {
            options.end_time - state.time
        } else {
            waiting
        };
        accumulate(
            &state,
            current_monod,
            state.time,
            elapsed,
            &mut bins,
            &mut gene_mrna_integral,
            &mut gene_protein_integral,
        );
        if next_time >= options.end_time {
            state.time = options.end_time;
            break;
        }
        state.time = next_time;

        // Pick the firing channel in fixed gene-then-reaction order.
        let mut target = uniform_unit_interval(&mut rng) * total_rate;
        let mut chosen = propensities.len() - 1;
        for (slot, rate) in propensities.iter().enumerate() {
            target -= rate;
            if target < 0.0 {
                chosen = slot;
                break;
            }
        }
        // Floating-point residue guard: land on the last positive channel.
        if propensities[chosen] == 0.0 {
            chosen = propensities
                .iter()
                .rposition(|rate| *rate > 0.0)
                .unwrap_or(chosen);
        }

        let gene_index = chosen / CHANNELS_PER_GENE;
        match chosen % CHANNELS_PER_GENE {
            // Slot 0 carries kon (when OFF) or koff (when ON) — a toggle.
            0 => state.promoter_on[gene_index] = !state.promoter_on[gene_index],
            1 => {
                state.mrna[gene_index] += 1;
                transcription[gene_index] += 1;
            }
            2 => state.mrna[gene_index] -= 1,
            3 => {
                state.protein[gene_index] += 1;
                state.biomass += model.genes[gene_index].yield_per_protein;
                state.substrate = state.substrate.saturating_sub(1);
                translation[gene_index] += 1;
            }
            _ => state.protein[gene_index] -= 1,
        }
        event_count += 1;
        if event_count > options.max_events {
            return Err(SimulationError::InvalidOption(format!(
                "event budget exhausted after {} events; raise max events, shorten the run, or reduce the gene count",
                options.max_events
            )));
        }

        // Biomass only changes on translation, so division is checked here.
        if state.biomass >= model.division_biomass {
            for index in 0..gene_count {
                state.mrna[index] = binomial_half(&mut rng, state.mrna[index]);
                state.protein[index] = binomial_half(&mut rng, state.protein[index]);
            }
            state.substrate = binomial_half(&mut rng, state.substrate);
            state.biomass /= 2.0;
            state.depth += 1;
            division_times.push(state.time);
            let index = ((state.time / bin_width) as usize).min(bins.len() - 1);
            bins[index].divisions += 1;
        }
    }

    let transcription_total = transcription.iter().sum();
    let translation_total = translation.iter().sum();
    Ok(LineageOutputs {
        bins,
        transcription,
        translation,
        transcription_total,
        translation_total,
        gene_mrna_integral,
        gene_protein_integral,
        division_times,
        event_count,
        final_state: state,
    })
}

fn write_time_series(
    output: &Path,
    options: &SimulateCellOptions,
    bins: &[BinAccumulator],
) -> Result<(), SimulationError> {
    let file = File::create(output)?;
    let mut writer = BufWriter::new(file);
    writeln!(
        writer,
        "bin\tt_start\tt_end\tmean_total_mrna\tmean_total_protein\tmean_biomass\tmean_monod_factor\tdivisions"
    )?;
    let bin_width = options.end_time / options.bins as f64;
    for (index, bin) in bins.iter().enumerate() {
        let start = bin_width * index as f64;
        let end = if index + 1 == bins.len() {
            options.end_time
        } else {
            bin_width * (index + 1) as f64
        };
        let mean = |sum: f64| if bin.dt > 0.0 { sum / bin.dt } else { 0.0 };
        writeln!(
            writer,
            "{}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{}",
            index,
            start,
            end,
            mean(bin.mrna),
            mean(bin.protein),
            mean(bin.biomass),
            mean(bin.monod),
            bin.divisions
        )?;
    }
    writer.flush()?;
    Ok(())
}

/// Simulates one lineage with the exact Gillespie direct method and writes the
/// binned time-series TSV. Identical `(seed, model, options)` inputs produce
/// byte-identical outputs.
pub fn simulate_cell_path(
    output: impl AsRef<Path>,
    options: &SimulateCellOptions,
) -> Result<SimulateCellSummary, SimulationError> {
    validate_cell_options(options)?;
    let (model, model_source) = match &options.model {
        Some(path) => {
            let file = File::open(path).map_err(|_| SimulationError::MissingInput(path.clone()))?;
            let parsed: CellModel =
                serde_json::from_reader(BufReader::new(file)).map_err(|error| {
                    SimulationError::InvalidOption(format!("invalid cell model JSON: {error}"))
                })?;
            (parsed, path.display().to_string())
        }
        None => (CellModel::builtin(), "builtin".to_owned()),
    };
    model.validate()?;
    let output = output.as_ref();
    if output.exists() {
        return Err(SimulationError::OutputAlreadyExists(output.to_path_buf()));
    }
    let initial_substrate = model.substrate_initial;
    let outputs = run_lineage(&model, options)?;
    write_time_series(output, options, &outputs.bins)?;
    let mean_generation_time = match outputs.division_times.as_slice() {
        [first, .., last] => (last - first) / (outputs.division_times.len() - 1) as f64,
        _ => 0.0,
    };
    let end_time = options.end_time;
    let per_gene = model
        .genes
        .iter()
        .enumerate()
        .map(|(index, gene)| CellGeneSummary {
            id: gene.id.clone(),
            mean_mrna: outputs.gene_mrna_integral[index] / end_time,
            mean_protein: outputs.gene_protein_integral[index] / end_time,
            final_mrna: outputs.final_state.mrna[index],
            final_protein: outputs.final_state.protein[index],
            transcription_events: outputs.transcription[index],
            translation_events: outputs.translation[index],
        })
        .collect();
    Ok(SimulateCellSummary {
        seed: options.seed,
        model_source,
        model,
        end_time,
        bins: options.bins,
        event_count: outputs.event_count,
        transcription_events: outputs.transcription_total,
        translation_events: outputs.translation_total,
        division_count: outputs.division_times.len() as u64,
        lineage_depth: outputs.final_state.depth,
        initial_substrate,
        final_substrate: outputs.final_state.substrate,
        final_biomass: outputs.final_state.biomass,
        mean_generation_time,
        per_gene,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        CellModel, SimulateCellOptions, SimulateReadsOptions, SimulateSequenceOptions,
        SimulationRng, simulate_cell_path, simulate_reads_fastq_path, simulate_sequence_fasta_path,
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

    #[test]
    fn cell_lineage_is_byte_reproducible() {
        let options = SimulateCellOptions {
            end_time: 240.0,
            bins: 12,
            seed: 7,
            ..SimulateCellOptions::default()
        };
        let first = scratch("cell-a.tsv");
        let second = scratch("cell-b.tsv");
        let summary_a = simulate_cell_path(&first, &options).expect("cell run");
        let summary_b = simulate_cell_path(&second, &options).expect("cell repeat");
        assert_eq!(summary_a, summary_b);
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        assert!(
            summary_a.division_count >= 2,
            "builtin panel should divide within 240 minutes"
        );
        assert!(summary_a.lineage_depth >= 1);
        let transcription_sum = summary_a
            .per_gene
            .iter()
            .map(|gene| gene.transcription_events)
            .sum::<u64>();
        assert_eq!(transcription_sum, summary_a.transcription_events);
        fs::remove_file(&first).unwrap();
        fs::remove_file(&second).unwrap();
    }

    #[test]
    fn starvation_stretches_generation_time() {
        let rich_options = SimulateCellOptions {
            end_time: 600.0,
            bins: 10,
            seed: 5,
            ..SimulateCellOptions::default()
        };
        let mut starved_model = CellModel::builtin();
        starved_model.substrate_initial = 120;
        let model_path = scratch("starved-model.json");
        fs::write(&model_path, serde_json::to_string(&starved_model).unwrap()).unwrap();
        let starved_options = SimulateCellOptions {
            model: Some(model_path.clone()),
            end_time: 600.0,
            bins: 10,
            seed: 5,
            ..SimulateCellOptions::default()
        };
        let rich_path = scratch("cell-rich.tsv");
        let starved_path = scratch("cell-starved.tsv");
        let rich = simulate_cell_path(&rich_path, &rich_options).expect("rich run");
        let starved = simulate_cell_path(&starved_path, &starved_options).expect("starved run");
        assert!(rich.mean_generation_time > 0.0);
        // Substrate depletion throttles translation (Monod), so the starved
        // lineage must complete far fewer divisions in the same window.
        assert!(
            starved.division_count * 3 < rich.division_count,
            "starved {} vs rich {} divisions",
            starved.division_count,
            rich.division_count
        );
        assert!(starved.final_substrate <= starved_model.substrate_initial / 2);
        fs::remove_file(&rich_path).unwrap();
        fs::remove_file(&starved_path).unwrap();
        fs::remove_file(&model_path).unwrap();
    }

    #[test]
    fn default_summary_matches_fixture() {
        // The committed fixture is the byte-level record of the default
        // builtin run (seed 42, end_time 120). First run generates it; every
        // later run — on any machine — must reproduce it exactly. Delete the
        // file only when the builtin model or defaults changed intentionally.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let fixture_path =
            manifest.join("../../../tests/fixtures/capability-results/simulate-cell.json");
        let out = scratch("cell-fixture.tsv");
        let summary = simulate_cell_path(&out, &SimulateCellOptions::default()).unwrap();
        let generated = serde_json::to_string_pretty(&summary).unwrap() + "\n";
        if fixture_path.exists() {
            let committed = fs::read_to_string(&fixture_path).unwrap();
            assert_eq!(
                committed, generated,
                "default cell summary drifted from the committed fixture"
            );
        } else {
            fs::write(&fixture_path, &generated).unwrap();
        }
        fs::remove_file(&out).unwrap();
    }

    #[test]
    fn cell_rejects_invalid_options_models_and_budgets() {
        let output = scratch("cell-bad.tsv");
        let error = simulate_cell_path(
            &output,
            &SimulateCellOptions {
                end_time: 0.0,
                ..SimulateCellOptions::default()
            },
        )
        .expect_err("zero end time rejected");
        assert!(error.to_string().contains("end time"));
        let error = simulate_cell_path(
            &output,
            &SimulateCellOptions {
                bins: 0,
                ..SimulateCellOptions::default()
            },
        )
        .expect_err("zero bins rejected");
        assert!(error.to_string().contains("bins"));
        let model_path = scratch("cell-bad-model.json");
        fs::write(
            &model_path,
            "{\"monod_constant\":1000.0,\"substrate_initial\":100,\"division_biomass\":10.0,\"genes\":[]}",
        )
        .unwrap();
        let error = simulate_cell_path(
            &output,
            &SimulateCellOptions {
                model: Some(model_path.clone()),
                ..SimulateCellOptions::default()
            },
        )
        .expect_err("empty gene panel rejected");
        assert!(error.to_string().contains("at least 1 gene"));
        let error = simulate_cell_path(
            &output,
            &SimulateCellOptions {
                max_events: 10,
                ..SimulateCellOptions::default()
            },
        )
        .expect_err("tiny event budget rejected");
        assert!(error.to_string().contains("event budget"));
        fs::remove_file(&model_path).unwrap();
        let _ = fs::remove_file(&output);
    }
}
