use crate::sequence_transform::{
    SequenceTransformError, reverse_complement_dna, visit_fasta_path, with_new_output,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const DEFAULT_KMER_SIZE: usize = 21;
pub const DEFAULT_KMER_TOP_N: usize = 50;
pub const MAX_KMER_SIZE: usize = 31;
pub const DEFAULT_EPCR_MAX_AMPLICON: usize = 5_000;
pub const DEFAULT_EPCR_MAX_HITS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KmerCountOptions {
    pub k: usize,
    pub canonical: bool,
    pub top_n: usize,
}

impl Default for KmerCountOptions {
    fn default() -> Self {
        Self {
            k: DEFAULT_KMER_SIZE,
            canonical: false,
            top_n: DEFAULT_KMER_TOP_N,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KmerCountEntry {
    pub kmer: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KmerCountSummary {
    pub sequence_count: u64,
    pub residue_count: u64,
    pub k: u64,
    pub canonical: bool,
    pub total_windows: u64,
    pub counted_windows: u64,
    pub skipped_ambiguous_windows: u64,
    pub distinct_kmers: u64,
    pub top_kmers: Vec<KmerCountEntry>,
}

pub fn count_kmers_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &KmerCountOptions,
) -> Result<KmerCountSummary, SequenceTransformError> {
    validate_kmer_options(options)?;
    let mut counts = HashMap::<u64, u64>::new();
    let mut sequence_count = 0_u64;
    let mut residue_count = 0_u64;
    let mut total_windows = 0_u64;
    let mut counted_windows = 0_u64;
    let mask = (1_u64 << (options.k * 2)) - 1;

    visit_fasta_path(input.as_ref(), |record| {
        sequence_count += 1;
        residue_count += u64::try_from(record.sequence.len()).expect("record length fits in u64");
        if record.sequence.len() >= options.k {
            total_windows += u64::try_from(record.sequence.len() - options.k + 1)
                .expect("window count fits in u64");
        }

        let mut encoded = 0_u64;
        let mut valid_run = 0_usize;
        for byte in &record.sequence {
            let Some(bits) = nucleotide_bits(*byte) else {
                encoded = 0;
                valid_run = 0;
                continue;
            };
            encoded = ((encoded << 2) | bits) & mask;
            valid_run += 1;
            if valid_run >= options.k {
                let key = if options.canonical {
                    encoded.min(reverse_complement_code(encoded, options.k))
                } else {
                    encoded
                };
                *counts.entry(key).or_default() += 1;
                counted_windows += 1;
            }
        }
        Ok(())
    })?;
    let skipped_ambiguous_windows = total_windows.saturating_sub(counted_windows);

    let mut entries = counts
        .into_iter()
        .map(|(encoded, count)| KmerCountEntry {
            kmer: decode_kmer(encoded, options.k),
            count,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.kmer.cmp(&right.kmer))
    });
    let distinct_kmers = u64::try_from(entries.len()).expect("k-mer count fits in u64");
    let top_kmers = entries
        .iter()
        .take(options.top_n)
        .cloned()
        .collect::<Vec<_>>();

    with_new_output(output.as_ref(), |writer| {
        writer.write_all(b"kmer\tcount\n")?;
        for entry in &entries {
            writeln!(writer, "{}\t{}", entry.kmer, entry.count)?;
        }
        Ok(())
    })?;

    Ok(KmerCountSummary {
        sequence_count,
        residue_count,
        k: u64::try_from(options.k).expect("k fits in u64"),
        canonical: options.canonical,
        total_windows,
        counted_windows,
        skipped_ambiguous_windows,
        distinct_kmers,
        top_kmers,
    })
}

fn validate_kmer_options(options: &KmerCountOptions) -> Result<(), SequenceTransformError> {
    if !(1..=MAX_KMER_SIZE).contains(&options.k) {
        return Err(SequenceTransformError::InvalidOption(format!(
            "k-mer size must be between 1 and {MAX_KMER_SIZE}"
        )));
    }
    if options.top_n == 0 {
        return Err(SequenceTransformError::InvalidOption(
            "top-N k-mer count must be at least one".to_owned(),
        ));
    }
    Ok(())
}

fn nucleotide_bits(byte: u8) -> Option<u64> {
    match byte.to_ascii_uppercase() {
        b'A' => Some(0),
        b'C' => Some(1),
        b'G' => Some(2),
        b'T' | b'U' => Some(3),
        _ => None,
    }
}

fn reverse_complement_code(mut code: u64, k: usize) -> u64 {
    let mut result = 0_u64;
    for _ in 0..k {
        result = (result << 2) | (3 - (code & 3));
        code >>= 2;
    }
    result
}

fn decode_kmer(mut code: u64, k: usize) -> String {
    let mut bytes = vec![b'A'; k];
    for index in (0..k).rev() {
        bytes[index] = match code & 3 {
            0 => b'A',
            1 => b'C',
            2 => b'G',
            _ => b'T',
        };
        code >>= 2;
    }
    String::from_utf8(bytes).expect("decoded k-mer is ASCII")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpcrOptions {
    pub min_amplicon: usize,
    pub max_amplicon: usize,
    pub max_hits: usize,
}

impl Default for EpcrOptions {
    fn default() -> Self {
        Self {
            min_amplicon: 1,
            max_amplicon: DEFAULT_EPCR_MAX_AMPLICON,
            max_hits: DEFAULT_EPCR_MAX_HITS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EpcrSummary {
    pub sequence_count: u64,
    pub primer_pair_count: u64,
    pub matched_primer_pair_count: u64,
    pub amplicon_count: u64,
    pub min_amplicon: u64,
    pub max_amplicon: u64,
}

#[derive(Debug, Clone)]
struct PrimerPair {
    id: String,
    forward: Vec<u8>,
    reverse_binding: Vec<u8>,
}

pub fn epcr_path(
    fasta: impl AsRef<Path>,
    primer_table: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &EpcrOptions,
) -> Result<EpcrSummary, SequenceTransformError> {
    if options.min_amplicon == 0 || options.min_amplicon > options.max_amplicon {
        return Err(SequenceTransformError::InvalidOption(
            "ePCR amplicon bounds require 1 <= min_amplicon <= max_amplicon".to_owned(),
        ));
    }
    if options.max_hits == 0 {
        return Err(SequenceTransformError::InvalidOption(
            "ePCR max_hits must be at least one".to_owned(),
        ));
    }
    let primers = read_primers(primer_table.as_ref())?;
    let mut rows = Vec::<String>::new();
    let mut matched = vec![false; primers.len()];
    let mut sequence_count = 0_u64;

    visit_fasta_path(fasta.as_ref(), |record| {
        sequence_count += 1;
        let sequence = normalize_epcr_reference_sequence(&record.sequence, &record.identifier)?;
        for (primer_index, primer) in primers.iter().enumerate() {
            let forward_hits = find_all(&sequence, &primer.forward);
            let reverse_hits = find_all(&sequence, &primer.reverse_binding);
            for forward_start in &forward_hits {
                for reverse_start in &reverse_hits {
                    if reverse_start < forward_start {
                        continue;
                    }
                    let end = reverse_start + primer.reverse_binding.len();
                    let length = end.saturating_sub(*forward_start);
                    if !(options.min_amplicon..=options.max_amplicon).contains(&length) {
                        continue;
                    }
                    if rows.len() == options.max_hits {
                        return Err(SequenceTransformError::InvalidOption(format!(
                            "ePCR exceeded the {}-hit safety limit",
                            options.max_hits
                        )));
                    }
                    matched[primer_index] = true;
                    rows.push(format!(
                        "{}\t{}\t{}\t{}\t{}\t+\n",
                        primer.id,
                        record.identifier,
                        forward_start + 1,
                        end,
                        length
                    ));
                }
            }
        }
        Ok(())
    })?;

    with_new_output(output.as_ref(), |writer| {
        writer.write_all(b"primer_id\tsequence_id\tstart\tend\tamplicon_length\tstrand\n")?;
        for row in &rows {
            writer.write_all(row.as_bytes())?;
        }
        Ok(())
    })?;

    Ok(EpcrSummary {
        sequence_count,
        primer_pair_count: u64::try_from(primers.len()).expect("primer count fits in u64"),
        matched_primer_pair_count: u64::try_from(matched.iter().filter(|value| **value).count())
            .expect("primer count fits in u64"),
        amplicon_count: u64::try_from(rows.len()).expect("hit count fits in u64"),
        min_amplicon: u64::try_from(options.min_amplicon).expect("length fits in u64"),
        max_amplicon: u64::try_from(options.max_amplicon).expect("length fits in u64"),
    })
}

fn read_primers(path: &Path) -> Result<Vec<PrimerPair>, SequenceTransformError> {
    let mut reader = csv::ReaderBuilder::new().delimiter(b'\t').from_path(path)?;
    let headers = reader.headers()?.clone();
    let id_index = header_index(&headers, "id")?;
    let forward_index = header_index(&headers, "forward")?;
    let reverse_index = header_index(&headers, "reverse")?;
    let mut primers = Vec::new();
    for (row_index, row) in reader.records().enumerate() {
        let row = row?;
        let id = row.get(id_index).unwrap_or_default().trim();
        if id.is_empty() {
            return Err(SequenceTransformError::InvalidOption(format!(
                "primer row {} has an empty id",
                row_index + 2
            )));
        }
        let forward =
            normalize_primer_sequence(row.get(forward_index).unwrap_or_default().as_bytes(), id)?;
        let reverse =
            normalize_primer_sequence(row.get(reverse_index).unwrap_or_default().as_bytes(), id)?;
        if forward.is_empty() || reverse.is_empty() {
            return Err(SequenceTransformError::InvalidOption(format!(
                "primer row {} requires non-empty forward and reverse sequences",
                row_index + 2
            )));
        }
        primers.push(PrimerPair {
            id: id.to_owned(),
            forward,
            reverse_binding: reverse_complement_dna(&reverse),
        });
    }
    if primers.is_empty() {
        return Err(SequenceTransformError::InvalidOption(
            "primer table contains no primer pairs".to_owned(),
        ));
    }
    Ok(primers)
}

fn header_index(headers: &csv::StringRecord, name: &str) -> Result<usize, SequenceTransformError> {
    headers
        .iter()
        .position(|header| header.trim().eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            SequenceTransformError::InvalidOption(format!(
                "primer table requires a {name:?} column"
            ))
        })
}

fn normalize_primer_sequence(
    sequence: &[u8],
    identifier: &str,
) -> Result<Vec<u8>, SequenceTransformError> {
    sequence
        .iter()
        .copied()
        .enumerate()
        .map(|(index, byte)| match byte.to_ascii_uppercase() {
            b'A' | b'C' | b'G' | b'T' => Ok(byte.to_ascii_uppercase()),
            b'U' => Ok(b'T'),
            symbol => Err(SequenceTransformError::InvalidNucleotide {
                identifier: identifier.to_owned(),
                position: index + 1,
                symbol,
            }),
        })
        .collect()
}

fn normalize_epcr_reference_sequence(
    sequence: &[u8],
    identifier: &str,
) -> Result<Vec<u8>, SequenceTransformError> {
    sequence
        .iter()
        .copied()
        .enumerate()
        .map(|(index, byte)| match byte.to_ascii_uppercase() {
            b'A' | b'C' | b'G' | b'T' | b'N' => Ok(byte.to_ascii_uppercase()),
            b'U' => Ok(b'T'),
            symbol => Err(SequenceTransformError::InvalidNucleotide {
                identifier: identifier.to_owned(),
                position: index + 1,
                symbol,
            }),
        })
        .collect()
}

fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.len() > haystack.len() {
        return Vec::new();
    }
    haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(index, window)| (window == needle).then_some(index))
        .collect()
}

// ── Consensus sequence ──────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SequenceAnalysisError {
    Io(io::Error),
    ReadLine { line: usize, source: io::Error },
    MalformedRecord { line: usize, message: String },
    OutputAlreadyExists(PathBuf),
    LimitExceeded { resource: &'static str, limit: u64 },
    InvalidOption(String),
    AlignmentError(String),
}

impl Display for SequenceAnalysisError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "sequence analysis failed: {error}"),
            Self::ReadLine { line, source } => {
                write!(
                    formatter,
                    "sequence analysis read error at line {line}: {source}"
                )
            }
            Self::MalformedRecord { line, message } => {
                write!(
                    formatter,
                    "malformed alignment record at line {line}: {message}"
                )
            }
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::LimitExceeded { resource, limit } => write!(
                formatter,
                "{resource} exceeds the {limit}-count safety limit"
            ),
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::AlignmentError(message) => formatter.write_str(message),
        }
    }
}

impl Error for SequenceAnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::ReadLine { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for SequenceAnalysisError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<SequenceTransformError> for SequenceAnalysisError {
    fn from(error: SequenceTransformError) -> Self {
        match error {
            SequenceTransformError::Io(error) => Self::Io(error),
            SequenceTransformError::OutputAlreadyExists(path) => Self::OutputAlreadyExists(path),
            SequenceTransformError::InvalidOption(message) => Self::InvalidOption(message),
            other => Self::AlignmentError(other.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConsensusOptions {
    pub threshold: f64,
}

impl Default for ConsensusOptions {
    fn default() -> Self {
        Self { threshold: 0.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConsensusResult {
    pub input_sequence_count: u64,
    pub alignment_length: u64,
    pub consensus_length: u64,
    pub ambiguous_position_count: u64,
    pub gc_content: f64,
    pub warnings: Vec<String>,
}

static CONSENSUS_MAX_SEQUENCES: u64 = 1_000_000;

pub fn consensus_from_alignment_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ConsensusOptions,
) -> Result<ConsensusResult, SequenceAnalysisError> {
    if !options.threshold.is_finite() || !(0.0..=1.0).contains(&options.threshold) {
        return Err(SequenceAnalysisError::InvalidOption(format!(
            "consensus threshold must be between 0.0 and 1.0, got {}",
            options.threshold
        )));
    }

    let mut sequences: Vec<Vec<u8>> = Vec::new();
    let mut alignment_length: Option<usize> = None;
    let mut seq_count = 0_u64;

    visit_fasta_path(input.as_ref(), |record| {
        seq_count += 1;
        if seq_count > CONSENSUS_MAX_SEQUENCES {
            return Err(SequenceTransformError::InvalidOption(format!(
                "consensus input exceeds the {CONSENSUS_MAX_SEQUENCES}-sequence safety limit"
            )));
        }
        let normalized =
            normalize_consensus_sequence(&record.sequence, &record.identifier, seq_count as usize)?;
        let len = normalized.len();
        match alignment_length {
            None => alignment_length = Some(len),
            Some(expected) if len != expected => {
                return Err(SequenceTransformError::InvalidOption(format!(
                    "alignment length mismatch: record {:?} has {} positions, expected {}",
                    record.identifier, len, expected
                )));
            }
            _ => {}
        }
        sequences.push(normalized);
        Ok(())
    })?;

    if seq_count == 0 {
        return Err(SequenceAnalysisError::AlignmentError(
            "alignment FASTA contains no records".to_owned(),
        ));
    }
    let alignment_length =
        u64::try_from(alignment_length.unwrap_or(0)).expect("alignment length fits in u64");

    let mut consensus = Vec::with_capacity(alignment_length as usize);
    let mut ambiguous = 0_u64;
    let mut gc = 0_u64;
    let mut warnings = Vec::new();

    if seq_count == 1 {
        warnings.push(
            "consensus computed from a single sequence — no column-level variation is present"
                .to_owned(),
        );
    }

    let threshold_count = (seq_count as f64 * options.threshold).ceil() as u64;

    for column in 0..(alignment_length as usize) {
        let mut counts = [0_u64; 6]; // A, C, G, T, U, N
        for seq in &sequences {
            match seq[column] {
                b'A' => counts[0] += 1,
                b'C' => counts[1] += 1,
                b'G' => counts[2] += 1,
                b'T' => counts[3] += 1,
                b'U' => counts[4] += 1,
                b'N' => counts[5] += 1,
                _ => {} // skip gaps
            }
        }
        let total = counts.iter().sum::<u64>();
        if total == 0 {
            // all-gap column — skip
            continue;
        }
        let (max_index, max_count) = counts.iter().enumerate().max_by_key(|&(_, c)| c).unwrap();
        let base = match max_index {
            0 => b'A',
            1 => b'C',
            2 => b'G',
            3 => b'T',
            4 => b'U',
            _ => b'N',
        };
        if *max_count >= threshold_count {
            consensus.push(base);
            if base == b'G' || base == b'C' {
                gc += 1;
            }
        } else {
            consensus.push(b'N');
            ambiguous += 1;
        }
    }

    let consensus_length = u64::try_from(consensus.len()).expect("consensus length fits in u64");
    let gc_content = if consensus_length == 0 {
        0.0
    } else {
        gc as f64 * 100.0 / consensus_length as f64
    };

    with_new_output(output.as_ref(), |writer| {
        writer.write_all(b">consensus\n")?;
        for chunk in consensus.chunks(80) {
            writer.write_all(chunk)?;
            writer.write_all(b"\n")?;
        }
        Ok(())
    })?;

    Ok(ConsensusResult {
        input_sequence_count: seq_count,
        alignment_length,
        consensus_length,
        ambiguous_position_count: ambiguous,
        gc_content,
        warnings,
    })
}

fn normalize_consensus_sequence(
    sequence: &[u8],
    identifier: &str,
    _ordinal: usize,
) -> Result<Vec<u8>, SequenceTransformError> {
    sequence
        .iter()
        .copied()
        .enumerate()
        .map(|(index, byte)| match byte.to_ascii_uppercase() {
            b'A' | b'C' | b'G' | b'T' | b'U' | b'N' | b'-' | b'.' => Ok(byte.to_ascii_uppercase()),
            b'R' | b'Y' | b'S' | b'W' | b'K' | b'M' | b'B' | b'D' | b'H' | b'V' => {
                Ok(byte.to_ascii_uppercase())
            }
            symbol => Err(SequenceTransformError::InvalidNucleotide {
                identifier: identifier.to_owned(),
                position: index + 1,
                symbol,
            }),
        })
        .collect()
}

// ── Sequence shuffle ────────────────────────────────────────────────────────

pub const DEFAULT_SHUFFLE_SEED: u64 = 42;

#[derive(Debug, Clone)]
pub struct ShuffleOptions {
    pub seed: u64,
}

impl Default for ShuffleOptions {
    fn default() -> Self {
        Self {
            seed: DEFAULT_SHUFFLE_SEED,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShuffleResult {
    pub sequence_count: u64,
    pub residue_count: u64,
    pub seed: u64,
}

pub fn shuffle_sequences_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ShuffleOptions,
) -> Result<ShuffleResult, SequenceAnalysisError> {
    let mut rng = StdRng::seed_from_u64(options.seed);

    // Read all sequences into memory
    let mut records: Vec<(String, Vec<u8>)> = Vec::new();
    let mut residue_count = 0_u64;

    visit_fasta_path(input.as_ref(), |record| {
        residue_count += u64::try_from(record.sequence.len()).expect("sequence length fits in u64");
        records.push((record.header.to_owned(), record.sequence.to_vec()));
        Ok(())
    })?;

    if records.is_empty() {
        return Err(SequenceAnalysisError::AlignmentError(
            "input FASTA contains no records".to_owned(),
        ));
    }

    let sequence_count = u64::try_from(records.len()).expect("record count fits in u64");

    // Fisher-Yates shuffle
    for i in (1..records.len()).rev() {
        let j = rng.gen_range(0..=i);
        records.swap(i, j);
    }

    with_new_output(output.as_ref(), |writer| {
        for (header, seq) in &records {
            writeln!(writer, ">{header}")?;
            for chunk in seq.chunks(80) {
                writer.write_all(chunk)?;
                writer.write_all(b"\n")?;
            }
        }
        Ok(())
    })?;

    Ok(ShuffleResult {
        sequence_count,
        residue_count,
        seed: options.seed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("linxira-{nonce}-{name}"))
    }

    #[test]
    fn counts_exact_and_canonical_kmers() {
        let input = temp_path("kmers.fa");
        let output = temp_path("kmers.tsv");
        fs::write(&input, ">a\nACGTAC\n>b\nGTNAC\n").unwrap();
        let summary = count_kmers_path(
            &input,
            &output,
            &KmerCountOptions {
                k: 3,
                canonical: true,
                top_n: 10,
            },
        )
        .unwrap();
        assert_eq!(summary.sequence_count, 2);
        assert_eq!(summary.total_windows, 7);
        assert_eq!(summary.counted_windows, 4);
        assert_eq!(summary.skipped_ambiguous_windows, 3);
        assert!(
            fs::read_to_string(&output)
                .unwrap()
                .starts_with("kmer\tcount\n")
        );
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn finds_simple_epcr_amplicons() {
        let fasta = temp_path("epcr.fa");
        let primers = temp_path("primers.tsv");
        let output = temp_path("epcr.tsv");
        fs::write(&fasta, ">chr1\nAAACCCGGGTTT\n").unwrap();
        fs::write(&primers, "id\tforward\treverse\np1\tAAA\tAAA\n").unwrap();
        let summary = epcr_path(&fasta, &primers, &output, &EpcrOptions::default()).unwrap();
        assert_eq!(summary.amplicon_count, 1);
        assert!(
            fs::read_to_string(&output)
                .unwrap()
                .contains("p1\tchr1\t1\t12\t12\t+")
        );
        let _ = fs::remove_file(fasta);
        let _ = fs::remove_file(primers);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn computes_consensus_with_threshold() {
        let input = temp_path("consensus.fa");
        let output = temp_path("consensus-out.fa");
        fs::write(
            &input,
            ">seq1\nACGT-A\n>seq2\nACGTGA\n>seq3\nAC-T-A\n>seq4\nAC-T-A\n",
        )
        .unwrap();
        let result =
            consensus_from_alignment_path(&input, &output, &ConsensusOptions { threshold: 0.5 })
                .unwrap();

        assert_eq!(result.input_sequence_count, 4);
        assert_eq!(result.alignment_length, 6);
        assert_eq!(result.consensus_length, 6);
        assert_eq!(result.ambiguous_position_count, 1);
        let out = fs::read_to_string(&output).unwrap();
        assert!(out.starts_with(">consensus\n"));
        assert!(out.contains("ACGTNA\n"));
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn consensus_high_threshold_marks_more_ambiguous() {
        let input = temp_path("consensus-strict.fa");
        let output = temp_path("consensus-strict-out.fa");
        fs::write(&input, ">seq1\nACGT\n>seq2\nACGA\n>seq3\nACGT\n").unwrap();
        let result =
            consensus_from_alignment_path(&input, &output, &ConsensusOptions { threshold: 0.9 })
                .unwrap();

        // Position 4: T=2, A=1 so max is 2/3 = 0.667 < 0.9 → N
        assert_eq!(result.ambiguous_position_count, 1);
        let out = fs::read_to_string(&output).unwrap();
        assert!(out.contains("ACGN\n"));
        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
    }

    #[test]
    fn shuffles_sequences_deterministically() {
        let input = temp_path("shuffle.fa");
        let output = temp_path("shuffle-out.fa");
        fs::write(&input, ">a\nAAAA\n>b\nCCCC\n>c\nGGGG\n>d\nTTTT\n").unwrap();

        let result = shuffle_sequences_path(&input, &output, &ShuffleOptions { seed: 42 }).unwrap();
        assert_eq!(result.sequence_count, 4);
        assert_eq!(result.residue_count, 16);

        // Same seed should produce the same output
        let output2 = temp_path("shuffle-out2.fa");
        let _result2 =
            shuffle_sequences_path(&input, &output2, &ShuffleOptions { seed: 42 }).unwrap();
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            fs::read_to_string(&output2).unwrap()
        );

        // Different seed should produce different order
        let output3 = temp_path("shuffle-out3.fa");
        shuffle_sequences_path(&input, &output3, &ShuffleOptions { seed: 99 }).unwrap();
        assert_ne!(
            fs::read_to_string(&output).unwrap(),
            fs::read_to_string(&output3).unwrap()
        );

        let _ = fs::remove_file(input);
        let _ = fs::remove_file(output);
        let _ = fs::remove_file(output2);
        let _ = fs::remove_file(output3);
    }
}
