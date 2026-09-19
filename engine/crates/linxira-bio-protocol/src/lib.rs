#![forbid(unsafe_code)]

//! Stable job and result contracts shared by CLI, GUI, workers, and agents.

//! Shared protocol types for Linxira Bio jobs, results, and workflow packs.

pub mod semver_range;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The legacy request and result envelope version.
pub const SCHEMA_VERSION: &str = "1";
/// The artifact-aware request and result envelope version.
pub const SCHEMA_VERSION_V2: &str = "2";
pub const DATASET_MANIFEST_SCHEMA_VERSION: &str = "1";
pub const WORKFLOW_PACK_SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    LocalCpu,
    LocalGpu,
    Hpc,
    Cloud,
    AuthenticatedBrowser,
    Container,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRequest {
    pub schema_version: String,
    pub job_id: String,
    pub capability: String,
    pub inputs: BTreeMap<String, String>,
    pub execution: ExecutionRequest,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub mode: ExecutionMode,
    /// Implementation backend for capabilities that ship more than one
    /// independent implementation (M2/M3). `None` and `Rust` both select the
    /// native engine; `Python`/`R` route to the benchmark packs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<ExecutionBackend>,
}

impl ExecutionRequest {
    pub fn local_cpu() -> Self {
        Self {
            mode: ExecutionMode::LocalCpu,
            backend: None,
        }
    }
}

/// Independent implementation backends of one capability. The wire form is
/// lowercase (`"rust"`, `"python"`, `"r"`) to match `--backend` on the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionBackend {
    #[default]
    Rust,
    Python,
    R,
}

impl ExecutionBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Python => "python",
            Self::R => "r",
        }
    }

    /// Parses the CLI/GUI spelling; `auto` is not a backend and must be
    /// resolved by the caller (runtime preferences, M2-T7) before this point.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "rust" => Some(Self::Rust),
            "python" => Some(Self::Python),
            "r" => Some(Self::R),
            _ => None,
        }
    }
}

/// Schema version of `runtime-preferences.json` (M2-T7).
pub const RUNTIME_PREFERENCES_SCHEMA_VERSION: u32 = 1;
/// Environment override pointing at a runtime preference table; the only
/// location error that is treated as fatal (explicit configuration must not
/// silently vanish).
pub const RUNTIME_PREFERENCES_ENV: &str = "LINXIRA_BIO_RUNTIME_PREFERENCES";

/// Benchmark-derived default backends, written back by benchmark runs
/// (ROADMAP M2-T7). One entry per capability (plus dataset class); the file
/// order is precedence when several entries match one capability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimePreferences {
    pub schema_version: u32,
    #[serde(default)]
    pub preferences: Vec<RuntimePreference>,
}

impl Default for RuntimePreferences {
    fn default() -> Self {
        Self {
            schema_version: RUNTIME_PREFERENCES_SCHEMA_VERSION,
            preferences: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimePreference {
    pub capability: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_class: Option<String>,
    pub default_backend: ExecutionBackend,
    #[serde(default)]
    pub measured: BTreeMap<String, BackendMeasurement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampled_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speedup: Option<f64>,
    /// `consistent` | `inconsistent` | `failed` as reported by the source
    /// benchmark; only `consistent` entries should drive the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consistency: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BackendMeasurement {
    pub wall_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_rss_mb: Option<f64>,
}

impl RuntimePreferences {
    /// Default backend for a capability: the first matching entry wins.
    /// A table with an unrecognized `schema_version` is inert, so an older
    /// binary never acts on a table format it cannot understand.
    pub fn default_backend_for(&self, capability: &str) -> Option<ExecutionBackend> {
        if self.schema_version != RUNTIME_PREFERENCES_SCHEMA_VERSION {
            return None;
        }
        self.preferences
            .iter()
            .find(|entry| entry.capability == capability)
            .map(|entry| entry.default_backend)
    }

    /// Loads the table: `LINXIRA_BIO_RUNTIME_PREFERENCES` (explicit; must
    /// exist and parse), then `runtime-preferences.json` in the working
    /// directory, then next to the executable, then the table embedded at
    /// build time. Candidate files that fail to parse are skipped so a
    /// stale table next to an old binary cannot break every job; an
    /// explicitly configured path propagates its error.
    pub fn load() -> Result<Self, String> {
        if let Some(configured) = std::env::var_os(RUNTIME_PREFERENCES_ENV) {
            if configured.is_empty() {
                return Err(format!("{RUNTIME_PREFERENCES_ENV} must not be empty"));
            }
            return read_preferences_file(std::path::Path::new(&configured)).map_err(|error| {
                format!("{RUNTIME_PREFERENCES_ENV} is not a usable preference table: {error}")
            });
        }
        let mut candidates = Vec::new();
        if let Ok(current) = std::env::current_dir() {
            candidates.push(current.join("runtime-preferences.json"));
        }
        if let Ok(executable) = std::env::current_exe()
            && let Some(directory) = executable.parent()
        {
            candidates.push(directory.join("runtime-preferences.json"));
        }
        for candidate in candidates {
            if candidate.is_file()
                && let Ok(preferences) = read_preferences_file(&candidate)
            {
                return Ok(preferences);
            }
        }
        Ok(embedded_runtime_preferences())
    }
}

fn read_preferences_file(path: &std::path::Path) -> Result<RuntimePreferences, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("could not parse {}: {error}", path.display()))
}

/// The repository's `runtime-preferences.json` at build time; the shipped
/// fallback when no file is found on disk.
pub fn embedded_runtime_preferences() -> RuntimePreferences {
    serde_json::from_str(include_str!("../../../../runtime-preferences.json"))
        .expect("embedded runtime-preferences.json must be valid")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub engine_version: String,
    pub execution_mode: ExecutionMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResult<T> {
    pub schema_version: String,
    pub job_id: String,
    pub capability: String,
    pub status: JobStatus,
    pub result: T,
    pub provenance: Provenance,
    pub warnings: Vec<String>,
}

impl<T> AnalysisResult<T>
where
    T: Serialize,
{
    pub fn ok(
        job_id: impl Into<String>,
        capability: impl Into<String>,
        result: T,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_owned(),
            job_id: job_id.into(),
            capability: capability.into(),
            status: JobStatus::Ok,
            result,
            provenance: Provenance {
                engine_version: env!("CARGO_PKG_VERSION").to_owned(),
                execution_mode,
            },
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BioDataFormat {
    Fasta,
    Fastq,
    Csv,
    Tsv,
    Txt,
    Bed,
    Gff3,
    Gtf,
    Vcf,
    Sam,
    Bam,
    Bcf,
    Cram,
    Bigwig,
    Genbank,
    Embl,
    H5ad,
    Loom,
    Hdf5,
    Rds,
    Pdb,
    Mmcif,
    Sdf,
    Axt,
    BlastTabular,
    BlastXml,
    ProteinDomains,
    MemeText,
    McscanxCollinearity,
    Newick,
    Svg,
    Xlsx,
    Json,
    Jsonl,
    Parquet,
    Zip,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompressionFormat {
    None,
    Gzip,
    Bgzip,
    Bzip2,
    Xz,
    Zstd,
    Zip,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactFile {
    pub file_id: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    pub format: BioDataFormat,
    pub compression: CompressionFormat,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputCardinality {
    Single,
    Paired,
    Batch,
}

/// A named input supplied to a capability.
///
/// `files` contains exactly one file for `single`, exactly two for `paired`,
/// and one or more files for `batch`; the JSON schema enforces this invariant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputArtifact {
    pub artifact_id: String,
    pub role: String,
    pub cardinality: InputCardinality,
    pub files: Vec<ArtifactFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_id: Option<String>,
}

impl InputArtifact {
    pub fn has_valid_cardinality(&self) -> bool {
        match self.cardinality {
            InputCardinality::Single => self.files.len() == 1,
            InputCardinality::Paired => self.files.len() == 2,
            InputCardinality::Batch => !self.files.is_empty(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetRelationshipKind {
    PairedEnd,
    IndexFor,
    ReferenceFor,
    DerivedFrom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetRelationship {
    pub kind: DatasetRelationshipKind,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValidationState {
    Pending,
    Valid,
    ValidWithWarnings,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetValidation {
    pub state: ValidationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatasetManifest {
    pub schema_version: String,
    pub dataset_id: String,
    pub display_name: String,
    pub created_at: String,
    pub files: Vec<ArtifactFile>,
    #[serde(default)]
    pub relationships: Vec<DatasetRelationship>,
    pub validation: DatasetValidation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobRequestV2 {
    pub schema_version: String,
    pub job_id: String,
    pub capability: String,
    pub inputs: Vec<InputArtifact>,
    pub execution: ExecutionRequest,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputArtifactKind {
    Table,
    Plot,
    DomainFile,
    Log,
    Report,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputArtifact {
    pub artifact_id: String,
    pub role: String,
    pub kind: OutputArtifactKind,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<BioDataFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoftwareProvenance {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceV2 {
    pub engine_version: String,
    pub execution_mode: ExecutionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub software: Vec<SoftwareProvenance>,
    #[serde(default)]
    pub input_sha256: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependency_lock_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResultV2<T> {
    pub schema_version: String,
    pub job_id: String,
    pub capability: String,
    pub status: JobStatus,
    pub result: T,
    #[serde(default)]
    pub artifacts: Vec<OutputArtifact>,
    pub provenance: ProvenanceV2,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> AnalysisResultV2<T>
where
    T: Serialize,
{
    pub fn ok(
        job_id: impl Into<String>,
        capability: impl Into<String>,
        result: T,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V2.to_owned(),
            job_id: job_id.into(),
            capability: capability.into(),
            status: JobStatus::Ok,
            result,
            artifacts: Vec::new(),
            provenance: provenance_v2(execution_mode),
            diagnostics: Vec::new(),
        }
    }
}

impl AnalysisResultV2<serde_json::Value> {
    pub fn error(
        job_id: impl Into<String>,
        capability: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
        execution_mode: ExecutionMode,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V2.to_owned(),
            job_id: job_id.into(),
            capability: capability.into(),
            status: JobStatus::Error,
            result: serde_json::Value::Object(serde_json::Map::new()),
            artifacts: Vec::new(),
            provenance: provenance_v2(execution_mode),
            diagnostics: vec![Diagnostic {
                code: code.into(),
                severity: DiagnosticSeverity::Error,
                message: message.into(),
                artifact_id: None,
                line: None,
                record: None,
                hint: None,
            }],
        }
    }
}

fn provenance_v2(execution_mode: ExecutionMode) -> ProvenanceV2 {
    ProvenanceV2 {
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        execution_mode,
        core_version: Some(env!("CARGO_PKG_VERSION").to_owned()),
        started_at: None,
        finished_at: None,
        software: Vec::new(),
        input_sha256: BTreeMap::new(),
        command: None,
        dependency_lock_sha256: None,
    }
}

/// Schema version of the benchmark report document.
pub const BENCHMARK_REPORT_SCHEMA_VERSION: &str = "1";

/// One timed execution of a capability under one backend (M2-T1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub backend: String,
    /// 1-based index within the backend's timed repeats (the warmup is
    /// never recorded).
    pub run_index: u32,
    pub wall_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peak_rss_mb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_read_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_write_bytes: Option<u64>,
    /// Size of the result envelope produced by the run.
    pub output_bytes: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    /// Wall time the backend measured around the analysis itself, so the
    /// interpreter start-up share of `wall_ms` is visible; `None` for the
    /// native engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_reported_wall_ms: Option<f64>,
    /// Peak RSS the backend observed for its own process (Python `resource`,
    /// R `/proc/self/status` VmHWM); `None` when not reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_reported_peak_rss_mb: Option<f64>,
}

/// Aggregated statistics for one backend: median wall-time and peak RSS with
/// min/max/±IQR, per the benchmark methodology.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkBackendSummary {
    pub backend: String,
    pub repeats: u32,
    pub median_wall_ms: f64,
    pub min_wall_ms: f64,
    pub max_wall_ms: f64,
    pub iqr_wall_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_peak_rss_mb: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub median_cpu_ms: Option<f64>,
    pub runs: Vec<BenchmarkRun>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BenchmarkVerdict {
    Consistent,
    Inconsistent,
    Failed,
}

/// One field-level difference found by the consistency diff engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkFinding {
    pub field: String,
    pub detail: String,
}

/// Machine-readable environment disclosure (methodology §7.1): every report
/// records what ran, where, and at what timing precision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkEnvironment {
    pub os: String,
    /// Exact source revision (git sha) embedded at build time; every
    /// evaluation is tied to the code that produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_revision: Option<String>,
    /// GPU model and driver as disclosed by the host; even CPU-only
    /// workloads disclose it so readers can rule out accelerator effects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
    /// RAM generation and speed as disclosed by the host (e.g. DDR3-1600).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    /// Storage topology backing the benchmark I/O (SSD vs HDD per mount).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_memory_mb: Option<u64>,
    /// PRETTY_NAME from `/etc/os-release` (the Linux guest view, e.g.
    /// "Arch Linux"); distinguishes the actual distro from the kernel string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// `WSL_DISTRO_NAME` when the benchmark runs inside a WSL distribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wsl_distro: Option<String>,
    /// `wsl2` or `wsl1`, derived from the kernel release string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wsl_version: Option<String>,
    /// Windows host version reported by `cmd.exe /c ver` over WSL interop
    /// (e.g. "Microsoft Windows [Version 10.0.26200.1]").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_os: Option<String>,
    /// Windows host machine model ("manufacturer model") over WSL interop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_model: Option<String>,
    /// Windows host CPU model over WSL interop; the guest `/proc/cpuinfo`
    /// already shows the same silicon but the host view is authoritative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_cpu_model: Option<String>,
    /// Logical processors of the Windows host (the guest view is capped by
    /// the WSL allocation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_logical_processors: Option<u64>,
    /// Physical RAM of the Windows host in MB, distinct from the WSL guest
    /// allocation reported by `total_memory_mb`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_total_memory_mb: Option<u64>,
    pub engine_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r_version: Option<String>,
    pub in_container: bool,
    /// `warm` when repeats follow a warmup run on the same machine.
    pub page_cache: String,
    /// `high` with an external `/usr/bin/time -v` wrapper, `degraded` with
    /// in-process `Instant` only (Windows).
    pub precision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub schema_version: String,
    pub capability: String,
    pub dataset_class: String,
    pub backends: Vec<BenchmarkBackendSummary>,
    /// median_wall(native) / median_wall(rust); None until a native backend
    /// benchmark lands (M2-T3 packs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speedup: Option<f64>,
    /// 1 - peak_rss(rust)/peak_rss(native); None without a native backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_saving: Option<f64>,
    pub consistency: BenchmarkVerdict,
    #[serde(default)]
    pub findings: Vec<BenchmarkFinding>,
    pub environment: BenchmarkEnvironment,
    /// RFC3339 UTC sample time.
    pub sampled_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPublisher {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkflowRuntimeKind {
    Python,
    R,
    Java,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyLock {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowInputContract {
    pub role: String,
    pub formats: Vec<BioDataFormat>,
    /// Compression formats this role accepts. Absent or empty means the
    /// role reads uncompressed inputs only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compression: Vec<CompressionFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowOutputContract {
    pub roles: Vec<String>,
    pub kind: OutputArtifactKind,
    pub formats: Vec<BioDataFormat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

/// Explicit, machine-readable execution contract for a workflow pack. When a
/// manifest declares this, executors must derive the pack's input roles,
/// formats, parameters, and artifact contract from it instead of hardcoded
/// tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowContractDecl {
    pub inputs: Vec<WorkflowInputContract>,
    pub outputs: WorkflowOutputContract,
    pub parameters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowResumeConfig {
    pub enabled: bool,
    pub state_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRuntime {
    pub kind: WorkflowRuntimeKind,
    pub version: String,
    pub core_compatibility: String,
    pub dependency_lock: DependencyLock,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowEntrypoint {
    pub path: String,
    #[serde(default)]
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SupportedPlatform {
    WindowsGnu,
    Debian,
    Arch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkAccess {
    None,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub access: NetworkAccess,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GpuRequirement {
    #[default]
    None,
    Optional,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ResourceRequirements {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_memory_mb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_disk_mb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_threads: Option<u16>,
    pub gpu: GpuRequirement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_vram_mb: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowPackFile {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowPackManifest {
    pub schema_version: String,
    pub id: String,
    pub version: String,
    pub publisher: WorkflowPublisher,
    pub license: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    pub entrypoint: WorkflowEntrypoint,
    pub runtime: WorkflowRuntime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<WorkflowResumeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<WorkflowContractDecl>,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub platforms: Vec<SupportedPlatform>,
    pub network: NetworkPolicy,
    pub resources: ResourceRequirements,
    pub files: Vec<WorkflowPackFile>,
}

#[cfg(test)]
mod tests {
    use super::{
        AnalysisResult, AnalysisResultV2, BENCHMARK_REPORT_SCHEMA_VERSION, BackendMeasurement,
        BenchmarkBackendSummary, BenchmarkEnvironment, BenchmarkReport, BenchmarkRun,
        BenchmarkVerdict, BioDataFormat, CompressionFormat, DatasetManifest,
        DatasetRelationshipKind, DiagnosticSeverity, ExecutionBackend, ExecutionMode,
        InputCardinality, JobRequest, JobRequestV2, JobStatus, NetworkAccess,
        RUNTIME_PREFERENCES_SCHEMA_VERSION, RuntimePreference, RuntimePreferences, SCHEMA_VERSION,
        SCHEMA_VERSION_V2, ValidationState, WorkflowPackManifest, WorkflowRuntimeKind,
    };
    use std::collections::BTreeMap;

    #[test]
    fn runtime_preferences_hit_miss_and_schema_version_paths() {
        let table = RuntimePreferences {
            schema_version: RUNTIME_PREFERENCES_SCHEMA_VERSION,
            preferences: vec![RuntimePreference {
                capability: "sequence.stats.v1".to_owned(),
                dataset_class: Some("sequence".to_owned()),
                default_backend: ExecutionBackend::Python,
                measured: BTreeMap::from([
                    (
                        "rust".to_owned(),
                        BackendMeasurement {
                            wall_ms: 30.0,
                            peak_rss_mb: Some(5.8),
                        },
                    ),
                    (
                        "python".to_owned(),
                        BackendMeasurement {
                            wall_ms: 270.0,
                            peak_rss_mb: Some(43.7),
                        },
                    ),
                ]),
                sampled_on: Some("2026-09-11T15:27:50Z".to_owned()),
                speedup: Some(9.0),
                consistency: Some("consistent".to_owned()),
            }],
        };

        assert_eq!(
            table.default_backend_for("sequence.stats.v1"),
            Some(ExecutionBackend::Python),
            "a matching capability resolves its recorded default backend"
        );
        assert_eq!(
            table.default_backend_for("expression.pca.v1"),
            None,
            "a missing capability falls back to the native engine"
        );
        assert_eq!(
            RuntimePreferences::default().default_backend_for("sequence.stats.v1"),
            None,
            "an empty table resolves nothing"
        );
        let stale = RuntimePreferences {
            schema_version: 99,
            ..table.clone()
        };
        assert_eq!(
            stale.default_backend_for("sequence.stats.v1"),
            None,
            "an unrecognized schema version makes the table inert"
        );

        let serialized = serde_json::to_string(&table).expect("serialize");
        let parsed: RuntimePreferences = serde_json::from_str(&serialized).expect("round trip");
        assert_eq!(parsed, table);
    }

    #[test]
    fn parses_local_job_request() {
        let request: JobRequest = serde_json::from_str(
            r#"{
                "schema_version": "1",
                "job_id": "example",
                "capability": "sequence.stats.v1",
                "inputs": {"fasta": "sample.fa"},
                "execution": {"mode": "local-cpu"},
                "parameters": {}
            }"#,
        )
        .expect("valid request");

        assert_eq!(request.schema_version, SCHEMA_VERSION);
        assert_eq!(request.execution.mode, ExecutionMode::LocalCpu);
        assert_eq!(
            request.inputs.get("fasta").map(String::as_str),
            Some("sample.fa")
        );
    }

    #[test]
    fn legacy_result_round_trips_without_shape_changes() {
        let result = AnalysisResult::ok(
            "legacy-job",
            "sequence.stats.v1",
            serde_json::json!({"sequences": 2}),
            ExecutionMode::LocalCpu,
        );
        let json = serde_json::to_value(&result).expect("serialize legacy result");

        assert_eq!(json["schema_version"], SCHEMA_VERSION);
        assert_eq!(json["status"], "ok");
        assert_eq!(json["warnings"], serde_json::json!([]));

        let parsed: AnalysisResult<serde_json::Value> =
            serde_json::from_value(json).expect("parse legacy result");
        assert_eq!(parsed, result);
    }

    #[test]
    fn parses_artifact_aware_job_request() {
        let request: JobRequestV2 = serde_json::from_str(
            r#"{
                "schema_version": "2",
                "job_id": "fastq-qc",
                "capability": "sequence.fastq-qc.v1",
                "inputs": [{
                    "artifact_id": "reads",
                    "role": "reads",
                    "cardinality": "paired",
                    "dataset_id": "sample-1",
                    "files": [
                        {"file_id": "r1", "path": "reads_R1.fastq.gz", "role": "read-1", "format": "fastq", "compression": "gzip", "size_bytes": 100},
                        {"file_id": "r2", "path": "reads_R2.fastq.gz", "role": "read-2", "format": "fastq", "compression": "gzip", "size_bytes": 101}
                    ]
                }],
                "execution": {"mode": "local-cpu"},
                "parameters": {"quality_offset": 33}
            }"#,
        )
        .expect("valid v2 request");

        assert_eq!(request.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(request.inputs[0].cardinality, InputCardinality::Paired);
        assert!(request.inputs[0].has_valid_cardinality());
        assert_eq!(request.inputs[0].files[0].format, BioDataFormat::Fastq);
        assert_eq!(
            request.inputs[0].files[0].compression,
            CompressionFormat::Gzip
        );
    }

    #[test]
    fn dataset_manifest_preserves_relationships_and_diagnostics() {
        let manifest: DatasetManifest = serde_json::from_str(
            r#"{
                "schema_version": "1",
                "dataset_id": "sample-1",
                "display_name": "Sample 1",
                "created_at": "2026-07-22T08:00:00Z",
                "files": [
                    {"file_id": "r1", "path": "reads_R1.fastq.gz", "format": "fastq", "compression": "gzip", "size_bytes": 100},
                    {"file_id": "r2", "path": "reads_R2.fastq.gz", "format": "fastq", "compression": "gzip", "size_bytes": 101}
                ],
                "relationships": [{"kind": "paired-end", "members": ["r1", "r2"]}],
                "validation": {
                    "state": "valid-with-warnings",
                    "diagnostics": [{
                        "code": "FASTQ_PHRED_ASSUMED",
                        "severity": "warning",
                        "message": "Phred+33 was inferred",
                        "hint": "Confirm the sequencing platform"
                    }]
                }
            }"#,
        )
        .expect("valid dataset manifest");

        assert_eq!(
            manifest.validation.state,
            ValidationState::ValidWithWarnings
        );
        assert_eq!(
            manifest.relationships[0].kind,
            DatasetRelationshipKind::PairedEnd
        );
        assert_eq!(manifest.validation.diagnostics.len(), 1);
    }

    #[test]
    fn v2_result_supports_structured_artifacts() {
        let mut result = AnalysisResultV2::ok(
            "fastq-qc",
            "sequence.fastq-qc.v1",
            serde_json::json!({"reads": 42}),
            ExecutionMode::LocalCpu,
        );
        result.artifacts.push(super::OutputArtifact {
            artifact_id: "qc-table".to_owned(),
            role: "summary-table".to_owned(),
            kind: super::OutputArtifactKind::Table,
            path: "results/qc.csv".to_owned(),
            format: Some(BioDataFormat::Csv),
            media_type: Some("text/csv".to_owned()),
            size_bytes: Some(128),
            sha256: Some("a".repeat(64)),
            metadata: Default::default(),
        });

        let json = serde_json::to_string(&result).expect("serialize v2 result");
        let parsed: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&json).expect("parse v2 result");

        assert_eq!(parsed, result);
        assert_eq!(parsed.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(parsed.artifacts[0].format, Some(BioDataFormat::Csv));
    }

    #[test]
    fn v2_error_result_has_one_structured_diagnostic() {
        let result = AnalysisResultV2::error(
            "failed-job",
            "sequence.stats.v1",
            "job-failed",
            "input FASTA is missing",
            ExecutionMode::LocalCpu,
        );

        assert_eq!(result.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(result.job_id, "failed-job");
        assert_eq!(result.capability, "sequence.stats.v1");
        assert_eq!(result.status, JobStatus::Error);
        assert_eq!(result.result, serde_json::json!({}));
        assert!(result.artifacts.is_empty());
        assert_eq!(result.provenance.execution_mode, ExecutionMode::LocalCpu);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "job-failed");
        assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
    }

    #[test]
    fn workflow_pack_manifest_captures_runtime_and_permissions() {
        let manifest: WorkflowPackManifest = serde_json::from_str(
            r#"{
                "schema_version": "2",
                "id": "org.linxira.bulk-expression-deseq2",
                "version": "1.0.0",
                "publisher": {"name": "Linxira OS", "url": "https://linxira.org"},
                "license": "AGPL-3.0-or-later",
                "entrypoint": {"path": "workflow/run.R", "arguments": ["--request", "{request}"]},
                "runtime": {
                    "kind": "r",
                    "version": ">=4.4,<5",
                    "core_compatibility": ">=0.1.0,<1.0.0",
                    "dependency_lock": {"path": "renv.lock", "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
                },
                "input_schema": {"type": "object"},
                "output_schema": {"type": "object"},
                "platforms": ["windows-gnu", "debian", "arch"],
                "network": {"access": "none", "allowed_hosts": []},
                "resources": {"minimum_memory_mb": 4096, "recommended_threads": 4, "gpu": "none"},
                "files": [
                    {"path": "workflow/run.R", "sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
                    {"path": "renv.lock", "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
                ]
            }"#,
        )
        .expect("valid workflow pack manifest");

        assert_eq!(manifest.runtime.kind, WorkflowRuntimeKind::R);
        assert_eq!(manifest.runtime.core_compatibility, ">=0.1.0,<1.0.0");
        assert_eq!(manifest.network.access, NetworkAccess::None);
        assert_eq!(manifest.files.len(), 2);
    }

    #[test]
    fn v2_envelope_provenance_records_core_version() {
        let result = AnalysisResultV2::ok(
            "job-1",
            "sequence.stats.v1",
            serde_json::json!({"sequence_count": 3}),
            ExecutionMode::LocalCpu,
        );
        let serialized = serde_json::to_string(&result).expect("serialize envelope");
        let parsed: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("parse envelope");
        assert_eq!(
            parsed.provenance.core_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        // Legacy envelopes without core_version still parse.
        let legacy: AnalysisResultV2<serde_json::Value> = serde_json::from_str(
            r#"{"schema_version":"2","job_id":"job-1","capability":"sequence.stats.v1","status":"ok","result":{},"artifacts":[],"provenance":{"engine_version":"0.1.1","execution_mode":"local-cpu"},"diagnostics":[]}"#,
        )
        .expect("legacy envelope parses");
        assert_eq!(legacy.provenance.core_version, None);
    }

    #[test]
    fn benchmark_report_round_trips_without_shape_changes() {
        let report = BenchmarkReport {
            schema_version: BENCHMARK_REPORT_SCHEMA_VERSION.to_owned(),
            capability: "sequence.stats.v1".to_owned(),
            dataset_class: "sequence".to_owned(),
            backends: vec![BenchmarkBackendSummary {
                backend: "rust".to_owned(),
                repeats: 3,
                median_wall_ms: 12.0,
                min_wall_ms: 11.0,
                max_wall_ms: 14.0,
                iqr_wall_ms: 1.5,
                median_peak_rss_mb: Some(4.2),
                median_cpu_ms: Some(11.5),
                runs: vec![BenchmarkRun {
                    backend: "rust".to_owned(),
                    run_index: 1,
                    wall_ms: 12.0,
                    cpu_ms: Some(11.5),
                    peak_rss_mb: Some(4.2),
                    disk_read_bytes: Some(1024),
                    disk_write_bytes: None,
                    output_bytes: 2048,
                    ok: true,
                    error_summary: None,
                    self_reported_wall_ms: None,
                    self_reported_peak_rss_mb: None,
                }],
            }],
            speedup: None,
            memory_saving: None,
            consistency: BenchmarkVerdict::Consistent,
            findings: Vec::new(),
            environment: BenchmarkEnvironment {
                os: "linux".to_owned(),
                code_revision: Some("0123456789abcdef".to_owned()),
                gpu: None,
                memory: None,
                storage: None,
                kernel: Some("6.18.33".to_owned()),
                cpu_model: Some("test cpu".to_owned()),
                total_memory_mb: Some(16_000),
                distro: None,
                wsl_distro: None,
                wsl_version: None,
                host_os: None,
                host_model: None,
                host_cpu_model: None,
                host_logical_processors: None,
                host_total_memory_mb: None,
                engine_version: "1.0.1".to_owned(),
                python_version: None,
                r_version: None,
                in_container: false,
                page_cache: "warm".to_owned(),
                precision: "high".to_owned(),
            },
            sampled_at: "2026-09-10T00:00:00Z".to_owned(),
        };
        let json = serde_json::to_string(&report).expect("serialize benchmark report");
        let parsed: BenchmarkReport = serde_json::from_str(&json).expect("parse benchmark report");
        assert_eq!(parsed, report);
        assert_eq!(parsed.backends[0].runs[0].backend, "rust");
    }

    #[test]
    fn public_schema_documents_are_valid_json() {
        let schemas = [
            include_str!("../../../../schemas/job-request.schema.json"),
            include_str!("../../../../schemas/analysis-result.schema.json"),
            include_str!("../../../../schemas/artifact.schema.json"),
            include_str!("../../../../schemas/dataset-manifest.schema.json"),
            include_str!("../../../../schemas/job-request-v2.schema.json"),
            include_str!("../../../../schemas/analysis-result-v2.schema.json"),
            include_str!("../../../../schemas/workflow-pack-manifest.schema.json"),
        ];

        for schema in schemas {
            let parsed: serde_json::Value =
                serde_json::from_str(schema).expect("valid JSON schema");
            assert_eq!(
                parsed["$schema"],
                "https://json-schema.org/draft/2020-12/schema"
            );
        }
    }
}
