#![forbid(unsafe_code)]

//! Stable job and result contracts shared by CLI, GUI, workers, and agents.

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
    Bed,
    Gff3,
    Gtf,
    Vcf,
    Sam,
    Bam,
    Bcf,
    Cram,
    Genbank,
    Embl,
    H5ad,
    Loom,
    Hdf5,
    Rds,
    Pdb,
    Mmcif,
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
        started_at: None,
        finished_at: None,
        software: Vec::new(),
        input_sha256: BTreeMap::new(),
        command: None,
        dependency_lock_sha256: None,
    }
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
pub struct WorkflowRuntime {
    pub kind: WorkflowRuntimeKind,
    pub version: String,
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
        AnalysisResult, AnalysisResultV2, BioDataFormat, CompressionFormat, DatasetManifest,
        DatasetRelationshipKind, DiagnosticSeverity, ExecutionMode, InputCardinality, JobRequest,
        JobRequestV2, JobStatus, NetworkAccess, SCHEMA_VERSION, SCHEMA_VERSION_V2, ValidationState,
        WorkflowPackManifest, WorkflowRuntimeKind,
    };

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
                "schema_version": "1",
                "id": "org.linxira.bulk-expression-deseq2",
                "version": "1.0.0",
                "publisher": {"name": "Linxira OS", "url": "https://linxira.org"},
                "license": "AGPL-3.0-or-later",
                "entrypoint": {"path": "workflow/run.R", "arguments": ["--request", "{request}"]},
                "runtime": {
                    "kind": "r",
                    "version": ">=4.4,<5",
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
        assert_eq!(manifest.network.access, NetworkAccess::None);
        assert_eq!(manifest.files.len(), 2);
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
