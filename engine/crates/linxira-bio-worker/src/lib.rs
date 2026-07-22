#![forbid(unsafe_code)]

//! Execute versioned Linxira Bio jobs through one shared local worker API.

use linxira_bio_core::dataset::{
    DatasetCompression, DatasetFormat, DatasetInspectionOptions, DetectionConfidence,
    inspect_dataset_with_options,
};
use linxira_bio_core::environment::{
    EnvironmentMode, EnvironmentPlanOptions, audit_environment, parse_environment_mode,
    plan_environment_with_options,
};
use linxira_bio_core::fastq::{
    DEFAULT_MAX_CYCLES, FastqQcOptions, QualityEncodingMode, fastq_qc_path,
};
use linxira_bio_core::sequence::fasta_stats_path;
use linxira_bio_core::variant::vcf_stats_path;
use linxira_bio_export::{ExportFormat, ensure_distinct_input_output, export_json_file};
use linxira_bio_protocol::{
    AnalysisResult, AnalysisResultV2, ArtifactFile, BioDataFormat, CompressionFormat, Diagnostic,
    DiagnosticSeverity, ExecutionMode, JobRequest, JobRequestV2, OutputArtifact,
    OutputArtifactKind, SCHEMA_VERSION, SCHEMA_VERSION_V2,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

pub type WorkerError = Box<dyn Error + Send + Sync>;
pub type WorkerResult<T> = Result<T, WorkerError>;

pub fn execute_path(request_path: &Path) -> WorkerResult<String> {
    // Parsing and typed deserialization happen before a reliable v2 identity exists. Failures at
    // this boundary remain process errors; semantic failures are enveloped by execute_request_v2.
    let request_file = File::open(request_path)?;
    let value: serde_json::Value = serde_json::from_reader(BufReader::new(request_file))?;
    let base_directory = request_path.parent().unwrap_or_else(|| Path::new("."));
    match value
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
    {
        Some(SCHEMA_VERSION) => execute_request(serde_json::from_value(value)?, base_directory),
        Some(SCHEMA_VERSION_V2) => {
            execute_request_v2(serde_json::from_value(value)?, base_directory)
        }
        Some(version) => Err(format!("unsupported job schema: {version}").into()),
        None => Err("job request requires schema_version".into()),
    }
}

pub fn execute_request(request: JobRequest, base_directory: &Path) -> WorkerResult<String> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(format!("unsupported job schema: {}", request.schema_version).into());
    }
    if request.execution.mode != ExecutionMode::LocalCpu {
        return Err("the current worker supports local-cpu execution only".into());
    }

    match request.capability.as_str() {
        "environment.audit.v1" => run_environment_audit(request),
        "environment.plan.v1" => run_environment_plan(base_directory, request),
        "dataset.inspect.v1" => run_dataset_inspection(base_directory, request),
        "fastq.qc.v1" => run_fastq_qc(base_directory, request),
        "table.export.v1" => run_table_export(base_directory, request),
        "sequence.stats.v1" => run_sequence_stats(base_directory, request),
        "variant.stats.v1" => run_variant_stats(base_directory, request),
        capability => Err(format!("unsupported capability: {capability}").into()),
    }
}

pub fn execute_request_v2(request: JobRequestV2, base_directory: &Path) -> WorkerResult<String> {
    if request.schema_version != SCHEMA_VERSION_V2 {
        return Err(format!("unsupported job schema: {}", request.schema_version).into());
    }
    if request.job_id.trim().is_empty() || request.capability.trim().is_empty() {
        return Err("v2 job request requires non-empty job_id and capability".into());
    }

    let job_id = request.job_id.clone();
    let capability = request.capability.clone();
    match execute_request_v2_inner(request, base_directory) {
        Ok(result) => Ok(result),
        Err(error) => Ok(serde_json::to_string(&AnalysisResultV2::error(
            job_id,
            capability,
            "job-failed",
            error.to_string(),
            ExecutionMode::LocalCpu,
        ))?),
    }
}

fn execute_request_v2_inner(request: JobRequestV2, base_directory: &Path) -> WorkerResult<String> {
    if request.execution.mode != ExecutionMode::LocalCpu {
        return Err("the current worker supports local-cpu execution only".into());
    }
    for input in &request.inputs {
        if !input.has_valid_cardinality() {
            return Err(format!(
                "input artifact {} does not match {:?} cardinality",
                input.artifact_id, input.cardinality
            )
            .into());
        }
    }
    let verified_inputs = validate_v2_inputs(&request, base_directory)?;

    match request.capability.as_str() {
        "environment.audit.v1" => {
            let audit = audit_environment()?;
            serialize_v2_result(&request, base_directory, &verified_inputs, audit)
        }
        "environment.plan.v1" => {
            let profile = request
                .parameters
                .get("profile")
                .map(|value| {
                    value
                        .as_str()
                        .ok_or("environment plan profile must be a string")
                })
                .transpose()?
                .unwrap_or("full-local");
            let mode = match request.parameters.get("mode") {
                Some(value) => parse_environment_mode(
                    value
                        .as_str()
                        .ok_or("environment plan mode must be a string")?,
                )?,
                None => EnvironmentMode::ManagedUser,
            };
            let project_root = request
                .parameters
                .get("project_root")
                .map(|value| {
                    value
                        .as_str()
                        .map(|path| resolve_input(base_directory, path))
                        .ok_or("environment plan project_root must be a string")
                })
                .transpose()?;
            if mode != EnvironmentMode::ProjectIsolated && project_root.is_some() {
                return Err("project_root is only valid in project-isolated mode".into());
            }
            let plan = plan_environment_with_options(
                profile,
                &audit_environment()?,
                &EnvironmentPlanOptions { mode, project_root },
            )?;
            serialize_v2_result(&request, base_directory, &verified_inputs, plan)
        }
        "dataset.inspect.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "file")?;
            let max_preview_records = optional_v2_usize_parameter(&request, "max_preview_records")?
                .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_RECORD_LIMIT);
            let max_preview_bytes = optional_v2_u64_parameter(&request, "max_preview_bytes")?
                .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_BYTE_LIMIT);
            let inspection = inspect_dataset_with_options(
                path,
                DatasetInspectionOptions {
                    max_preview_records,
                    max_preview_bytes,
                },
            )?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                inspection.clone(),
                ExecutionMode::LocalCpu,
            );
            result.diagnostics.extend(
                inspection
                    .warnings
                    .iter()
                    .map(|issue| inspection_diagnostic(issue, DiagnosticSeverity::Warning)),
            );
            result.diagnostics.extend(
                inspection
                    .errors
                    .iter()
                    .map(|issue| inspection_diagnostic(issue, DiagnosticSeverity::Error)),
            );
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "fastq.qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "fastq")?;
            let metrics = fastq_qc_path(path, fastq_options_v2(&request)?)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                metrics.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(metrics.warnings.iter().map(|message| Diagnostic {
                    code: "fastq-qc-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "table.export.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "table")?;
            let output = required_v2_string_parameter(&request, "output")?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let receipt = export_json_file(&input, &output)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                receipt.clone(),
                ExecutionMode::LocalCpu,
            );
            result.artifacts.push(OutputArtifact {
                artifact_id: "exported-table".to_owned(),
                role: "table".to_owned(),
                kind: OutputArtifactKind::Table,
                path: receipt.output_path,
                format: Some(export_bio_format(receipt.format)),
                media_type: Some(export_media_type(receipt.format).to_owned()),
                size_bytes: Some(receipt.size_bytes),
                sha256: Some(sha256_file(&output)?),
                metadata: Default::default(),
            });
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "sequence.stats.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "fasta")?;
            serialize_v2_result(
                &request,
                base_directory,
                &verified_inputs,
                fasta_stats_path(path)?,
            )
        }
        "variant.stats.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "vcf")?;
            let stats = vcf_stats_path(path)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                stats.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(stats.warnings.iter().map(|message| Diagnostic {
                    code: "variant-stats-warning".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: message.clone(),
                    artifact_id: None,
                    line: None,
                    record: None,
                    hint: None,
                }));
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        capability => Err(format!("unsupported capability: {capability}").into()),
    }
}

fn serialize_v2_result<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    value: T,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let mut result = AnalysisResultV2::ok(
        request.job_id.clone(),
        request.capability.clone(),
        value,
        ExecutionMode::LocalCpu,
    );
    finalize_v2_input_hashes(&mut result, request, base_directory, verified_inputs)?;
    Ok(serde_json::to_string(&result)?)
}

fn validate_v2_inputs(
    request: &JobRequestV2,
    base_directory: &Path,
) -> WorkerResult<BTreeMap<String, String>> {
    let mut file_ids = HashSet::new();
    let mut hashes = BTreeMap::new();
    for artifact in &request.inputs {
        for file in &artifact.files {
            if !file_ids.insert(file.file_id.clone()) {
                return Err(format!("duplicate input file_id: {}", file.file_id).into());
            }
            let path = resolve_input(base_directory, &file.path);
            let actual_size = std::fs::metadata(&path)?.len();
            if actual_size != file.size_bytes {
                return Err(format!(
                    "input {} size mismatch: request declares {} bytes but file has {} bytes",
                    file.file_id, file.size_bytes, actual_size
                )
                .into());
            }
            validate_v2_artifact_declaration(file, &path)?;
            let actual_hash = sha256_file(&path)?;
            if let Some(expected_hash) = &file.sha256
                && !actual_hash.eq_ignore_ascii_case(expected_hash)
            {
                return Err(format!(
                    "input {} SHA-256 mismatch: expected {} but found {}",
                    file.file_id, expected_hash, actual_hash
                )
                .into());
            }
            hashes.insert(file.file_id.clone(), actual_hash);
        }
    }
    Ok(hashes)
}

fn validate_v2_artifact_declaration(file: &ArtifactFile, path: &Path) -> WorkerResult<()> {
    let inspection = inspect_dataset_with_options(
        path,
        DatasetInspectionOptions {
            max_preview_records: 1,
            max_preview_bytes: 64 * 1024,
        },
    )?;

    if format_declaration_conflicts(file.format, inspection.format, inspection.confidence) {
        let declared_format = format!("{:?}", file.format).to_ascii_lowercase();
        return Err(format!(
            "input {} format mismatch: request declares {} but content identifies {}",
            file.file_id, declared_format, inspection.format
        )
        .into());
    }

    if compression_declaration_conflicts(file.compression, inspection.compression) {
        return Err(format!(
            "input {} compression mismatch: request declares {} but signature identifies {}",
            file.file_id,
            compression_format_name(file.compression),
            dataset_compression_name(inspection.compression)
        )
        .into());
    }

    Ok(())
}

fn format_declaration_conflicts(
    declared: BioDataFormat,
    actual: DatasetFormat,
    confidence: DetectionConfidence,
) -> bool {
    if declared == BioDataFormat::Unknown
        || actual == DatasetFormat::Unknown
        || matches!(
            confidence,
            DetectionConfidence::Low | DetectionConfidence::None
        )
    {
        return false;
    }

    match declared_dataset_format(declared) {
        Some(expected) => !dataset_formats_are_compatible(expected, actual),
        None if declared == BioDataFormat::Xlsx && actual == DatasetFormat::Zip => false,
        // Unsupported declarations are contradicted only by a strong, known content signature.
        None => confidence == DetectionConfidence::High,
    }
}

fn declared_dataset_format(format: BioDataFormat) -> Option<DatasetFormat> {
    Some(match format {
        BioDataFormat::Fasta => DatasetFormat::Fasta,
        BioDataFormat::Fastq => DatasetFormat::Fastq,
        BioDataFormat::Csv => DatasetFormat::Csv,
        BioDataFormat::Tsv => DatasetFormat::Tsv,
        BioDataFormat::Bed => DatasetFormat::Bed,
        BioDataFormat::Gff3 => DatasetFormat::Gff3,
        BioDataFormat::Gtf => DatasetFormat::Gtf,
        BioDataFormat::Vcf => DatasetFormat::Vcf,
        BioDataFormat::Sam => DatasetFormat::Sam,
        BioDataFormat::Bam => DatasetFormat::Bam,
        BioDataFormat::Bcf => DatasetFormat::Bcf,
        BioDataFormat::Cram => DatasetFormat::Cram,
        BioDataFormat::H5ad => DatasetFormat::H5ad,
        BioDataFormat::Loom => DatasetFormat::Loom,
        BioDataFormat::Hdf5 => DatasetFormat::Hdf5,
        BioDataFormat::Rds => DatasetFormat::Rds,
        BioDataFormat::Pdb => DatasetFormat::Pdb,
        BioDataFormat::Mmcif => DatasetFormat::Mmcif,
        BioDataFormat::Zip => DatasetFormat::Zip,
        BioDataFormat::Genbank
        | BioDataFormat::Embl
        | BioDataFormat::Xlsx
        | BioDataFormat::Json
        | BioDataFormat::Jsonl
        | BioDataFormat::Parquet
        | BioDataFormat::Unknown => return None,
    })
}

fn dataset_formats_are_compatible(declared: DatasetFormat, actual: DatasetFormat) -> bool {
    declared == actual
        || matches!(
            (declared, actual),
            (
                DatasetFormat::H5ad | DatasetFormat::Loom | DatasetFormat::Hdf5,
                DatasetFormat::H5ad | DatasetFormat::Loom | DatasetFormat::Hdf5
            )
        )
}

fn compression_declaration_conflicts(
    declared: CompressionFormat,
    actual: DatasetCompression,
) -> bool {
    match declared {
        CompressionFormat::Unknown => false,
        CompressionFormat::None => actual != DatasetCompression::None,
        CompressionFormat::Gzip => actual != DatasetCompression::Gzip,
        CompressionFormat::Bgzip => actual != DatasetCompression::Bgzip,
        CompressionFormat::Zip => actual != DatasetCompression::Zip,
        // These formats are valid protocol values, but the current inspector cannot verify them.
        // Reject the declaration instead of recording unverified compression provenance.
        CompressionFormat::Bzip2 | CompressionFormat::Xz | CompressionFormat::Zstd => true,
    }
}

fn compression_format_name(format: CompressionFormat) -> &'static str {
    match format {
        CompressionFormat::None => "none",
        CompressionFormat::Gzip => "gzip",
        CompressionFormat::Bgzip => "bgzip",
        CompressionFormat::Bzip2 => "bzip2",
        CompressionFormat::Xz => "xz",
        CompressionFormat::Zstd => "zstd",
        CompressionFormat::Zip => "zip",
        CompressionFormat::Unknown => "unknown",
    }
}

fn dataset_compression_name(compression: DatasetCompression) -> &'static str {
    match compression {
        DatasetCompression::None => "none",
        DatasetCompression::Gzip => "gzip",
        DatasetCompression::Bgzip => "bgzip",
        DatasetCompression::Zip => "zip",
    }
}

fn finalize_v2_input_hashes<T>(
    result: &mut AnalysisResultV2<T>,
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<()>
where
    T: serde::Serialize,
{
    for artifact in &request.inputs {
        for file in &artifact.files {
            let path = resolve_input(base_directory, &file.path);
            let final_hash = sha256_file(&path)?;
            let initial_hash = verified_inputs
                .get(&file.file_id)
                .ok_or_else(|| format!("input {} was not verified", file.file_id))?;
            if &final_hash != initial_hash {
                return Err(
                    format!("input {} changed while the job was running", file.file_id).into(),
                );
            }
        }
    }
    result.provenance.input_sha256 = verified_inputs.clone();
    Ok(())
}

fn sha256_file(path: &Path) -> WorkerResult<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    let mut encoded = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(&mut encoded, "{byte:02x}").expect("write to String");
    }
    Ok(encoded)
}

fn resolve_v2_single_input(
    base_directory: &Path,
    request: &JobRequestV2,
    role: &str,
) -> WorkerResult<PathBuf> {
    let artifact = request
        .inputs
        .iter()
        .find(|artifact| artifact.role == role)
        .ok_or_else(|| {
            format!(
                "{} requires an input artifact with role {role}",
                request.capability
            )
        })?;
    if artifact.files.len() != 1 {
        return Err(format!("input role {role} requires exactly one file").into());
    }
    Ok(resolve_input(base_directory, &artifact.files[0].path))
}

fn ensure_v2_export_output_is_distinct(
    request: &JobRequestV2,
    base_directory: &Path,
    output: &Path,
) -> WorkerResult<()> {
    for artifact in &request.inputs {
        for file in &artifact.files {
            ensure_distinct_input_output(&resolve_input(base_directory, &file.path), output)?;
        }
    }
    Ok(())
}

fn inspection_diagnostic(
    issue: &linxira_bio_core::dataset::InspectionIssue,
    severity: DiagnosticSeverity,
) -> Diagnostic {
    Diagnostic {
        code: issue.code.clone(),
        severity,
        message: issue.message.clone(),
        artifact_id: None,
        line: issue.line,
        record: None,
        hint: None,
    }
}

fn optional_v2_u64_parameter(request: &JobRequestV2, key: &str) -> WorkerResult<Option<u64>> {
    match request.parameters.get(key) {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer").into()),
        None => Ok(None),
    }
}

fn optional_v2_usize_parameter(request: &JobRequestV2, key: &str) -> WorkerResult<Option<usize>> {
    optional_v2_u64_parameter(request, key)?
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| format!("{key} exceeds this platform's size limit").into())
        })
        .transpose()
}

fn run_dataset_inspection(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("file")
        .ok_or("dataset.inspect.v1 requires inputs.file")?;
    let max_preview_records = optional_usize_parameter(&request, "max_preview_records")?
        .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_RECORD_LIMIT);
    let max_preview_bytes = optional_u64_parameter(&request, "max_preview_bytes")?
        .unwrap_or(linxira_bio_core::dataset::DEFAULT_PREVIEW_BYTE_LIMIT);
    let inspection = inspect_dataset_with_options(
        resolve_input(base_directory, input),
        DatasetInspectionOptions {
            max_preview_records,
            max_preview_bytes,
        },
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        inspection.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = inspection
        .warnings
        .iter()
        .map(|warning| warning.message.clone())
        .collect();
    Ok(serde_json::to_string(&result)?)
}

fn run_table_export(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("json")
        .ok_or("table.export.v1 requires inputs.json")?;
    let output = request
        .parameters
        .get("output")
        .and_then(serde_json::Value::as_str)
        .ok_or("table.export.v1 requires string parameters.output")?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    for declared_input in request.inputs.values() {
        ensure_distinct_input_output(&resolve_input(base_directory, declared_input), &output)?;
    }
    let receipt = export_json_file(&input, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        receipt,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_fastq_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("fastq")
        .ok_or("fastq.qc.v1 requires inputs.fastq")?;
    let metrics = fastq_qc_path(
        resolve_input(base_directory, input),
        fastq_options_v1(&request)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_variant_stats(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("vcf")
        .ok_or("variant.stats.v1 requires inputs.vcf")?;
    let stats = vcf_stats_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = stats.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn fastq_options_v1(request: &JobRequest) -> WorkerResult<FastqQcOptions> {
    Ok(FastqQcOptions {
        max_cycles: optional_usize_parameter(request, "max_cycles")?.unwrap_or(DEFAULT_MAX_CYCLES),
        quality_encoding: parse_quality_encoding(request.parameters.get("quality_encoding"))?,
    })
}

fn fastq_options_v2(request: &JobRequestV2) -> WorkerResult<FastqQcOptions> {
    Ok(FastqQcOptions {
        max_cycles: optional_v2_usize_parameter(request, "max_cycles")?
            .unwrap_or(DEFAULT_MAX_CYCLES),
        quality_encoding: parse_quality_encoding(request.parameters.get("quality_encoding"))?,
    })
}

fn parse_quality_encoding(value: Option<&serde_json::Value>) -> WorkerResult<QualityEncodingMode> {
    match value.and_then(serde_json::Value::as_str).unwrap_or("auto") {
        "auto" => Ok(QualityEncodingMode::Auto),
        "phred+33" => Ok(QualityEncodingMode::Phred33),
        "phred+64" => Ok(QualityEncodingMode::Phred64),
        value => Err(format!(
            "unsupported quality_encoding {value:?}; expected auto, phred+33, or phred+64"
        )
        .into()),
    }
}

fn required_v2_string_parameter<'a>(request: &'a JobRequestV2, key: &str) -> WorkerResult<&'a str> {
    request
        .parameters
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{} requires string parameters.{key}", request.capability).into())
}

fn export_bio_format(format: ExportFormat) -> BioDataFormat {
    match format {
        ExportFormat::Csv => BioDataFormat::Csv,
        ExportFormat::Tsv => BioDataFormat::Tsv,
        ExportFormat::Json => BioDataFormat::Json,
        ExportFormat::Jsonl => BioDataFormat::Jsonl,
        ExportFormat::Xlsx => BioDataFormat::Xlsx,
    }
}

fn export_media_type(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv => "text/csv",
        ExportFormat::Tsv => "text/tab-separated-values",
        ExportFormat::Json => "application/json",
        ExportFormat::Jsonl => "application/x-ndjson",
        ExportFormat::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    }
}

fn run_environment_audit(request: JobRequest) -> WorkerResult<String> {
    let audit = audit_environment()?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        audit,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_environment_plan(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let profile = match request.parameters.get("profile") {
        Some(value) => value
            .as_str()
            .ok_or("environment plan profile must be a string")?,
        None => "full-local",
    };
    let mode = match request.parameters.get("mode") {
        Some(value) => parse_environment_mode(
            value
                .as_str()
                .ok_or("environment plan mode must be a string")?,
        )?,
        None => EnvironmentMode::ManagedUser,
    };
    let project_root = match request.parameters.get("project_root") {
        Some(value) => Some(resolve_input(
            base_directory,
            value
                .as_str()
                .ok_or("environment plan project_root must be a string")?,
        )),
        None => None,
    };
    if mode != EnvironmentMode::ProjectIsolated && project_root.is_some() {
        return Err("project_root is only valid in project-isolated mode".into());
    }
    let audit = audit_environment()?;
    let plan = plan_environment_with_options(
        profile,
        &audit,
        &EnvironmentPlanOptions { mode, project_root },
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        plan,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_stats(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("fasta")
        .ok_or("sequence.stats.v1 requires inputs.fasta")?;
    let input_path = resolve_input(base_directory, input);
    let stats = fasta_stats_path(input_path)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn optional_u64_parameter(request: &JobRequest, key: &str) -> WorkerResult<Option<u64>> {
    match request.parameters.get(key) {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer").into()),
        None => Ok(None),
    }
}

fn optional_usize_parameter(request: &JobRequest, key: &str) -> WorkerResult<Option<usize>> {
    optional_u64_parameter(request, key)?
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| format!("{key} exceeds this platform's size limit").into())
        })
        .transpose()
}

fn resolve_input(base_directory: &Path, input: &str) -> PathBuf {
    let input_path = PathBuf::from(input);
    if input_path.is_absolute() {
        input_path
    } else {
        base_directory.join(input_path)
    }
}

#[cfg(test)]
mod tests {
    use super::{execute_request, execute_request_v2, validate_v2_inputs};
    use linxira_bio_protocol::{
        AnalysisResultV2, ArtifactFile, BioDataFormat, CompressionFormat, DiagnosticSeverity,
        ExecutionMode, ExecutionRequest, InputArtifact, InputCardinality, JobRequest, JobRequestV2,
        JobStatus, SCHEMA_VERSION, SCHEMA_VERSION_V2,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn rejects_non_string_environment_mode() {
        let error = execute_request(
            environment_plan_request(serde_json::json!({"mode": 42})),
            Path::new("."),
        )
        .expect_err("invalid mode must fail");

        assert!(error.to_string().contains("mode must be a string"));
    }

    #[test]
    fn rejects_project_root_outside_project_mode() {
        let error = execute_request(
            environment_plan_request(serde_json::json!({
                "mode": "managed-user",
                "project_root": "."
            })),
            Path::new("."),
        )
        .expect_err("unexpected project root must fail");

        assert!(error.to_string().contains("only valid in project-isolated"));
    }

    #[test]
    fn v2_execution_failure_returns_an_error_envelope() {
        let request: JobRequestV2 = serde_json::from_value(serde_json::json!({
            "schema_version": "2",
            "job_id": "unsupported-capability-test",
            "capability": "unknown.operation.v1",
            "inputs": [],
            "execution": {"mode": "local-cpu"},
            "parameters": {}
        }))
        .expect("typed v2 request");

        let json = execute_request_v2(request, Path::new("."))
            .expect("failure must use the v2 result transport");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&json).expect("v2 error result");

        assert_eq!(result.job_id, "unsupported-capability-test");
        assert_eq!(result.capability, "unknown.operation.v1");
        assert_eq!(result.status, JobStatus::Error);
        assert_eq!(result.result, serde_json::json!({}));
        assert!(result.artifacts.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, "job-failed");
        assert_eq!(result.diagnostics[0].severity, DiagnosticSeverity::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("unsupported capability")
        );
    }

    #[test]
    fn rejects_v2_fasta_and_vcf_format_mismatches() {
        let cases: [(&str, &[u8], BioDataFormat, &str); 3] = [
            (
                "actual-fasta.fa",
                b">sequence\nACGT\n",
                BioDataFormat::Vcf,
                "content identifies fasta",
            ),
            (
                "actual-variants.vcf",
                b"##fileformat=VCFv4.3\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\n",
                BioDataFormat::Fasta,
                "content identifies vcf",
            ),
            (
                "fasta-declared-json.fa",
                b">sequence\nACGT\n",
                BioDataFormat::Json,
                "content identifies fasta",
            ),
        ];

        for (name, contents, declared_format, expected_message) in cases {
            let path = write_temporary(name, contents);
            let request = artifact_request(
                &path,
                declared_format,
                CompressionFormat::None,
                "dataset.inspect.v1",
                "file",
            );
            let error = validate_v2_inputs(&request, Path::new("."))
                .expect_err("format mismatch must fail validation");
            fs::remove_file(&path).expect("remove format fixture");

            assert!(error.to_string().contains("format mismatch"), "{name}");
            assert!(error.to_string().contains(expected_message), "{name}");
        }
    }

    #[test]
    fn rejects_v2_unverifiable_and_detected_compression_mismatches() {
        let gzip_signature = [0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0];
        let cases: [(&str, &[u8], CompressionFormat, &str); 5] = [
            (
                "plain.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Gzip,
                "signature identifies none",
            ),
            (
                "compressed.data",
                &gzip_signature,
                CompressionFormat::None,
                "signature identifies gzip",
            ),
            (
                "claimed-bzip2.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Bzip2,
                "signature identifies none",
            ),
            (
                "claimed-xz.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Xz,
                "signature identifies none",
            ),
            (
                "claimed-zstd.fa",
                b">sequence\nACGT\n",
                CompressionFormat::Zstd,
                "signature identifies none",
            ),
        ];

        for (name, contents, declared_compression, expected_message) in cases {
            let path = write_temporary(name, contents);
            let request = artifact_request(
                &path,
                BioDataFormat::Unknown,
                declared_compression,
                "dataset.inspect.v1",
                "file",
            );
            let error = validate_v2_inputs(&request, Path::new("."))
                .expect_err("compression mismatch must fail validation");
            fs::remove_file(&path).expect("remove compression fixture");

            assert!(error.to_string().contains("compression mismatch"), "{name}");
            assert!(error.to_string().contains(expected_message), "{name}");
        }
    }

    #[test]
    fn v2_unknown_declarations_and_unknown_detection_are_non_blocking() {
        let fasta = write_temporary("known.fa", b">sequence\nACGT\n");
        let unknown_declaration = artifact_request(
            &fasta,
            BioDataFormat::Unknown,
            CompressionFormat::Unknown,
            "dataset.inspect.v1",
            "file",
        );
        validate_v2_inputs(&unknown_declaration, Path::new("."))
            .expect("unknown declarations are wildcards");
        fs::remove_file(&fasta).expect("remove known fixture");

        let opaque = write_temporary("opaque.fa", b"one opaque line\n");
        let unknown_detection = artifact_request(
            &opaque,
            BioDataFormat::Vcf,
            CompressionFormat::None,
            "dataset.inspect.v1",
            "file",
        );
        validate_v2_inputs(&unknown_detection, Path::new("."))
            .expect("extension-only detection does not contradict a declaration");
        fs::remove_file(&opaque).expect("remove opaque fixture");
    }

    #[test]
    fn v2_json_table_export_is_not_blocked_by_unknown_format_detection() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/results/metrics.json")
            .canonicalize()
            .expect("metrics fixture");
        let output = temporary_path("export.csv");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Json,
            CompressionFormat::None,
            "table.export.v1",
            "table",
        );
        request.parameters = serde_json::json!({"output": output});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("JSON table export remains executable");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid table export result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "table.export.v1");
        assert!(fs::metadata(&output).expect("exported table").len() > 0);
        fs::remove_file(output).expect("remove exported table");
    }

    #[test]
    fn v2_table_export_refuses_to_replace_any_declared_input() {
        let table = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/results/metrics.json")
            .canonicalize()
            .expect("metrics fixture");
        let protected = write_temporary("protected.json", br#"{"protected":true}"#);
        let mut request = artifact_request(
            &table,
            BioDataFormat::Json,
            CompressionFormat::None,
            "table.export.v1",
            "table",
        );
        request.inputs.push(InputArtifact {
            artifact_id: "protected-artifact".to_owned(),
            role: "metadata".to_owned(),
            cardinality: InputCardinality::Single,
            files: vec![ArtifactFile {
                file_id: "protected-file".to_owned(),
                path: protected.to_string_lossy().into_owned(),
                role: None,
                format: BioDataFormat::Json,
                compression: CompressionFormat::None,
                size_bytes: fs::metadata(&protected).expect("protected metadata").len(),
                modified_at: None,
                sha256: None,
            }],
            dataset_id: None,
        });
        request.parameters = serde_json::json!({"output": protected});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("worker returns a v2 error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(result.diagnostics[0].message.contains("must be different"));
        assert_eq!(
            fs::read_to_string(&protected).expect("declared input remains readable"),
            r#"{"protected":true}"#
        );
        fs::remove_file(protected).expect("remove protected input");
    }

    fn environment_plan_request(parameters: serde_json::Value) -> JobRequest {
        JobRequest {
            schema_version: SCHEMA_VERSION.to_owned(),
            job_id: "environment-plan-test".to_owned(),
            capability: "environment.plan.v1".to_owned(),
            inputs: BTreeMap::new(),
            execution: ExecutionRequest {
                mode: ExecutionMode::LocalCpu,
            },
            parameters,
        }
    }

    fn artifact_request(
        path: &Path,
        format: BioDataFormat,
        compression: CompressionFormat,
        capability: &str,
        role: &str,
    ) -> JobRequestV2 {
        JobRequestV2 {
            schema_version: SCHEMA_VERSION_V2.to_owned(),
            job_id: "artifact-validation-test".to_owned(),
            capability: capability.to_owned(),
            inputs: vec![InputArtifact {
                artifact_id: "input-artifact".to_owned(),
                role: role.to_owned(),
                cardinality: InputCardinality::Single,
                files: vec![ArtifactFile {
                    file_id: "input-file".to_owned(),
                    path: path.to_string_lossy().into_owned(),
                    role: None,
                    format,
                    compression,
                    size_bytes: fs::metadata(path).expect("input metadata").len(),
                    modified_at: None,
                    sha256: None,
                }],
                dataset_id: None,
            }],
            execution: ExecutionRequest {
                mode: ExecutionMode::LocalCpu,
            },
            parameters: serde_json::json!({}),
        }
    }

    fn write_temporary(name: &str, contents: &[u8]) -> PathBuf {
        let path = temporary_path(name);
        fs::write(&path, contents).expect("write artifact fixture");
        path
    }

    fn temporary_path(name: &str) -> PathBuf {
        let counter = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "linxira-bio-worker-artifact-{}-{counter}-{name}",
            std::process::id()
        ))
    }
}
