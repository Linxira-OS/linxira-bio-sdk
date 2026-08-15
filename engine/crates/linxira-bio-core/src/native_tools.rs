use serde::Serialize;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_THREADS: usize = 1_024;
const MAX_STDERR_BYTES: usize = 64 * 1_024;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlastProgram {
    Blastn,
    Blastp,
    Blastx,
    Tblastn,
    Tblastx,
}

impl BlastProgram {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blastn => "blastn",
            Self::Blastp => "blastp",
            Self::Blastx => "blastx",
            Self::Tblastn => "tblastn",
            Self::Tblastx => "tblastx",
        }
    }

    fn database_type(self) -> &'static str {
        match self {
            Self::Blastn | Self::Tblastn | Self::Tblastx => "nucl",
            Self::Blastp | Self::Blastx => "prot",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiamondMode {
    Blastp,
    Blastx,
}

impl DiamondMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blastp => "blastp",
            Self::Blastx => "blastx",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HmmerMode {
    Hmmsearch,
    Hmmscan,
}

impl HmmerMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hmmsearch => "hmmsearch",
            Self::Hmmscan => "hmmscan",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuscleMode {
    Align,
    Super5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimalMode {
    Automated1,
    Gappyout,
    Strict,
    Strictplus,
    Nogaps,
}

impl TrimalMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automated1 => "automated1",
            Self::Gappyout => "gappyout",
            Self::Strict => "strict",
            Self::Strictplus => "strictplus",
            Self::Nogaps => "nogaps",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemeAlphabet {
    Dna,
    Rna,
    Protein,
}

impl MemeAlphabet {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dna => "dna",
            Self::Rna => "rna",
            Self::Protein => "protein",
        }
    }
}

impl MuscleMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Align => "align",
            Self::Super5 => "super5",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimilaritySearchOptions {
    pub threads: usize,
    pub evalue: f64,
    pub max_target_sequences: usize,
    pub outfmt: u8,
}

impl Default for SimilaritySearchOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            evalue: 1e-3,
            max_target_sequences: 50,
            outfmt: 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HmmerOptions {
    pub threads: usize,
    pub evalue: f64,
}

impl Default for HmmerOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            evalue: 10.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MuscleOptions {
    pub threads: usize,
    pub mode: MuscleMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IqtreeOptions {
    pub threads: usize,
    pub model: String,
    pub seed: u64,
}

impl Default for IqtreeOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            model: "MFP".to_owned(),
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemeOptions {
    pub threads: usize,
    pub alphabet: MemeAlphabet,
    pub distribution: String,
    pub motif_count: usize,
    pub minimum_width: usize,
    pub maximum_width: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortReadAlignmentOptions {
    pub threads: usize,
}

impl Default for ShortReadAlignmentOptions {
    fn default() -> Self {
        Self { threads: 1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Minimap2Preset {
    MapOnt,
    MapPb,
    MapHifi,
    Splice,
    Asm5,
    Asm10,
    Asm20,
    Sr,
}

impl Minimap2Preset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MapOnt => "map-ont",
            Self::MapPb => "map-pb",
            Self::MapHifi => "map-hifi",
            Self::Splice => "splice",
            Self::Asm5 => "asm5",
            Self::Asm10 => "asm10",
            Self::Asm20 => "asm20",
            Self::Sr => "sr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Minimap2LongReadOptions {
    pub preset: Minimap2Preset,
    pub threads: usize,
    pub secondary: bool,
    pub max_secondary: usize,
}

impl Default for Minimap2LongReadOptions {
    fn default() -> Self {
        Self {
            preset: Minimap2Preset::MapOnt,
            threads: 1,
            secondary: false,
            max_secondary: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnpEffOptions {
    pub database: String,
    pub upstream_downstream: Option<usize>,
    pub no_stats: bool,
    pub no_log: bool,
}

impl Default for SnpEffOptions {
    fn default() -> Self {
        Self {
            database: "GRCh38.99".to_owned(),
            upstream_downstream: None,
            no_stats: false,
            no_log: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MastOptions {
    pub threads: usize,
    pub evalue: f64,
    pub hit_list: bool,
    pub add_self_compat: bool,
}

impl Default for MastOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            evalue: 1e-5,
            hit_list: false,
            add_self_compat: false,
        }
    }
}

impl Default for MemeOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            alphabet: MemeAlphabet::Dna,
            distribution: "zoops".to_owned(),
            motif_count: 3,
            minimum_width: 6,
            maximum_width: 15,
        }
    }
}

impl Default for MuscleOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            mode: MuscleMode::Align,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NativeToolResult {
    pub tool: String,
    pub mode: String,
    pub output_path: String,
    pub output_bytes: u64,
    pub thread_count: usize,
    pub command_count: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum NativeToolError {
    InvalidOption(String),
    MissingInput(PathBuf),
    OutputAlreadyExists(PathBuf),
    InputEqualsOutput(PathBuf),
    Io(std::io::Error),
    Spawn {
        tool: String,
        source: std::io::Error,
    },
    Failed {
        tool: String,
        status: Option<i32>,
        stderr: String,
    },
    MissingOutput {
        tool: String,
        path: PathBuf,
    },
}

impl Display for NativeToolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOption(message) => formatter.write_str(message),
            Self::MissingInput(path) => {
                write!(formatter, "input file does not exist: {}", path.display())
            }
            Self::OutputAlreadyExists(path) => write!(
                formatter,
                "refusing to overwrite existing output: {}",
                path.display()
            ),
            Self::InputEqualsOutput(path) => write!(
                formatter,
                "input and output resolve to the same path: {}",
                path.display()
            ),
            Self::Io(error) => write!(formatter, "native workflow I/O failed: {error}"),
            Self::Spawn { tool, source } => write!(
                formatter,
                "failed to start {tool}; install or configure the required executable: {source}"
            ),
            Self::Failed {
                tool,
                status,
                stderr,
            } => write!(
                formatter,
                "{tool} exited with status {}: {stderr}",
                status.map_or_else(|| "unknown".to_owned(), |value| value.to_string())
            ),
            Self::MissingOutput { tool, path } => write!(
                formatter,
                "{tool} reported success but did not create {}",
                path.display()
            ),
        }
    }
}

impl Error for NativeToolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Spawn { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for NativeToolError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub fn parse_blast_program(value: &str) -> Result<BlastProgram, NativeToolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "blastn" => Ok(BlastProgram::Blastn),
        "blastp" => Ok(BlastProgram::Blastp),
        "blastx" => Ok(BlastProgram::Blastx),
        "tblastn" => Ok(BlastProgram::Tblastn),
        "tblastx" => Ok(BlastProgram::Tblastx),
        _ => Err(NativeToolError::InvalidOption(format!(
            "unsupported BLAST program: {value}"
        ))),
    }
}

pub fn parse_diamond_mode(value: &str) -> Result<DiamondMode, NativeToolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "blastp" => Ok(DiamondMode::Blastp),
        "blastx" => Ok(DiamondMode::Blastx),
        _ => Err(NativeToolError::InvalidOption(format!(
            "unsupported DIAMOND mode: {value}"
        ))),
    }
}

pub fn parse_hmmer_mode(value: &str) -> Result<HmmerMode, NativeToolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "hmmsearch" => Ok(HmmerMode::Hmmsearch),
        "hmmscan" => Ok(HmmerMode::Hmmscan),
        _ => Err(NativeToolError::InvalidOption(format!(
            "unsupported HMMER mode: {value}"
        ))),
    }
}

pub fn parse_muscle_mode(value: &str) -> Result<MuscleMode, NativeToolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "align" => Ok(MuscleMode::Align),
        "super5" => Ok(MuscleMode::Super5),
        _ => Err(NativeToolError::InvalidOption(format!(
            "unsupported MUSCLE mode: {value}"
        ))),
    }
}

pub fn parse_trimal_mode(value: &str) -> Result<TrimalMode, NativeToolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "automated1" => Ok(TrimalMode::Automated1),
        "gappyout" => Ok(TrimalMode::Gappyout),
        "strict" => Ok(TrimalMode::Strict),
        "strictplus" => Ok(TrimalMode::Strictplus),
        "nogaps" => Ok(TrimalMode::Nogaps),
        _ => Err(NativeToolError::InvalidOption(format!(
            "unsupported trimAl mode: {value}"
        ))),
    }
}

pub fn parse_meme_alphabet(value: &str) -> Result<MemeAlphabet, NativeToolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "dna" => Ok(MemeAlphabet::Dna),
        "rna" => Ok(MemeAlphabet::Rna),
        "protein" => Ok(MemeAlphabet::Protein),
        _ => Err(NativeToolError::InvalidOption(format!(
            "unsupported MEME alphabet: {value}"
        ))),
    }
}

pub fn run_blast_fasta_path(
    query: impl AsRef<Path>,
    reference: impl AsRef<Path>,
    output: impl AsRef<Path>,
    program: BlastProgram,
    options: &SimilaritySearchOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_similarity_options(options)?;
    let query = query.as_ref();
    let reference = reference.as_ref();
    let output = output.as_ref();
    validate_paths(&[query, reference], output)?;
    let temporary = create_temporary_directory(output, "blast-db")?;
    let database = temporary.join("reference");
    let result = (|| {
        let makeblastdb = configured_program("LINXIRA_BIO_MAKEBLASTDB", "makeblastdb");
        let make_args = vec![
            OsString::from("-in"),
            reference.as_os_str().to_owned(),
            OsString::from("-dbtype"),
            OsString::from(program.database_type()),
            OsString::from("-out"),
            database.as_os_str().to_owned(),
            OsString::from("-parse_seqids"),
        ];
        run_native_command(&makeblastdb, &make_args, true)?;

        let executable = configured_program(
            &format!("LINXIRA_BIO_{}", program.as_str().to_ascii_uppercase()),
            program.as_str(),
        );
        let search_args = blast_arguments(query, &database, output, options);
        run_native_command(&executable, &search_args, true)?;
        finish_result("ncbi-blast", program.as_str(), output, options.threads, 2)
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_diamond_fasta_path(
    query: impl AsRef<Path>,
    reference: impl AsRef<Path>,
    output: impl AsRef<Path>,
    mode: DiamondMode,
    options: &SimilaritySearchOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_similarity_options(options)?;
    let query = query.as_ref();
    let reference = reference.as_ref();
    let output = output.as_ref();
    validate_paths(&[query, reference], output)?;
    let temporary = create_temporary_directory(output, "diamond-db")?;
    let database = temporary.join("reference");
    let result = (|| {
        let diamond = configured_program("LINXIRA_BIO_DIAMOND", "diamond");
        let database_args = vec![
            OsString::from("makedb"),
            OsString::from("--in"),
            reference.as_os_str().to_owned(),
            OsString::from("--db"),
            database.as_os_str().to_owned(),
            OsString::from("--threads"),
            OsString::from(options.threads.to_string()),
        ];
        run_native_command(&diamond, &database_args, false)?;
        let search_args = diamond_arguments(query, &database, output, mode, options);
        run_native_command(&diamond, &search_args, false)?;
        finish_result("diamond", mode.as_str(), output, options.threads, 2)
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_hmmer_path(
    profile: impl AsRef<Path>,
    sequences: impl AsRef<Path>,
    output: impl AsRef<Path>,
    mode: HmmerMode,
    options: &HmmerOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(options.threads)?;
    validate_evalue(options.evalue)?;
    let profile = profile.as_ref();
    let sequences = sequences.as_ref();
    let output = output.as_ref();
    validate_paths(&[profile, sequences], output)?;
    let executable = configured_program(
        &format!("LINXIRA_BIO_{}", mode.as_str().to_ascii_uppercase()),
        mode.as_str(),
    );
    let arguments = hmmer_arguments(profile, sequences, output, options);
    let result = run_native_command(&executable, &arguments, false)
        .and_then(|_| finish_result("hmmer", mode.as_str(), output, options.threads, 1));
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_muscle_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &MuscleOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(options.threads)?;
    let input = input.as_ref();
    let output = output.as_ref();
    validate_paths(&[input], output)?;
    let executable = configured_program("LINXIRA_BIO_MUSCLE", "muscle");
    let arguments = muscle_arguments(input, output, options);
    let result = run_native_command(&executable, &arguments, false)
        .and_then(|_| finish_result("muscle", options.mode.as_str(), output, options.threads, 1));
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_trimal_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    mode: TrimalMode,
) -> Result<NativeToolResult, NativeToolError> {
    let input = input.as_ref();
    let output = output.as_ref();
    validate_paths(&[input], output)?;
    let executable = configured_program("LINXIRA_BIO_TRIMAL", "trimal");
    let arguments = trimal_arguments(input, output, mode);
    let result = run_native_command(&executable, &arguments, false)
        .and_then(|_| finish_result("trimal", mode.as_str(), output, 1, 1));
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_iqtree_path(
    alignment: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &IqtreeOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(options.threads)?;
    if options.model.trim().is_empty() || options.model.len() > 256 {
        return Err(NativeToolError::InvalidOption(
            "IQ-TREE model must contain 1 to 256 characters".to_owned(),
        ));
    }
    let alignment = alignment.as_ref();
    let output = output.as_ref();
    validate_paths(&[alignment], output)?;
    let temporary = create_temporary_directory(output, "iqtree")?;
    let prefix = temporary.join("analysis");
    let generated = PathBuf::from(format!("{}.treefile", prefix.to_string_lossy()));
    let result = (|| {
        let executable = configured_program("LINXIRA_BIO_IQTREE", "iqtree2");
        let arguments = iqtree_arguments(alignment, &prefix, options);
        run_native_command(&executable, &arguments, false)?;
        copy_generated_output("iqtree", &generated, output)?;
        finish_result("iqtree", &options.model, output, options.threads, 1)
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_mcscanx_path(
    gene_positions: impl AsRef<Path>,
    similarity_hits: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<NativeToolResult, NativeToolError> {
    let gene_positions = gene_positions.as_ref();
    let similarity_hits = similarity_hits.as_ref();
    let output = output.as_ref();
    validate_paths(&[gene_positions, similarity_hits], output)?;
    let temporary = create_temporary_directory(output, "mcscanx")?;
    let dataset = temporary.join("dataset");
    let gff = temporary.join("dataset.gff");
    let blast = temporary.join("dataset.blast");
    let generated = temporary.join("dataset.collinearity");
    let result = (|| {
        fs::copy(gene_positions, &gff)?;
        fs::copy(similarity_hits, &blast)?;
        let executable = configured_program("LINXIRA_BIO_MCSCANX", "MCScanX");
        run_native_command(&executable, &mcscanx_arguments(&dataset), false)?;
        copy_generated_output("MCScanX", &generated, output)?;
        finish_result("mcscanx", "collinearity", output, 1, 2)
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_kaks_path(
    codon_alignment: impl AsRef<Path>,
    output: impl AsRef<Path>,
    method: &str,
) -> Result<NativeToolResult, NativeToolError> {
    let method = parse_kaks_method(method)?;
    let codon_alignment = codon_alignment.as_ref();
    let output = output.as_ref();
    validate_paths(&[codon_alignment], output)?;
    let executable = configured_program("LINXIRA_BIO_KAKS_CALCULATOR", "KaKs_Calculator");
    let result = run_native_command(
        &executable,
        &kaks_arguments(codon_alignment, output, method),
        false,
    )
    .and_then(|_| finish_result("kaks-calculator", method, output, 1, 1));
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_meme_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &MemeOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(options.threads)?;
    if !matches!(options.distribution.as_str(), "oops" | "zoops" | "anr") {
        return Err(NativeToolError::InvalidOption(
            "MEME distribution must be oops, zoops, or anr".to_owned(),
        ));
    }
    if options.motif_count == 0 || options.motif_count > 1_000 {
        return Err(NativeToolError::InvalidOption(
            "MEME motif_count must be between 1 and 1000".to_owned(),
        ));
    }
    if options.minimum_width < 2
        || options.maximum_width < options.minimum_width
        || options.maximum_width > 1_000
    {
        return Err(NativeToolError::InvalidOption(
            "MEME widths require 2 <= minimum_width <= maximum_width <= 1000".to_owned(),
        ));
    }
    let input = input.as_ref();
    let output = output.as_ref();
    validate_paths(&[input], output)?;
    let temporary = create_temporary_directory(output, "meme")?;
    let generated = temporary.join("meme.txt");
    let result = (|| {
        let executable = configured_program("LINXIRA_BIO_MEME", "meme");
        let arguments = meme_arguments(input, &temporary, options);
        run_native_command(&executable, &arguments, false)?;
        copy_generated_output("meme", &generated, output)?;
        finish_result(
            "meme",
            options.alphabet.as_str(),
            output,
            options.threads,
            1,
        )
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_dssp_path(
    structure: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<NativeToolResult, NativeToolError> {
    let structure = structure.as_ref();
    let output = output.as_ref();
    validate_paths(&[structure], output)?;
    let executable = configured_program("LINXIRA_BIO_MKDSSP", "mkdssp");
    let arguments = dssp_arguments(structure, output);
    let result = run_native_command(&executable, &arguments, false)
        .and_then(|_| finish_result("mkdssp", "secondary-structure", output, 1, 1));
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

/// Write a reproducible `samtools stats` or `samtools coverage` report.
///
/// The report is deliberately retained as the native tool's tabular output so
/// downstream software can use every metric supported by its installed version.
pub fn run_samtools_report_path(
    input: impl AsRef<Path>,
    reference: Option<&Path>,
    output: impl AsRef<Path>,
    mode: &str,
) -> Result<NativeToolResult, NativeToolError> {
    if !matches!(mode, "stats" | "coverage") {
        return Err(NativeToolError::InvalidOption(
            "samtools report mode must be stats or coverage".to_owned(),
        ));
    }
    let input = input.as_ref();
    let output = output.as_ref();
    let mut inputs = vec![input];
    if let Some(reference) = reference {
        inputs.push(reference);
    }
    validate_paths(&inputs, output)?;
    let executable = configured_program("LINXIRA_BIO_SAMTOOLS", "samtools");
    let arguments = samtools_report_arguments(input, reference, mode);
    let result = (|| {
        let native_output = run_native_command(&executable, &arguments, false)?;
        fs::write(output, native_output.stdout)?;
        finish_result("samtools", mode, output, 1, 1)
    })();
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

/// Convert an indexed BAM/CRAM alignment to a BigWig coverage track with deepTools.
pub fn run_bam_to_bigwig_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    threads: usize,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(threads)?;
    let input = input.as_ref();
    let output = output.as_ref();
    validate_paths(&[input], output)?;
    let executable = configured_program("LINXIRA_BIO_BAMCOVERAGE", "bamCoverage");
    let arguments = bam_coverage_arguments(input, output, threads);
    let result = run_native_command(&executable, &arguments, false)
        .and_then(|_| finish_result("bamCoverage", "bam-to-bigwig", output, 1, 1));
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

/// Align long reads (PacBio, ONT) to a reference with minimap2.
pub fn run_minimap2_long_read_path(
    reference: impl AsRef<Path>,
    reads: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &Minimap2LongReadOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(options.threads)?;
    let reference = reference.as_ref();
    let reads = reads.as_ref();
    let output = output.as_ref();
    validate_paths(&[reference, reads], output)?;
    let executable = configured_program("LINXIRA_BIO_MINIMAP2", "minimap2");
    let arguments = minimap2_long_read_arguments(reference, reads, output, options);
    let result = run_native_command(&executable, &arguments, false)
        .and_then(|_| finish_result("minimap2", "long-read", output, options.threads, 1));
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

/// Annotate variants with snpEff.
pub fn run_snpeff_path(
    vcf: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &SnpEffOptions,
) -> Result<NativeToolResult, NativeToolError> {
    let vcf = vcf.as_ref();
    let output = output.as_ref();
    validate_paths(&[vcf], output)?;
    let executable = configured_program("LINXIRA_BIO_SNPEFF", "snpEff");
    let arguments = snpeff_arguments(vcf, options);
    let result = (|| {
        let native_output = run_native_command(&executable, &arguments, false)?;
        fs::write(output, native_output.stdout)?;
        finish_result("snpEff", "annotate", output, 1, 1)
    })();
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_mast_path(
    motif: impl AsRef<Path>,
    sequences: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &MastOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(options.threads)?;
    let motif = motif.as_ref();
    let sequences = sequences.as_ref();
    let output = output.as_ref();
    validate_paths(&[motif, sequences], output)?;
    let temporary = create_temporary_directory(output, "mast")?;
    let generated = temporary.join("mast.txt");
    let result = (|| {
        let executable = configured_program("LINXIRA_BIO_MAST", "mast");
        let arguments = mast_arguments(motif, sequences, &temporary, options);
        run_native_command(&executable, &arguments, false)?;
        copy_generated_output("mast", &generated, output)?;
        finish_result("mast", "motif-scan", output, options.threads, 1)
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn run_short_read_alignment_path(
    reference: impl AsRef<Path>,
    reads: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ShortReadAlignmentOptions,
) -> Result<NativeToolResult, NativeToolError> {
    validate_threads(options.threads)?;
    let reference = reference.as_ref();
    let reads = reads.as_ref();
    let output = output.as_ref();
    validate_paths(&[reference, reads], output)?;
    let temporary = create_temporary_directory(output, "short-read-alignment")?;
    let sam = temporary.join("alignment.sam");
    let result = (|| {
        let minimap2 = configured_program("LINXIRA_BIO_MINIMAP2", "minimap2");
        run_native_command(
            &minimap2,
            &minimap2_short_read_arguments(reference, reads, &sam, options),
            false,
        )?;
        let samtools = configured_program("LINXIRA_BIO_SAMTOOLS", "samtools");
        run_native_command(
            &samtools,
            &samtools_sort_arguments(&sam, output, options),
            false,
        )?;
        finish_result(
            "minimap2-samtools",
            "short-read",
            output,
            options.threads,
            2,
        )
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn blast_arguments(
    query: &Path,
    database: &Path,
    output: &Path,
    options: &SimilaritySearchOptions,
) -> Vec<OsString> {
    vec![
        OsString::from("-query"),
        query.as_os_str().to_owned(),
        OsString::from("-db"),
        database.as_os_str().to_owned(),
        OsString::from("-out"),
        output.as_os_str().to_owned(),
        OsString::from("-outfmt"),
        OsString::from(options.outfmt.to_string()),
        OsString::from("-evalue"),
        OsString::from(options.evalue.to_string()),
        OsString::from("-max_target_seqs"),
        OsString::from(options.max_target_sequences.to_string()),
        OsString::from("-num_threads"),
        OsString::from(options.threads.to_string()),
    ]
}

pub fn diamond_arguments(
    query: &Path,
    database: &Path,
    output: &Path,
    mode: DiamondMode,
    options: &SimilaritySearchOptions,
) -> Vec<OsString> {
    vec![
        OsString::from(mode.as_str()),
        OsString::from("--query"),
        query.as_os_str().to_owned(),
        OsString::from("--db"),
        database.as_os_str().to_owned(),
        OsString::from("--out"),
        output.as_os_str().to_owned(),
        OsString::from("--outfmt"),
        OsString::from(options.outfmt.to_string()),
        OsString::from("--evalue"),
        OsString::from(options.evalue.to_string()),
        OsString::from("--max-target-seqs"),
        OsString::from(options.max_target_sequences.to_string()),
        OsString::from("--threads"),
        OsString::from(options.threads.to_string()),
    ]
}

pub fn hmmer_arguments(
    profile: &Path,
    sequences: &Path,
    output: &Path,
    options: &HmmerOptions,
) -> Vec<OsString> {
    vec![
        OsString::from("--cpu"),
        OsString::from(options.threads.to_string()),
        OsString::from("-E"),
        OsString::from(options.evalue.to_string()),
        OsString::from("--domtblout"),
        output.as_os_str().to_owned(),
        profile.as_os_str().to_owned(),
        sequences.as_os_str().to_owned(),
    ]
}

pub fn muscle_arguments(input: &Path, output: &Path, options: &MuscleOptions) -> Vec<OsString> {
    vec![
        OsString::from(format!("-{}", options.mode.as_str())),
        input.as_os_str().to_owned(),
        OsString::from("-output"),
        output.as_os_str().to_owned(),
        OsString::from("-threads"),
        OsString::from(options.threads.to_string()),
    ]
}

pub fn trimal_arguments(input: &Path, output: &Path, mode: TrimalMode) -> Vec<OsString> {
    vec![
        OsString::from("-in"),
        input.as_os_str().to_owned(),
        OsString::from("-out"),
        output.as_os_str().to_owned(),
        OsString::from(format!("-{}", mode.as_str())),
    ]
}

pub fn iqtree_arguments(alignment: &Path, prefix: &Path, options: &IqtreeOptions) -> Vec<OsString> {
    vec![
        OsString::from("-s"),
        alignment.as_os_str().to_owned(),
        OsString::from("-pre"),
        prefix.as_os_str().to_owned(),
        OsString::from("-nt"),
        OsString::from(options.threads.to_string()),
        OsString::from("-m"),
        OsString::from(&options.model),
        OsString::from("-seed"),
        OsString::from(options.seed.to_string()),
    ]
}

pub fn mcscanx_arguments(dataset: &Path) -> Vec<OsString> {
    vec![dataset.as_os_str().to_owned()]
}

pub fn kaks_arguments(input: &Path, output: &Path, method: &str) -> Vec<OsString> {
    vec![
        OsString::from("-i"),
        input.as_os_str().to_owned(),
        OsString::from("-o"),
        output.as_os_str().to_owned(),
        OsString::from("-m"),
        OsString::from(method),
    ]
}

/// Run RNAfold (ViennaRNA) to predict RNA secondary structure.
pub fn run_rnafold_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    temperature: f64,
) -> Result<NativeToolResult, NativeToolError> {
    let input = input.as_ref();
    let output = output.as_ref();
    validate_paths(&[input], output)?;
    if !temperature.is_finite() || !(0.0..=100.0).contains(&temperature) {
        return Err(NativeToolError::InvalidOption(format!(
            "temperature must be between 0 and 100, got {temperature}"
        )));
    }
    let executable = configured_program("LINXIRA_BIO_RNAFOLD", "RNAfold");
    let arguments = rnafold_arguments(input, temperature);
    let result = (|| {
        let native_output = run_native_command(&executable, &arguments, false)?;
        let stdout = String::from_utf8_lossy(&native_output.stdout);
        let mut cleaned = String::new();
        for line in stdout.lines() {
            if !line.starts_with('>') {
                cleaned.push_str(line);
                cleaned.push('\n');
            }
        }
        fs::write(output, cleaned.as_bytes())?;
        finish_result("RNAfold", "secondary-structure", output, 1, 1)
    })();
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

pub fn rnafold_arguments(input: &Path, temperature: f64) -> Vec<OsString> {
    vec![
        OsString::from("--noPS"),
        OsString::from("--temp"),
        OsString::from(temperature.to_string()),
        input.as_os_str().to_owned(),
    ]
}

/// Options for the Kraken2 metagenomic classifier.
#[derive(Debug, Clone, PartialEq)]
pub struct Kraken2Options {
    pub database: PathBuf,
    pub confidence: f64,
    pub minimum_hit_groups: usize,
    pub threads: usize,
}

/// One row of a Kraken2 `--report` (clade-level taxonomy counts).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Kraken2TaxonRow {
    /// Clade fraction of total reads, 0–100.
    pub percentage: f64,
    pub clade_count: u64,
    pub taxon_count: u64,
    /// Kraken2 rank code: `R`, `R1`, `D`, `P`, `C`, `O`, `F`, `G`, `S`, `S1`, `U`, …
    pub rank: String,
    pub taxon_id: u64,
    pub name: String,
}

/// Structured summary of a Kraken2 classification run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetagenomicsClassifyResult {
    pub tool: String,
    pub output_path: String,
    pub output_bytes: u64,
    pub thread_count: usize,
    pub command_count: u64,
    pub total_reads: u64,
    pub classified_reads: u64,
    pub unclassified_reads: u64,
    pub classified_fraction: f64,
    pub taxon_count: usize,
    pub warnings: Vec<String>,
}

impl Default for Kraken2Options {
    fn default() -> Self {
        Self {
            database: PathBuf::new(),
            confidence: 0.0,
            minimum_hit_groups: 2,
            threads: 1,
        }
    }
}

/// Build the shell-free Kraken2 argument vector.
pub fn kraken2_arguments(
    input: &Path,
    report: &Path,
    classified: &Path,
    options: &Kraken2Options,
) -> Vec<OsString> {
    vec![
        OsString::from("--db"),
        options.database.as_os_str().to_owned(),
        OsString::from("--threads"),
        OsString::from(options.threads.to_string()),
        OsString::from("--confidence"),
        OsString::from(options.confidence.to_string()),
        OsString::from("--minimum-hit-groups"),
        OsString::from(options.minimum_hit_groups.to_string()),
        OsString::from("--report"),
        report.as_os_str().to_owned(),
        OsString::from("--output"),
        classified.as_os_str().to_owned(),
        input.as_os_str().to_owned(),
    ]
}

/// Parse a Kraken2 `--report` into taxonomy rows.
///
/// Each non-empty line has six whitespace-separated columns:
/// `percentage clade_count taxon_count rank taxid name`. Names are left
/// trimmed (Kraken2 indents them by rank). Malformed lines are rejected.
pub fn parse_kraken2_report(report: &str) -> Result<Vec<Kraken2TaxonRow>, NativeToolError> {
    let mut rows = Vec::new();
    for (index, line) in report.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let percentage = fields
            .next()
            .ok_or_else(|| invalid_report(index, "missing percentage"))?;
        let clade_count = fields
            .next()
            .ok_or_else(|| invalid_report(index, "missing clade count"))?;
        let taxon_count = fields
            .next()
            .ok_or_else(|| invalid_report(index, "missing taxon count"))?;
        let rank = fields
            .next()
            .ok_or_else(|| invalid_report(index, "missing rank"))?;
        let taxon_id = fields
            .next()
            .ok_or_else(|| invalid_report(index, "missing taxon id"))?;
        let name = fields.collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            return Err(invalid_report(index, "missing taxon name"));
        }
        rows.push(Kraken2TaxonRow {
            percentage: percentage
                .parse()
                .map_err(|_| invalid_report(index, format!("invalid percentage: {percentage}")))?,
            clade_count: clade_count.parse().map_err(|_| {
                invalid_report(index, format!("invalid clade count: {clade_count}"))
            })?,
            taxon_count: taxon_count.parse().map_err(|_| {
                invalid_report(index, format!("invalid taxon count: {taxon_count}"))
            })?,
            rank: rank.to_owned(),
            taxon_id: taxon_id
                .parse()
                .map_err(|_| invalid_report(index, format!("invalid taxon id: {taxon_id}")))?,
            name,
        });
    }
    if rows.is_empty() {
        return Err(NativeToolError::InvalidOption(
            "kraken2 report contains no taxonomy rows".to_owned(),
        ));
    }
    Ok(rows)
}

fn invalid_report(line: usize, detail: impl Into<String>) -> NativeToolError {
    NativeToolError::InvalidOption(format!(
        "invalid kraken2 report line {}: {}",
        line + 1,
        detail.into()
    ))
}

/// Render taxonomy rows as a TSV abundance table.
pub fn render_kraken2_abundance_table(rows: &[Kraken2TaxonRow]) -> String {
    let mut table = String::from("percentage\tclade_count\ttaxon_count\trank\ttaxon_id\tname\n");
    for row in rows {
        table.push_str(&format!(
            "{:.2}\t{}\t{}\t{}\t{}\t{}\n",
            row.percentage, row.clade_count, row.taxon_count, row.rank, row.taxon_id, row.name
        ));
    }
    table
}

/// Run Kraken2 with controlled arguments (no shell) and reduce its `--report`
/// into the abundance table written to `output`.
pub fn run_kraken2_path(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &Kraken2Options,
) -> Result<MetagenomicsClassifyResult, NativeToolError> {
    let input = input.as_ref();
    let output = output.as_ref();
    validate_paths(&[input], output)?;
    if options.database.as_os_str().is_empty() {
        return Err(NativeToolError::InvalidOption(
            "kraken2 classification requires a --database directory".to_owned(),
        ));
    }
    if !options.database.is_dir() {
        return Err(NativeToolError::InvalidOption(format!(
            "kraken2 database is not a directory: {}",
            options.database.display()
        )));
    }
    if !options.confidence.is_finite() || !(0.0..=1.0).contains(&options.confidence) {
        return Err(NativeToolError::InvalidOption(format!(
            "confidence must be between 0 and 1, got {}",
            options.confidence
        )));
    }
    if options.minimum_hit_groups < 1 {
        return Err(NativeToolError::InvalidOption(format!(
            "minimum_hit_groups must be at least 1, got {}",
            options.minimum_hit_groups
        )));
    }
    if options.threads < 1 || options.threads > MAX_THREADS {
        return Err(NativeToolError::InvalidOption(format!(
            "threads must be between 1 and {MAX_THREADS}, got {}",
            options.threads
        )));
    }
    let executable = configured_program("LINXIRA_BIO_KRAKEN2", "kraken2");
    let working = create_temporary_directory(output, "kraken2")?;
    let report_path = working.join("report.txt");
    let classified_path = working.join("classified.txt");
    let result = (|| {
        let arguments = kraken2_arguments(input, &report_path, &classified_path, options);
        run_native_command(&executable, &arguments, false)?;
        let report = fs::read_to_string(&report_path).map_err(NativeToolError::Io)?;
        let rows = parse_kraken2_report(&report)?;
        let table = render_kraken2_abundance_table(&rows);
        fs::write(output, table.as_bytes())?;
        let (total_reads, classified_reads, unclassified_reads) = summarize_kraken2(&rows);
        let classified_fraction = if total_reads == 0 {
            0.0
        } else {
            classified_reads as f64 / total_reads as f64
        };
        if !output.is_file() {
            return Err(NativeToolError::MissingOutput {
                tool: "kraken2".to_owned(),
                path: output.to_path_buf(),
            });
        }
        Ok(MetagenomicsClassifyResult {
            tool: "kraken2".to_owned(),
            output_path: output.to_string_lossy().into_owned(),
            output_bytes: fs::metadata(output)?.len(),
            thread_count: options.threads,
            command_count: 1,
            total_reads,
            classified_reads,
            unclassified_reads,
            classified_fraction,
            taxon_count: rows.len(),
            warnings: Vec::new(),
        })
    })();
    let _ = fs::remove_dir_all(&working);
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

/// Derive read totals from the report: the root (`R`) clade count is the
/// classified total and the `unclassified` (`U`) row carries the rest.
fn summarize_kraken2(rows: &[Kraken2TaxonRow]) -> (u64, u64, u64) {
    let classified = rows
        .iter()
        .find(|row| row.rank == "R")
        .map_or(0, |row| row.clade_count);
    let unclassified = rows
        .iter()
        .find(|row| row.rank == "U")
        .map_or(0, |row| row.clade_count);
    (classified + unclassified, classified, unclassified)
}

pub fn meme_arguments(
    input: &Path,
    output_directory: &Path,
    options: &MemeOptions,
) -> Vec<OsString> {
    vec![
        input.as_os_str().to_owned(),
        OsString::from("-oc"),
        output_directory.as_os_str().to_owned(),
        OsString::from(format!("-{}", options.alphabet.as_str())),
        OsString::from("-mod"),
        OsString::from(&options.distribution),
        OsString::from("-nmotifs"),
        OsString::from(options.motif_count.to_string()),
        OsString::from("-minw"),
        OsString::from(options.minimum_width.to_string()),
        OsString::from("-maxw"),
        OsString::from(options.maximum_width.to_string()),
        OsString::from("-p"),
        OsString::from(options.threads.to_string()),
    ]
}

pub fn dssp_arguments(structure: &Path, output: &Path) -> Vec<OsString> {
    vec![
        OsString::from("-i"),
        structure.as_os_str().to_owned(),
        OsString::from("-o"),
        output.as_os_str().to_owned(),
    ]
}

pub fn samtools_report_arguments(
    input: &Path,
    reference: Option<&Path>,
    mode: &str,
) -> Vec<OsString> {
    let mut arguments = vec![OsString::from(mode)];
    if let Some(reference) = reference {
        arguments.push(OsString::from("--reference"));
        arguments.push(reference.as_os_str().to_owned());
    }
    arguments.push(input.as_os_str().to_owned());
    arguments
}

pub fn bam_coverage_arguments(input: &Path, output: &Path, threads: usize) -> Vec<OsString> {
    vec![
        OsString::from("--bam"),
        input.as_os_str().to_os_string(),
        OsString::from("--outFileName"),
        output.as_os_str().to_os_string(),
        OsString::from("--outFileFormat"),
        OsString::from("bigwig"),
        OsString::from("--numberOfProcessors"),
        OsString::from(threads.to_string()),
    ]
}

pub fn minimap2_short_read_arguments(
    reference: &Path,
    reads: &Path,
    output: &Path,
    options: &ShortReadAlignmentOptions,
) -> Vec<OsString> {
    vec![
        OsString::from("-a"),
        OsString::from("-x"),
        OsString::from("sr"),
        OsString::from("-t"),
        OsString::from(options.threads.to_string()),
        OsString::from("-o"),
        output.as_os_str().to_owned(),
        reference.as_os_str().to_owned(),
        reads.as_os_str().to_owned(),
    ]
}

pub fn minimap2_long_read_arguments(
    reference: &Path,
    reads: &Path,
    output: &Path,
    options: &Minimap2LongReadOptions,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-a"),
        OsString::from("-x"),
        OsString::from(options.preset.as_str()),
        OsString::from("-t"),
        OsString::from(options.threads.to_string()),
        OsString::from("-o"),
        output.as_os_str().to_owned(),
    ];
    if !options.secondary {
        args.push(OsString::from("--secondary=no"));
    } else if options.max_secondary > 0 {
        args.push(OsString::from("-N"));
        args.push(OsString::from(options.max_secondary.to_string()));
    }
    args.push(reference.as_os_str().to_owned());
    args.push(reads.as_os_str().to_owned());
    args
}

pub fn snpeff_arguments(vcf: &Path, options: &SnpEffOptions) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-v"),
        OsString::from(&options.database),
        vcf.as_os_str().to_owned(),
    ];
    if let Some(ud) = options.upstream_downstream {
        args.push(OsString::from("-ud"));
        args.push(OsString::from(ud.to_string()));
    }
    if options.no_stats {
        args.push(OsString::from("-noStats"));
    }
    if options.no_log {
        args.push(OsString::from("-noLog"));
    }
    args
}

pub fn mast_arguments(
    motif: &Path,
    sequences: &Path,
    output_directory: &Path,
    options: &MastOptions,
) -> Vec<OsString> {
    let mut args = vec![
        motif.as_os_str().to_owned(),
        sequences.as_os_str().to_owned(),
        OsString::from("-oc"),
        output_directory.as_os_str().to_owned(),
        OsString::from("-mt"),
        OsString::from(options.evalue.to_string()),
    ];
    if options.hit_list {
        args.push(OsString::from("-hit_list"));
    }
    if options.add_self_compat {
        args.push(OsString::from("-add_self_compat"));
    }
    args
}

pub fn parse_minimap2_preset(value: &str) -> Result<Minimap2Preset, NativeToolError> {
    match value {
        "map-ont" => Ok(Minimap2Preset::MapOnt),
        "map-pb" => Ok(Minimap2Preset::MapPb),
        "map-hifi" => Ok(Minimap2Preset::MapHifi),
        "splice" => Ok(Minimap2Preset::Splice),
        "asm5" => Ok(Minimap2Preset::Asm5),
        "asm10" => Ok(Minimap2Preset::Asm10),
        "asm20" => Ok(Minimap2Preset::Asm20),
        "sr" => Ok(Minimap2Preset::Sr),
        other => Err(NativeToolError::InvalidOption(format!(
            "unknown minimap2 preset: {other}; expected map-ont, map-pb, map-hifi, splice, asm5, asm10, asm20, or sr"
        ))),
    }
}

pub fn samtools_sort_arguments(
    input: &Path,
    output: &Path,
    options: &ShortReadAlignmentOptions,
) -> Vec<OsString> {
    vec![
        OsString::from("sort"),
        OsString::from("-@"),
        OsString::from(options.threads.to_string()),
        OsString::from("-o"),
        output.as_os_str().to_owned(),
        input.as_os_str().to_owned(),
    ]
}

fn validate_similarity_options(options: &SimilaritySearchOptions) -> Result<(), NativeToolError> {
    validate_threads(options.threads)?;
    validate_evalue(options.evalue)?;
    if options.max_target_sequences == 0 {
        return Err(NativeToolError::InvalidOption(
            "max_target_sequences must be greater than zero".to_owned(),
        ));
    }
    if !matches!(options.outfmt, 6 | 7) {
        return Err(NativeToolError::InvalidOption(
            "outfmt must be 6 or 7 for reusable local tabular output".to_owned(),
        ));
    }
    Ok(())
}

fn parse_kaks_method(method: &str) -> Result<&str, NativeToolError> {
    match method.trim().to_ascii_uppercase().as_str() {
        "NG" => Ok("NG"),
        "LWL" => Ok("LWL"),
        "LPB" => Ok("LPB"),
        "YN" => Ok("YN"),
        _ => Err(NativeToolError::InvalidOption(
            "Ka/Ks method must be NG, LWL, LPB, or YN".to_owned(),
        )),
    }
}

fn validate_threads(threads: usize) -> Result<(), NativeToolError> {
    if !(1..=MAX_THREADS).contains(&threads) {
        return Err(NativeToolError::InvalidOption(format!(
            "threads must be between 1 and {MAX_THREADS}"
        )));
    }
    Ok(())
}

fn validate_evalue(evalue: f64) -> Result<(), NativeToolError> {
    if !evalue.is_finite() || evalue <= 0.0 {
        return Err(NativeToolError::InvalidOption(
            "evalue must be finite and greater than zero".to_owned(),
        ));
    }
    Ok(())
}

fn validate_paths(inputs: &[&Path], output: &Path) -> Result<(), NativeToolError> {
    for input in inputs {
        if !input.is_file() {
            return Err(NativeToolError::MissingInput((*input).to_path_buf()));
        }
        if paths_equivalent(input, output)? {
            return Err(NativeToolError::InputEqualsOutput(output.to_path_buf()));
        }
    }
    if output.exists() {
        return Err(NativeToolError::OutputAlreadyExists(output.to_path_buf()));
    }
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(NativeToolError::InvalidOption(format!(
            "output directory does not exist: {}",
            parent.display()
        )));
    }
    Ok(())
}

fn paths_equivalent(input: &Path, output: &Path) -> Result<bool, NativeToolError> {
    let input = fs::canonicalize(input)?;
    let output_parent = output.parent().unwrap_or_else(|| Path::new("."));
    let output_parent = fs::canonicalize(output_parent)?;
    let output_name = output
        .file_name()
        .ok_or_else(|| NativeToolError::InvalidOption("output requires a file name".to_owned()))?;
    Ok(input == output_parent.join(output_name))
}

fn configured_program(variable: &str, fallback: &str) -> OsString {
    std::env::var_os(variable).unwrap_or_else(|| OsString::from(fallback))
}

fn run_native_command(
    program: &OsStr,
    arguments: &[OsString],
    disable_blast_reporting: bool,
) -> Result<Output, NativeToolError> {
    let tool = program.to_string_lossy().into_owned();
    let mut command = Command::new(program);
    command.args(arguments);
    if disable_blast_reporting {
        command.env("BLAST_USAGE_REPORT", "false");
    }
    let output = command.output().map_err(|source| NativeToolError::Spawn {
        tool: tool.clone(),
        source,
    })?;
    if !output.status.success() {
        return Err(NativeToolError::Failed {
            tool,
            status: output.status.code(),
            stderr: stderr_summary(&output.stderr),
        });
    }
    Ok(output)
}

fn finish_result(
    tool: &str,
    mode: &str,
    output: &Path,
    threads: usize,
    command_count: u64,
) -> Result<NativeToolResult, NativeToolError> {
    if !output.is_file() {
        return Err(NativeToolError::MissingOutput {
            tool: tool.to_owned(),
            path: output.to_path_buf(),
        });
    }
    Ok(NativeToolResult {
        tool: tool.to_owned(),
        mode: mode.to_owned(),
        output_path: output.to_string_lossy().into_owned(),
        output_bytes: fs::metadata(output)?.len(),
        thread_count: threads,
        command_count,
        warnings: Vec::new(),
    })
}

fn copy_generated_output(
    tool: &str,
    generated: &Path,
    output: &Path,
) -> Result<(), NativeToolError> {
    if !generated.is_file() {
        return Err(NativeToolError::MissingOutput {
            tool: tool.to_owned(),
            path: generated.to_path_buf(),
        });
    }
    fs::copy(generated, output)?;
    Ok(())
}

fn create_temporary_directory(output: &Path, purpose: &str) -> Result<PathBuf, NativeToolError> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    for _ in 0..100 {
        let ordinal = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".linxira-{purpose}-{}-{ordinal}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(NativeToolError::Io(error)),
        }
    }
    Err(NativeToolError::InvalidOption(
        "could not allocate an isolated native-tool temporary directory".to_owned(),
    ))
}

fn remove_incomplete_output(output: &Path) {
    if output.is_file() {
        let _ = fs::remove_file(output);
    }
}

#[derive(Debug, Clone)]
pub struct WgcnaOptions {
    pub threads: usize,
    pub min_expression: f64,
    pub min_samples: usize,
    pub min_module_size: usize,
    pub merge_cut_height: f64,
    pub network_type: String,
    pub power: usize,
    pub log_transform: bool,
}

impl Default for WgcnaOptions {
    fn default() -> Self {
        Self {
            threads: 1,
            min_expression: 1.0,
            min_samples: 3,
            min_module_size: 30,
            merge_cut_height: 0.25,
            network_type: "signed".to_owned(),
            power: 0,
            log_transform: true,
        }
    }
}

pub fn run_wgcna_path(
    expression: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &WgcnaOptions,
) -> Result<NativeToolResult, NativeToolError> {
    let expression = expression.as_ref();
    let output = output.as_ref();
    validate_paths(&[expression], output)?;
    let temporary = create_temporary_directory(output, "wgcna")?;
    let result = (|| {
        let executable = configured_program("LINXIRA_BIO_RSCRIPT", "Rscript");
        let wgcna_script = find_wgcna_script()?;
        let request_path = temporary.join("request.json");
        let request = serde_json::json!({
            "schema_version": "2",
            "job_id": "linxira-wgcna-cli",
            "capability": "expression.wgcna.v1",
            "inputs": [{
                "artifact_id": "expression",
                "role": "expression",
                "cardinality": "single",
                "files": [{
                    "file_id": "expr",
                    "path": expression.to_string_lossy(),
                    "format": if expression.to_string_lossy().ends_with(".tsv") { "tsv" } else { "csv" },
                    "compression": "none",
                    "size_bytes": expression.metadata().map(|m| m.len()).unwrap_or(0)
                }]
            }],
            "execution": { "mode": "local-cpu" },
            "parameters": {
                "output_directory": temporary.to_string_lossy(),
                "min_expression": options.min_expression,
                "min_samples": options.min_samples,
                "min_module_size": options.min_module_size,
                "merge_cut_height": options.merge_cut_height,
                "network_type": &options.network_type,
                "power": options.power,
                "log_transform": options.log_transform,
                "threads": options.threads
            }
        });
        fs::write(
            &request_path,
            serde_json::to_vec(&request).map_err(|e| {
                NativeToolError::InvalidOption(format!("JSON serialization failed: {e}"))
            })?,
        )?;
        let result_path = temporary.join("result.json");
        let arguments: Vec<OsString> = vec![
            wgcna_script.into(),
            OsString::from("--request"),
            request_path.into(),
            OsString::from("--result"),
            result_path.into(),
        ];
        let output_result = run_native_command(&executable, &arguments, false)?;
        let result_json = temporary.join("result.json");
        if !result_json.exists() {
            let stderr = String::from_utf8_lossy(&output_result.stderr);
            return Err(NativeToolError::InvalidOption(format!(
                "WGCNA workflow did not produce result.json: {}",
                stderr.trim()
            )));
        }
        fs::copy(&result_json, output)?;
        finish_result("wgcna", "co-expression-network", output, options.threads, 1)
    })();
    let cleanup = fs::remove_dir_all(&temporary);
    if let Err(error) = cleanup
        && result.is_ok()
    {
        return Err(NativeToolError::Io(error));
    }
    if result.is_err() {
        remove_incomplete_output(output);
    }
    result
}

fn find_wgcna_script() -> Result<PathBuf, NativeToolError> {
    if let Ok(path) = std::env::var("LINXIRA_BIO_WGCNA_SCRIPT") {
        let script = PathBuf::from(&path);
        if script.exists() {
            return Ok(script);
        }
    }
    let candidates = [
        "workflows/org.linxira.expression-wgcna/src/run_wgcna.R",
        "../workflows/org.linxira.expression-wgcna/src/run_wgcna.R",
    ];
    for candidate in &candidates {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(NativeToolError::InvalidOption(
        "WGCNA R script not found; set LINXIRA_BIO_WGCNA_SCRIPT or run from the project root"
            .to_owned(),
    ))
}

fn stderr_summary(stderr: &[u8]) -> String {
    let length = stderr.len().min(MAX_STDERR_BYTES);
    let mut summary = String::from_utf8_lossy(&stderr[..length]).trim().to_owned();
    if stderr.len() > length {
        summary.push_str(" [truncated]");
    }
    if summary.is_empty() {
        summary = "no stderr output".to_owned();
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::{
        BlastProgram, DiamondMode, HmmerOptions, IqtreeOptions, MemeAlphabet, MemeOptions,
        MuscleMode, MuscleOptions, ShortReadAlignmentOptions, SimilaritySearchOptions, TrimalMode,
        bam_coverage_arguments, blast_arguments, diamond_arguments, dssp_arguments,
        hmmer_arguments, iqtree_arguments, kaks_arguments, mcscanx_arguments, meme_arguments,
        minimap2_short_read_arguments, muscle_arguments, parse_blast_program, parse_diamond_mode,
        parse_hmmer_mode, parse_meme_alphabet, parse_muscle_mode, parse_trimal_mode,
        samtools_report_arguments, samtools_sort_arguments, trimal_arguments,
    };
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    #[test]
    fn builds_shell_free_native_tool_arguments() {
        let similarity = SimilaritySearchOptions {
            threads: 8,
            evalue: 1e-5,
            max_target_sequences: 25,
            outfmt: 7,
        };
        let blast = blast_arguments(
            Path::new("query with spaces.fa"),
            Path::new("database prefix"),
            Path::new("result.tsv"),
            &similarity,
        );
        assert_eq!(blast[0], OsString::from("-query"));
        assert!(blast.contains(&OsString::from("query with spaces.fa")));
        assert!(blast.contains(&OsString::from("8")));

        let diamond = diamond_arguments(
            Path::new("query.fa"),
            Path::new("database"),
            Path::new("result.tsv"),
            DiamondMode::Blastx,
            &similarity,
        );
        assert_eq!(diamond[0], OsString::from("blastx"));
        assert!(diamond.contains(&OsString::from("--max-target-seqs")));

        let hmmer = hmmer_arguments(
            Path::new("profile.hmm"),
            Path::new("proteins.fa"),
            Path::new("domains.tsv"),
            &HmmerOptions {
                threads: 4,
                evalue: 0.01,
            },
        );
        assert_eq!(hmmer[0], OsString::from("--cpu"));
        assert_eq!(hmmer.last(), Some(&OsString::from("proteins.fa")));

        let muscle = muscle_arguments(
            Path::new("sequences.fa"),
            Path::new("alignment.fa"),
            &MuscleOptions {
                threads: 6,
                mode: MuscleMode::Super5,
            },
        );
        assert_eq!(muscle[0], OsString::from("-super5"));
        assert!(muscle.contains(&OsString::from("-output")));

        let trimal = trimal_arguments(
            Path::new("alignment.fa"),
            Path::new("trimmed.fa"),
            TrimalMode::Automated1,
        );
        assert_eq!(trimal.last(), Some(&OsString::from("-automated1")));

        let iqtree = iqtree_arguments(
            Path::new("alignment.fa"),
            Path::new("run prefix"),
            &IqtreeOptions {
                threads: 8,
                model: "MFP".to_owned(),
                seed: 7,
            },
        );
        assert!(iqtree.contains(&OsString::from("run prefix")));
        assert!(iqtree.contains(&OsString::from("7")));

        let mcscanx = mcscanx_arguments(Path::new("isolated workspace/dataset"));
        assert_eq!(mcscanx, vec![OsString::from("isolated workspace/dataset")]);

        let kaks = kaks_arguments(Path::new("codons.axt"), Path::new("kaks.tsv"), "YN");
        assert_eq!(kaks[0], OsString::from("-i"));
        assert_eq!(kaks[4], OsString::from("-m"));
        assert_eq!(kaks[5], OsString::from("YN"));

        let meme = meme_arguments(
            Path::new("sequences.fa"),
            Path::new("meme output"),
            &MemeOptions {
                threads: 4,
                alphabet: MemeAlphabet::Protein,
                distribution: "anr".to_owned(),
                motif_count: 5,
                minimum_width: 4,
                maximum_width: 20,
            },
        );
        assert!(meme.contains(&OsString::from("-protein")));
        assert!(meme.contains(&OsString::from("meme output")));

        let dssp = dssp_arguments(Path::new("model.cif"), Path::new("model.dssp"));
        assert_eq!(dssp[0], OsString::from("-i"));
        assert_eq!(dssp[2], OsString::from("-o"));

        let report = samtools_report_arguments(
            Path::new("reads.cram"),
            Some(Path::new("reference.fa")),
            "stats",
        );
        let bigwig = bam_coverage_arguments(Path::new("reads.bam"), Path::new("track.bw"), 4);
        assert_eq!(bigwig[0], OsString::from("--bam"));
        assert!(bigwig.contains(&OsString::from("--outFileFormat")));
        assert!(bigwig.contains(&OsString::from("bigwig")));
        assert_eq!(report[0], OsString::from("stats"));
        assert!(report.contains(&OsString::from("--reference")));

        let short_read = minimap2_short_read_arguments(
            Path::new("reference.fa"),
            Path::new("reads.fq"),
            Path::new("alignment.sam"),
            &ShortReadAlignmentOptions { threads: 4 },
        );
        assert_eq!(short_read[0], OsString::from("-a"));
        assert!(short_read.contains(&OsString::from("sr")));
        let sorted = samtools_sort_arguments(
            Path::new("alignment.sam"),
            Path::new("alignment.bam"),
            &ShortReadAlignmentOptions { threads: 4 },
        );
        assert_eq!(sorted[0], OsString::from("sort"));
        assert!(sorted.contains(&OsString::from("alignment.bam")));
    }

    #[test]
    fn parses_only_supported_modes() {
        assert_eq!(parse_blast_program("blastn").unwrap(), BlastProgram::Blastn);
        assert_eq!(parse_diamond_mode("blastp").unwrap(), DiamondMode::Blastp);
        assert_eq!(parse_hmmer_mode("hmmscan").unwrap().as_str(), "hmmscan");
        assert_eq!(parse_muscle_mode("align").unwrap(), MuscleMode::Align);
        assert_eq!(parse_trimal_mode("gappyout").unwrap(), TrimalMode::Gappyout);
        assert_eq!(parse_meme_alphabet("rna").unwrap(), MemeAlphabet::Rna);
        assert!(parse_blast_program("remote").is_err());
        assert!(parse_diamond_mode("makedb").is_err());
        assert!(parse_hmmer_mode("jackhmmer").is_err());
        assert!(parse_muscle_mode("profile").is_err());
        assert!(parse_trimal_mode("manual").is_err());
        assert!(parse_meme_alphabet("codon").is_err());
    }

    #[test]
    fn blast_program_selects_reference_alphabet() {
        assert_eq!(BlastProgram::Blastn.database_type(), "nucl");
        assert_eq!(BlastProgram::Blastx.database_type(), "prot");
        assert_eq!(BlastProgram::Tblastn.database_type(), "nucl");
    }

    #[test]
    fn kraken2_report_parses_into_the_golden_abundance_table() {
        use super::{parse_kraken2_report, render_kraken2_abundance_table};
        use std::fs;
        use std::path::Path;

        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/fixtures/metagenomics");
        let report = fs::read_to_string(root.join("kraken2-report.txt")).expect("report fixture");
        let golden =
            fs::read_to_string(root.join("abundance-golden.tsv")).expect("golden table fixture");

        let rows = parse_kraken2_report(&report).expect("parse synthetic kraken2 report");
        assert_eq!(rows.len(), 16);
        assert_eq!(rows[0].rank, "R");
        assert_eq!(rows[0].taxon_id, 1);
        assert_eq!(rows[0].name, "root");
        assert_eq!(rows[3].rank, "P");
        assert_eq!(rows[3].name, "Proteobacteria");
        assert_eq!(rows[15].rank, "U");
        assert_eq!(rows[15].taxon_id, 0);
        assert_eq!(render_kraken2_abundance_table(&rows), golden);
    }

    #[test]
    fn kraken2_report_rejects_malformed_rows() {
        use super::parse_kraken2_report;

        assert!(parse_kraken2_report("").is_err());
        assert!(parse_kraken2_report("99.90\t999\t999\tR\t1").is_err());
        assert!(parse_kraken2_report("nope\t999\t999\tR\t1\troot").is_err());
        let valid = "99.90\t999\t999\tR\t1\troot\n";
        assert!(parse_kraken2_report(valid).is_ok());
    }

    #[test]
    fn kraken2_arguments_are_controlled_and_shell_free() {
        use super::{Kraken2Options, kraken2_arguments};
        use std::path::Path;

        let options = Kraken2Options {
            database: PathBuf::from("db with spaces"),
            confidence: 0.25,
            minimum_hit_groups: 3,
            threads: 4,
        };
        let arguments = kraken2_arguments(
            Path::new("reads with spaces.fq"),
            Path::new("report.txt"),
            Path::new("classified.txt"),
            &options,
        );
        assert_eq!(arguments[0], OsString::from("--db"));
        assert!(arguments.contains(&OsString::from("db with spaces")));
        assert!(arguments.contains(&OsString::from("0.25")));
        assert!(arguments.contains(&OsString::from("3")));
        assert!(arguments.contains(&OsString::from("4")));
        assert!(arguments.contains(&OsString::from("reads with spaces.fq")));
        assert!(
            arguments
                .iter()
                .all(|value| !value.to_string_lossy().contains([';', '&', '|', '$', '`']))
        );
    }
}
