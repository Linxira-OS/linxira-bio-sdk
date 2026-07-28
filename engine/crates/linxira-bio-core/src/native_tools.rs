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
        blast_arguments, diamond_arguments, dssp_arguments, hmmer_arguments, iqtree_arguments,
        meme_arguments, minimap2_short_read_arguments, muscle_arguments, parse_blast_program,
        parse_diamond_mode, parse_hmmer_mode, parse_meme_alphabet, parse_muscle_mode,
        parse_trimal_mode, samtools_report_arguments, samtools_sort_arguments, trimal_arguments,
    };
    use std::ffi::OsString;
    use std::path::Path;

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
}
