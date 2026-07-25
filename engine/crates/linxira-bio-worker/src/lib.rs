#![forbid(unsafe_code)]

//! Execute versioned Linxira Bio jobs through one shared local worker API.

use linxira_bio_core::alignment::sam_qc_path;
use linxira_bio_core::dataset::{
    DatasetCompression, DatasetFormat, DatasetInspectionOptions, DetectionConfidence,
    inspect_dataset_with_options,
};
use linxira_bio_core::environment::{
    EnvironmentMode, EnvironmentPlanOptions, audit_environment, parse_environment_mode,
    plan_environment_with_options,
};
use linxira_bio_core::expression::expression_matrix_qc_path;
use linxira_bio_core::fastq::{
    DEFAULT_MAX_CYCLES, FastqQcOptions, QualityEncodingMode, fastq_qc_path,
};
use linxira_bio_core::fastq_transform::{
    DEFAULT_ADAPTER_MIN_OVERLAP, DEFAULT_MIN_LENGTH, DEFAULT_TRIM_QUALITY, FastqAdapterOptions,
    FastqTransformError, FastqTransformQualityEncoding, FastqTrimOptions, fastq_adapter_trim_path,
    fastq_trim_path,
};
use linxira_bio_core::interval::{
    IntervalMergeOptions, bed_intersect_path, bed_merge_path, bed_subtract_path,
};
use linxira_bio_core::sequence::fasta_stats_path;
use linxira_bio_core::sequence_transform::{
    SequenceExtractOptions, SequenceFilterOptions, SequenceFromTableOptions,
    SequenceIdNormalizeOptions, SequenceMergeOptions, SequenceOrfOptions, SequenceSplitOptions,
    SequenceTableDelimiter, SequenceToTableOptions, SequenceTransformError,
    SequenceTranslateOptions, extract_fasta_path, fasta_to_table_path, filter_fasta_path,
    find_orfs_fasta_path, merge_fasta_paths, normalize_fasta_ids_path, parse_sequence_region_spec,
    reverse_complement_fasta_path, split_fasta_path, table_to_fasta_path, translate_fasta_path,
};
use linxira_bio_core::structure::{PdbSummaryOptions, pdb_summary_path};
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
        "alignment.qc.v1" => run_alignment_qc(base_directory, request),
        "environment.audit.v1" => run_environment_audit(request),
        "environment.plan.v1" => run_environment_plan(base_directory, request),
        "dataset.inspect.v1" => run_dataset_inspection(base_directory, request),
        "fastq.qc.v1" => run_fastq_qc(base_directory, request),
        "fastq.trim.v1" => run_fastq_trim(base_directory, request),
        "fastq.adapter.v1" => run_fastq_adapter_trim(base_directory, request),
        "expression.matrix.qc.v1" => run_expression_matrix_qc(base_directory, request),
        "interval.intersect.v1" => run_interval_intersect(base_directory, request),
        "interval.merge.v1" => run_interval_merge(base_directory, request),
        "interval.subtract.v1" => run_interval_subtract(base_directory, request),
        "table.export.v1" => run_table_export(base_directory, request),
        "sequence.extract.v1" => run_sequence_extract(base_directory, request),
        "sequence.filter.v1" => run_sequence_filter(base_directory, request),
        "sequence.reverse-complement.v1" => {
            run_sequence_reverse_complement(base_directory, request)
        }
        "sequence.stats.v1" => run_sequence_stats(base_directory, request),
        "sequence.translate.v1" => run_sequence_translate(base_directory, request),
        "sequence.orf.v1" => run_sequence_orf(base_directory, request),
        "sequence.id.normalize.v1" => run_sequence_id_normalize(base_directory, request),
        "sequence.merge.v1" => run_sequence_merge(base_directory, request),
        "sequence.split.v1" => run_sequence_split(base_directory, request),
        "sequence.to-table.v1" => run_sequence_to_table(base_directory, request),
        "sequence.from-table.v1" => run_sequence_from_table(base_directory, request),
        "structure.pdb.summary.v1" => run_pdb_summary(base_directory, request),
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
    validate_v2_contract(&request)?;
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
        "alignment.qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "sam")?;
            let metrics = sam_qc_path(path)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                metrics.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(metrics.warnings.iter().map(|message| Diagnostic {
                    code: "alignment-qc-warning".to_owned(),
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
        "fastq.trim.v1" => {
            let options = fastq_trim_options(&request.parameters)?;
            execute_fastq_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| fastq_trim_path(input, output, &options),
            )
        }
        "fastq.adapter.v1" => {
            let options = fastq_adapter_options(&request.parameters)?;
            execute_fastq_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| fastq_adapter_trim_path(input, output, &options),
            )
        }
        "expression.matrix.qc.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "matrix")?;
            let metrics = expression_matrix_qc_path(path)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                metrics.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(metrics.warnings.iter().map(|message| Diagnostic {
                    code: "expression-matrix-qc-warning".to_owned(),
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
        "interval.intersect.v1" => {
            let left = resolve_v2_single_input(base_directory, &request, "left-bed")?;
            let right = resolve_v2_single_input(base_directory, &request, "right-bed")?;
            let stats = bed_intersect_path(left, right)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                stats.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(stats.warnings.iter().map(|message| Diagnostic {
                    code: "interval-intersect-warning".to_owned(),
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
        "interval.merge.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "bed")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let stats = bed_merge_path(
                input,
                &output,
                IntervalMergeOptions {
                    max_gap: optional_parameter_u64(&request.parameters, "max_gap")?.unwrap_or(0),
                },
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                stats,
                FileArtifactSpec {
                    artifact_id: "interval-output",
                    role: "bed",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Bed),
                    media_type: Some("text/x-bed"),
                },
            )
        }
        "interval.subtract.v1" => {
            let left = resolve_v2_single_input(base_directory, &request, "left-bed")?;
            let right = resolve_v2_single_input(base_directory, &request, "right-bed")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let stats = bed_subtract_path(left, right, &output)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                stats,
                FileArtifactSpec {
                    artifact_id: "interval-output",
                    role: "bed",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Bed),
                    media_type: Some("text/x-bed"),
                },
            )
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
        "sequence.extract.v1" => {
            let options = sequence_extract_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| extract_fasta_path(input, output, &options),
            )
        }
        "sequence.filter.v1" => {
            let options = sequence_filter_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| filter_fasta_path(input, output, &options),
            )
        }
        "sequence.reverse-complement.v1" => execute_sequence_transform_v2(
            &request,
            base_directory,
            &verified_inputs,
            |input, output| reverse_complement_fasta_path(input, output),
        ),
        "sequence.translate.v1" => {
            let options = sequence_translate_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| translate_fasta_path(input, output, &options),
            )
        }
        "sequence.orf.v1" => {
            let options = sequence_orf_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| find_orfs_fasta_path(input, output, &options),
            )
        }
        "sequence.id.normalize.v1" => {
            let options = sequence_id_normalize_options(&request.parameters)?;
            execute_sequence_transform_v2(
                &request,
                base_directory,
                &verified_inputs,
                |input, output| normalize_fasta_ids_path(input, output, &options),
            )
        }
        "sequence.merge.v1" => {
            let inputs = resolve_v2_input_files(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let options = sequence_merge_options(&request.parameters)?;
            let summary = merge_fasta_paths(&inputs, &output, &options)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "sequence-output",
                    role: "fasta",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "sequence.split.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output_directory = request
                .parameters
                .get("output_directory")
                .and_then(serde_json::Value::as_str)
                .ok_or("sequence.split.v1 requires string parameters.output_directory")?;
            let output_directory = resolve_input(base_directory, output_directory);
            let options = sequence_split_options(&request.parameters)?;
            let summary = split_fasta_path(input, &output_directory, &options)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                summary.clone(),
                ExecutionMode::LocalCpu,
            );
            result.artifacts.push(OutputArtifact {
                artifact_id: "sequence-output-directory".to_owned(),
                role: "fasta-directory".to_owned(),
                kind: OutputArtifactKind::Directory,
                path: output_directory.to_string_lossy().into_owned(),
                format: None,
                media_type: None,
                size_bytes: None,
                sha256: None,
                metadata: BTreeMap::from([(
                    "file_count".to_owned(),
                    serde_json::json!(summary.output_files),
                )]),
            });
            finalize_v2_input_hashes(&mut result, &request, base_directory, &verified_inputs)?;
            Ok(serde_json::to_string(&result)?)
        }
        "sequence.to-table.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "fasta")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let delimiter =
                sequence_table_delimiter_option(&request.parameters)?.unwrap_or_else(|| {
                    SequenceTableDelimiter::infer_from_path(&output)
                        .unwrap_or(SequenceTableDelimiter::Csv)
                });
            let summary = fasta_to_table_path(
                input,
                &output,
                &SequenceToTableOptions {
                    delimiter,
                    include_header: optional_parameter_bool(&request.parameters, "include_header")?
                        .unwrap_or(true),
                },
            )?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "sequence-table",
                    role: "table",
                    kind: OutputArtifactKind::Table,
                    path: output,
                    format: Some(sequence_table_format(delimiter)),
                    media_type: Some(delimiter.media_type()),
                },
            )
        }
        "sequence.from-table.v1" => {
            let input = resolve_v2_single_input(base_directory, &request, "table")?;
            let output = required_sequence_output(&request.parameters, &request.capability)?;
            let output = resolve_input(base_directory, output);
            ensure_v2_export_output_is_distinct(&request, base_directory, &output)?;
            let mut options = sequence_from_table_options(&request.parameters)?;
            if sequence_table_delimiter_option(&request.parameters)?.is_none() {
                options.delimiter = SequenceTableDelimiter::infer_from_path(&input)
                    .unwrap_or(SequenceTableDelimiter::Csv);
            }
            let summary = table_to_fasta_path(input, &output, &options)?;
            serialize_v2_file_artifact_result(
                &request,
                base_directory,
                &verified_inputs,
                summary,
                FileArtifactSpec {
                    artifact_id: "sequence-output",
                    role: "fasta",
                    kind: OutputArtifactKind::DomainFile,
                    path: output,
                    format: Some(BioDataFormat::Fasta),
                    media_type: Some("text/x-fasta"),
                },
            )
        }
        "structure.pdb.summary.v1" => {
            let path = resolve_v2_single_input(base_directory, &request, "pdb")?;
            let summary = pdb_summary_path(path, pdb_options(&request.parameters)?)?;
            let mut result = AnalysisResultV2::ok(
                request.job_id.clone(),
                request.capability.clone(),
                summary.clone(),
                ExecutionMode::LocalCpu,
            );
            result
                .diagnostics
                .extend(summary.warnings.iter().map(|message| Diagnostic {
                    code: "pdb-summary-warning".to_owned(),
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

fn validate_v2_contract(request: &JobRequestV2) -> WorkerResult<()> {
    let (required_roles, allowed_parameters): (&[&str], &[&str]) = match request.capability.as_str()
    {
        "alignment.qc.v1" => (&["sam"], &[]),
        "environment.audit.v1" => (&[], &[]),
        "environment.plan.v1" => (&[], &["profile", "mode", "project_root"]),
        "dataset.inspect.v1" => (&["file"], &["max_preview_records", "max_preview_bytes"]),
        "fastq.qc.v1" => (&["fastq"], &["max_cycles", "quality_encoding"]),
        "fastq.trim.v1" => (
            &["fastq"],
            &["output", "min_quality", "min_length", "quality_encoding"],
        ),
        "fastq.adapter.v1" => (
            &["fastq"],
            &["output", "adapter", "adapters", "min_overlap", "min_length"],
        ),
        "expression.matrix.qc.v1" => (&["matrix"], &[]),
        "interval.intersect.v1" => (&["left-bed", "right-bed"], &[]),
        "interval.merge.v1" => (&["bed"], &["output", "max_gap"]),
        "interval.subtract.v1" => (&["left-bed", "right-bed"], &["output"]),
        "table.export.v1" => (&["table"], &["output"]),
        "sequence.stats.v1" => (&["fasta"], &[]),
        "sequence.extract.v1" => (&["fasta"], &["output", "identifiers", "regions", "strict"]),
        "sequence.filter.v1" => (
            &["fasta"],
            &[
                "output",
                "min_length",
                "max_length",
                "min_gc_percent",
                "max_gc_percent",
                "max_n_percent",
            ],
        ),
        "sequence.reverse-complement.v1" => (&["fasta"], &["output"]),
        "sequence.translate.v1" => (
            &["fasta"],
            &["output", "frames", "trim_terminal_stop", "stop_at_first"],
        ),
        "sequence.orf.v1" => (
            &["fasta"],
            &[
                "output",
                "min_amino_acids",
                "include_reverse_strand",
                "include_partial_3prime",
            ],
        ),
        "sequence.id.normalize.v1" => (
            &["fasta"],
            &["output", "prefix", "start", "width", "keep_description"],
        ),
        "sequence.merge.v1" => (&["fasta"], &["output", "allow_duplicate_ids"]),
        "sequence.split.v1" => (
            &["fasta"],
            &["output_directory", "records_per_file", "prefix"],
        ),
        "sequence.to-table.v1" => (&["fasta"], &["output", "delimiter", "include_header"]),
        "sequence.from-table.v1" => (
            &["table"],
            &[
                "output",
                "delimiter",
                "id_column",
                "sequence_column",
                "description_column",
            ],
        ),
        "structure.pdb.summary.v1" => (&["pdb"], &["interpret_b_factors_as_plddt"]),
        "variant.stats.v1" => (&["vcf"], &[]),
        capability => return Err(format!("unsupported capability: {capability}").into()),
    };

    let mut artifact_ids = HashSet::new();
    let mut roles = HashSet::new();
    for artifact in &request.inputs {
        if artifact.artifact_id.trim().is_empty() || artifact.role.trim().is_empty() {
            return Err("v2 input artifacts require non-empty artifact_id and role".into());
        }
        if !artifact_ids.insert(artifact.artifact_id.as_str()) {
            return Err(format!("duplicate input artifact_id: {}", artifact.artifact_id).into());
        }
        if !roles.insert(artifact.role.as_str()) {
            return Err(format!("duplicate input role: {}", artifact.role).into());
        }
    }
    for role in required_roles {
        if !roles.contains(role) {
            return Err(format!("{} requires input role {role}", request.capability).into());
        }
    }
    for role in roles {
        if !required_roles.contains(&role) {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }

    let parameters = match &request.parameters {
        serde_json::Value::Null => return Ok(()),
        serde_json::Value::Object(parameters) => parameters,
        _ => return Err("v2 parameters must be an object".into()),
    };
    for parameter in parameters.keys() {
        if !allowed_parameters.contains(&parameter.as_str()) {
            return Err(format!(
                "{} does not accept parameter {parameter}",
                request.capability
            )
            .into());
        }
    }
    Ok(())
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

struct FileArtifactSpec {
    artifact_id: &'static str,
    role: &'static str,
    kind: OutputArtifactKind,
    path: PathBuf,
    format: Option<BioDataFormat>,
    media_type: Option<&'static str>,
}

fn serialize_v2_file_artifact_result<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    value: T,
    artifact: FileArtifactSpec,
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
    result.artifacts.push(OutputArtifact {
        artifact_id: artifact.artifact_id.to_owned(),
        role: artifact.role.to_owned(),
        kind: artifact.kind,
        path: artifact.path.to_string_lossy().into_owned(),
        format: artifact.format,
        media_type: artifact.media_type.map(str::to_owned),
        size_bytes: Some(std::fs::metadata(&artifact.path)?.len()),
        sha256: Some(sha256_file(&artifact.path)?),
        metadata: Default::default(),
    });
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
    let mut matching = request
        .inputs
        .iter()
        .filter(|artifact| artifact.role == role);
    let artifact = matching.next().ok_or_else(|| {
        format!(
            "{} requires an input artifact with role {role}",
            request.capability
        )
    })?;
    if matching.next().is_some() {
        return Err(format!("duplicate input role: {role}").into());
    }
    if artifact.files.len() != 1 {
        return Err(format!("input role {role} requires exactly one file").into());
    }
    Ok(resolve_input(base_directory, &artifact.files[0].path))
}

fn resolve_v2_input_files(
    base_directory: &Path,
    request: &JobRequestV2,
    role: &str,
) -> WorkerResult<Vec<PathBuf>> {
    let mut matching = request
        .inputs
        .iter()
        .filter(|artifact| artifact.role == role);
    let artifact = matching.next().ok_or_else(|| {
        format!(
            "{} requires an input artifact with role {role}",
            request.capability
        )
    })?;
    if matching.next().is_some() {
        return Err(format!("duplicate input role: {role}").into());
    }
    if artifact.files.is_empty() {
        return Err(format!("input role {role} requires at least one file").into());
    }
    Ok(artifact
        .files
        .iter()
        .map(|file| resolve_input(base_directory, &file.path))
        .collect())
}

fn sequence_table_format(delimiter: SequenceTableDelimiter) -> BioDataFormat {
    match delimiter {
        SequenceTableDelimiter::Csv => BioDataFormat::Csv,
        SequenceTableDelimiter::Tsv => BioDataFormat::Tsv,
    }
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

fn run_fastq_trim(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "fastq",
        &["output", "min_quality", "min_length", "quality_encoding"],
    )?;
    let options = fastq_trim_options(&request.parameters)?;
    execute_fastq_transform_v1(base_directory, request, |input, output| {
        fastq_trim_path(input, output, &options)
    })
}

fn run_fastq_adapter_trim(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_named_input_contract(
        &request,
        "fastq",
        &["output", "adapter", "adapters", "min_overlap", "min_length"],
    )?;
    let options = fastq_adapter_options(&request.parameters)?;
    execute_fastq_transform_v1(base_directory, request, |input, output| {
        fastq_adapter_trim_path(input, output, &options)
    })
}

fn run_alignment_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("sam")
        .ok_or("alignment.qc.v1 requires inputs.sam")?;
    let metrics = sam_qc_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_expression_matrix_qc(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("matrix")
        .ok_or("expression.matrix.qc.v1 requires inputs.matrix")?;
    let metrics = expression_matrix_qc_path(resolve_input(base_directory, input))?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        metrics.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = metrics.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_interval_intersect(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let left = request
        .inputs
        .get("left-bed")
        .ok_or("interval.intersect.v1 requires inputs.left-bed")?;
    let right = request
        .inputs
        .get("right-bed")
        .ok_or("interval.intersect.v1 requires inputs.right-bed")?;
    let stats = bed_intersect_path(
        resolve_input(base_directory, left),
        resolve_input(base_directory, right),
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = stats.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn run_interval_merge(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_interval_merge_contract(&request)?;
    let input = request
        .inputs
        .get("bed")
        .ok_or("interval.merge.v1 requires inputs.bed")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let stats = bed_merge_path(
        &input,
        &output,
        IntervalMergeOptions {
            max_gap: optional_parameter_u64(&request.parameters, "max_gap")?.unwrap_or(0),
        },
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_interval_subtract(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_interval_subtract_contract(&request)?;
    let left = request
        .inputs
        .get("left-bed")
        .ok_or("interval.subtract.v1 requires inputs.left-bed")?;
    let right = request
        .inputs
        .get("right-bed")
        .ok_or("interval.subtract.v1 requires inputs.right-bed")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let left = resolve_input(base_directory, left);
    let right = resolve_input(base_directory, right);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&left, &output)?;
    ensure_distinct_input_output(&right, &output)?;
    let stats = bed_subtract_path(&left, &right, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats,
        ExecutionMode::LocalCpu,
    );
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

fn run_pdb_summary(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let input = request
        .inputs
        .get("pdb")
        .ok_or("structure.pdb.summary.v1 requires inputs.pdb")?;
    let summary = pdb_summary_path(
        resolve_input(base_directory, input),
        pdb_options(&request.parameters)?,
    )?;
    let mut result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary.clone(),
        ExecutionMode::LocalCpu,
    );
    result.warnings = summary.warnings;
    Ok(serde_json::to_string(&result)?)
}

fn pdb_options(parameters: &serde_json::Value) -> WorkerResult<PdbSummaryOptions> {
    let interpret_b_factors_as_plddt = match parameters.get("interpret_b_factors_as_plddt") {
        Some(value) => value
            .as_bool()
            .ok_or("interpret_b_factors_as_plddt must be a boolean")?,
        None => false,
    };
    Ok(PdbSummaryOptions {
        interpret_b_factors_as_plddt,
    })
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

fn fastq_trim_options(parameters: &serde_json::Value) -> WorkerResult<FastqTrimOptions> {
    Ok(FastqTrimOptions {
        min_quality: optional_parameter_u8(parameters, "min_quality")?
            .unwrap_or(DEFAULT_TRIM_QUALITY),
        min_length: optional_parameter_usize(parameters, "min_length")?
            .unwrap_or(DEFAULT_MIN_LENGTH),
        quality_encoding: parse_fastq_transform_quality_encoding(
            parameters.get("quality_encoding"),
        )?,
    })
}

fn fastq_adapter_options(parameters: &serde_json::Value) -> WorkerResult<FastqAdapterOptions> {
    let adapters = match (
        optional_parameter_string(parameters, "adapter")?,
        parameters.get("adapters"),
    ) {
        (Some(adapter), None) => vec![adapter.to_owned()],
        (None, Some(value)) => value
            .as_array()
            .ok_or("adapters must be an array of strings")?
            .iter()
            .enumerate()
            .map(|(index, value)| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("adapters[{index}] must be a string").into())
            })
            .collect::<WorkerResult<Vec<_>>>()?,
        (Some(_), Some(_)) => {
            return Err("use either adapter or adapters, not both".into());
        }
        (None, None) => FastqAdapterOptions::default().adapters,
    };
    Ok(FastqAdapterOptions {
        adapters,
        min_overlap: optional_parameter_usize(parameters, "min_overlap")?
            .unwrap_or(DEFAULT_ADAPTER_MIN_OVERLAP),
        min_length: optional_parameter_usize(parameters, "min_length")?
            .unwrap_or(DEFAULT_MIN_LENGTH),
    })
}

fn parse_fastq_transform_quality_encoding(
    value: Option<&serde_json::Value>,
) -> WorkerResult<FastqTransformQualityEncoding> {
    match value
        .and_then(serde_json::Value::as_str)
        .unwrap_or("phred+33")
    {
        "phred+33" => Ok(FastqTransformQualityEncoding::Phred33),
        "phred+64" => Ok(FastqTransformQualityEncoding::Phred64),
        value => Err(format!(
            "unsupported quality_encoding {value:?}; expected phred+33 or phred+64"
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

fn run_sequence_extract(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output", "identifiers", "regions", "strict"])?;
    let options = sequence_extract_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        extract_fasta_path(input, output, &options)
    })
}

fn run_sequence_filter(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &[
            "output",
            "min_length",
            "max_length",
            "min_gc_percent",
            "max_gc_percent",
            "max_n_percent",
        ],
    )?;
    let options = sequence_filter_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        filter_fasta_path(input, output, &options)
    })
}

fn run_sequence_reverse_complement(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output"])?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        reverse_complement_fasta_path(input, output)
    })
}

fn run_sequence_translate(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &["output", "frames", "trim_terminal_stop", "stop_at_first"],
    )?;
    let options = sequence_translate_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        translate_fasta_path(input, output, &options)
    })
}

fn run_sequence_orf(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &[
            "output",
            "min_amino_acids",
            "include_reverse_strand",
            "include_partial_3prime",
        ],
    )?;
    let options = sequence_orf_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        find_orfs_fasta_path(input, output, &options)
    })
}

fn run_sequence_id_normalize(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &["output", "prefix", "start", "width", "keep_description"],
    )?;
    let options = sequence_id_normalize_options(&request.parameters)?;
    execute_sequence_transform_v1(base_directory, request, |input, output| {
        normalize_fasta_ids_path(input, output, &options)
    })
}

fn run_sequence_merge(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !["output", "allow_duplicate_ids"].contains(&parameter.as_str()) {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let output = resolve_input(base_directory, output);
    let mut inputs = Vec::new();
    for (role, path) in &request.inputs {
        if role != "fasta" && !role.starts_with("fasta-") {
            return Err(format!("sequence.merge.v1 does not accept input role {role}").into());
        }
        let input = resolve_input(base_directory, path);
        ensure_distinct_input_output(&input, &output)?;
        inputs.push(input);
    }
    if inputs.is_empty() {
        return Err("sequence.merge.v1 requires at least one FASTA input".into());
    }
    let options = sequence_merge_options(&request.parameters)?;
    let summary = merge_fasta_paths(&inputs, &output, &options)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_split(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(
        &request,
        &["output_directory", "records_per_file", "prefix"],
    )?;
    let input = request
        .inputs
        .get("fasta")
        .ok_or("sequence.split.v1 requires inputs.fasta")?;
    let output_directory = request
        .parameters
        .get("output_directory")
        .and_then(serde_json::Value::as_str)
        .ok_or("sequence.split.v1 requires string parameters.output_directory")?;
    let options = sequence_split_options(&request.parameters)?;
    let summary = split_fasta_path(
        resolve_input(base_directory, input),
        resolve_input(base_directory, output_directory),
        &options,
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_to_table(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    validate_v1_sequence_contract(&request, &["output", "delimiter", "include_header"])?;
    let input = request
        .inputs
        .get("fasta")
        .ok_or("sequence.to-table.v1 requires inputs.fasta")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let delimiter = sequence_table_delimiter_option(&request.parameters)?.unwrap_or_else(|| {
        SequenceTableDelimiter::infer_from_path(&output).unwrap_or(SequenceTableDelimiter::Csv)
    });
    let summary = fasta_to_table_path(
        &input,
        &output,
        &SequenceToTableOptions {
            delimiter,
            include_header: optional_parameter_bool(&request.parameters, "include_header")?
                .unwrap_or(true),
        },
    )?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn run_sequence_from_table(base_directory: &Path, request: JobRequest) -> WorkerResult<String> {
    let allowed = [
        "output",
        "delimiter",
        "id_column",
        "sequence_column",
        "description_column",
    ];
    validate_v1_named_input_contract(&request, "table", &allowed)?;
    let input = request
        .inputs
        .get("table")
        .ok_or("sequence.from-table.v1 requires inputs.table")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let mut options = sequence_from_table_options(&request.parameters)?;
    if sequence_table_delimiter_option(&request.parameters)?.is_none() {
        options.delimiter =
            SequenceTableDelimiter::infer_from_path(&input).unwrap_or(SequenceTableDelimiter::Csv);
    }
    let summary = table_to_fasta_path(&input, &output, &options)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn execute_sequence_transform_v1<T>(
    base_directory: &Path,
    request: JobRequest,
    operation: impl FnOnce(&Path, &Path) -> Result<T, SequenceTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = request
        .inputs
        .get("fasta")
        .ok_or_else(|| format!("{} requires inputs.fasta", request.capability))?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let summary = operation(&input, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn execute_sequence_transform_v2<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    operation: impl FnOnce(&Path, &Path) -> Result<T, SequenceTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = resolve_v2_single_input(base_directory, request, "fasta")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let output = resolve_input(base_directory, output);
    ensure_v2_export_output_is_distinct(request, base_directory, &output)?;
    let summary = operation(&input, &output)?;
    let size_bytes = std::fs::metadata(&output)?.len();
    let sha256 = sha256_file(&output)?;
    let mut result = AnalysisResultV2::ok(
        request.job_id.clone(),
        request.capability.clone(),
        summary,
        ExecutionMode::LocalCpu,
    );
    result.artifacts.push(OutputArtifact {
        artifact_id: "sequence-output".to_owned(),
        role: "fasta".to_owned(),
        kind: OutputArtifactKind::DomainFile,
        path: output.to_string_lossy().into_owned(),
        format: Some(BioDataFormat::Fasta),
        media_type: Some("text/x-fasta".to_owned()),
        size_bytes: Some(size_bytes),
        sha256: Some(sha256),
        metadata: Default::default(),
    });
    finalize_v2_input_hashes(&mut result, request, base_directory, verified_inputs)?;
    Ok(serde_json::to_string(&result)?)
}

fn execute_fastq_transform_v1<T>(
    base_directory: &Path,
    request: JobRequest,
    operation: impl FnOnce(&Path, &Path) -> Result<T, FastqTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = request
        .inputs
        .get("fastq")
        .ok_or_else(|| format!("{} requires inputs.fastq", request.capability))?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let input = resolve_input(base_directory, input);
    let output = resolve_input(base_directory, output);
    ensure_distinct_input_output(&input, &output)?;
    let summary = operation(&input, &output)?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        summary,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn execute_fastq_transform_v2<T>(
    request: &JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
    operation: impl FnOnce(&Path, &Path) -> Result<T, FastqTransformError>,
) -> WorkerResult<String>
where
    T: serde::Serialize,
{
    let input = resolve_v2_single_input(base_directory, request, "fastq")?;
    let output = required_sequence_output(&request.parameters, &request.capability)?;
    let output = resolve_input(base_directory, output);
    ensure_v2_export_output_is_distinct(request, base_directory, &output)?;
    let summary = operation(&input, &output)?;
    serialize_v2_file_artifact_result(
        request,
        base_directory,
        verified_inputs,
        summary,
        FileArtifactSpec {
            artifact_id: "fastq-output",
            role: "fastq",
            kind: OutputArtifactKind::DomainFile,
            path: output,
            format: Some(BioDataFormat::Fastq),
            media_type: Some("text/x-fastq"),
        },
    )
}

fn validate_v1_sequence_contract(request: &JobRequest, allowed: &[&str]) -> WorkerResult<()> {
    if !request.inputs.contains_key("fasta") {
        return Err(format!("{} requires inputs.fasta", request.capability).into());
    }
    for role in request.inputs.keys() {
        if role != "fasta" {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !allowed.contains(&parameter.as_str()) {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_v1_named_input_contract(
    request: &JobRequest,
    expected_role: &str,
    allowed: &[&str],
) -> WorkerResult<()> {
    if !request.inputs.contains_key(expected_role) {
        return Err(format!("{} requires inputs.{expected_role}", request.capability).into());
    }
    for role in request.inputs.keys() {
        if role != expected_role {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !allowed.contains(&parameter.as_str()) {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_v1_interval_merge_contract(request: &JobRequest) -> WorkerResult<()> {
    if !request.inputs.contains_key("bed") {
        return Err(format!("{} requires inputs.bed", request.capability).into());
    }
    for role in request.inputs.keys() {
        if role != "bed" {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if !matches!(parameter.as_str(), "output" | "max_gap") {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn validate_v1_interval_subtract_contract(request: &JobRequest) -> WorkerResult<()> {
    for required in ["left-bed", "right-bed"] {
        if !request.inputs.contains_key(required) {
            return Err(format!("{} requires inputs.{required}", request.capability).into());
        }
    }
    for role in request.inputs.keys() {
        if !matches!(role.as_str(), "left-bed" | "right-bed") {
            return Err(format!("{} does not accept input role {role}", request.capability).into());
        }
    }
    if let Some(parameters) = parameter_object(&request.parameters)? {
        for parameter in parameters.keys() {
            if parameter != "output" {
                return Err(format!(
                    "{} does not accept parameter {parameter}",
                    request.capability
                )
                .into());
            }
        }
    }
    Ok(())
}

fn parameter_object(
    parameters: &serde_json::Value,
) -> WorkerResult<Option<&serde_json::Map<String, serde_json::Value>>> {
    match parameters {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Object(parameters) => Ok(Some(parameters)),
        _ => Err("sequence transform parameters must be an object".into()),
    }
}

fn required_sequence_output<'a>(
    parameters: &'a serde_json::Value,
    capability: &str,
) -> WorkerResult<&'a str> {
    let output = parameters
        .get("output")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{capability} requires string parameters.output"))?;
    if output.trim().is_empty() {
        return Err(format!("{capability} requires a non-empty parameters.output").into());
    }
    Ok(output)
}

fn sequence_extract_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceExtractOptions> {
    let identifiers = optional_parameter_array(parameters, "identifiers")?;
    let identifiers = identifiers
        .iter()
        .enumerate()
        .map(|(index, value)| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("identifiers[{index}] must be a string").into())
        })
        .collect::<WorkerResult<Vec<_>>>()?;
    let regions = optional_parameter_array(parameters, "regions")?;
    let regions = regions
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let specification = value.as_str().ok_or_else(|| -> WorkerError {
                format!("regions[{index}] must be a string").into()
            })?;
            parse_sequence_region_spec(specification).map_err(Into::into)
        })
        .collect::<WorkerResult<Vec<_>>>()?;
    Ok(SequenceExtractOptions {
        identifiers,
        regions,
        strict: optional_parameter_bool(parameters, "strict")?.unwrap_or(false),
    })
}

fn optional_parameter_array(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Vec<serde_json::Value>> {
    match parameters.get(key) {
        Some(value) => value
            .as_array()
            .cloned()
            .ok_or_else(|| format!("{key} must be an array").into()),
        None => Ok(Vec::new()),
    }
}

fn sequence_filter_options(parameters: &serde_json::Value) -> WorkerResult<SequenceFilterOptions> {
    let options = SequenceFilterOptions {
        min_length: optional_parameter_u64(parameters, "min_length")?.unwrap_or(0),
        max_length: optional_parameter_u64(parameters, "max_length")?,
        min_gc_percent: optional_parameter_percentage(parameters, "min_gc_percent")?,
        max_gc_percent: optional_parameter_percentage(parameters, "max_gc_percent")?,
        max_n_percent: optional_parameter_percentage(parameters, "max_n_percent")?,
    };
    if options
        .max_length
        .is_some_and(|maximum| maximum < options.min_length)
    {
        return Err("max_length must be at least min_length".into());
    }
    if matches!(
        (options.min_gc_percent, options.max_gc_percent),
        (Some(minimum), Some(maximum)) if maximum < minimum
    ) {
        return Err("max_gc_percent must be at least min_gc_percent".into());
    }
    Ok(options)
}

fn sequence_translate_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceTranslateOptions> {
    let frames = match parameters.get("frames") {
        None => vec![1],
        Some(value) => {
            let values = value
                .as_array()
                .ok_or("frames must be an array of integers")?;
            if values.is_empty() {
                return Err("frames must contain at least one translation frame".into());
            }
            values
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let frame = value
                        .as_i64()
                        .ok_or_else(|| format!("frames[{index}] must be an integer"))?;
                    let frame = i8::try_from(frame)
                        .map_err(|_| format!("frames[{index}] is outside the supported range"))?;
                    if !matches!(frame, -3..=-1 | 1..=3) {
                        return Err(format!(
                            "unsupported translation frame {frame}; expected -3, -2, -1, 1, 2, or 3"
                        )
                        .into());
                    }
                    Ok(frame)
                })
                .collect::<WorkerResult<Vec<_>>>()?
        }
    };
    Ok(SequenceTranslateOptions {
        frames,
        trim_terminal_stop: optional_parameter_bool(parameters, "trim_terminal_stop")?
            .unwrap_or(false),
        stop_at_first: optional_parameter_bool(parameters, "stop_at_first")?.unwrap_or(false),
    })
}

fn sequence_orf_options(parameters: &serde_json::Value) -> WorkerResult<SequenceOrfOptions> {
    let mut options = SequenceOrfOptions::default();
    if let Some(minimum) = optional_parameter_usize(parameters, "min_amino_acids")? {
        if minimum == 0 {
            return Err("min_amino_acids must be at least 1".into());
        }
        options.min_amino_acids = minimum;
    }
    if let Some(include) = optional_parameter_bool(parameters, "include_reverse_strand")? {
        options.include_reverse_strand = include;
    }
    if let Some(include) = optional_parameter_bool(parameters, "include_partial_3prime")? {
        options.include_partial_3prime = include;
    }
    Ok(options)
}

fn sequence_id_normalize_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceIdNormalizeOptions> {
    let mut options = SequenceIdNormalizeOptions::default();
    if let Some(prefix) = optional_parameter_string(parameters, "prefix")? {
        options.prefix = prefix.to_owned();
    }
    if let Some(start) = optional_parameter_u64(parameters, "start")? {
        if start == 0 {
            return Err("start must be at least 1".into());
        }
        options.start = start;
    }
    if let Some(width) = optional_parameter_usize(parameters, "width")? {
        if width == 0 {
            return Err("width must be at least 1".into());
        }
        options.width = Some(width);
    }
    if let Some(keep) = optional_parameter_bool(parameters, "keep_description")? {
        options.keep_description = keep;
    }
    Ok(options)
}

fn sequence_merge_options(parameters: &serde_json::Value) -> WorkerResult<SequenceMergeOptions> {
    Ok(SequenceMergeOptions {
        allow_duplicate_ids: optional_parameter_bool(parameters, "allow_duplicate_ids")?
            .unwrap_or(false),
    })
}

fn sequence_split_options(parameters: &serde_json::Value) -> WorkerResult<SequenceSplitOptions> {
    let mut options = SequenceSplitOptions::default();
    if let Some(records_per_file) = optional_parameter_usize(parameters, "records_per_file")? {
        if records_per_file == 0 {
            return Err("records_per_file must be at least 1".into());
        }
        options.records_per_file = records_per_file;
    }
    if let Some(prefix) = optional_parameter_string(parameters, "prefix")? {
        options.prefix = prefix.to_owned();
    }
    Ok(options)
}

fn sequence_from_table_options(
    parameters: &serde_json::Value,
) -> WorkerResult<SequenceFromTableOptions> {
    let mut options = SequenceFromTableOptions::default();
    if let Some(delimiter) = sequence_table_delimiter_option(parameters)? {
        options.delimiter = delimiter;
    }
    if let Some(column) = optional_parameter_string(parameters, "id_column")? {
        options.id_column = column.to_owned();
    }
    if let Some(column) = optional_parameter_string(parameters, "sequence_column")? {
        options.sequence_column = column.to_owned();
    }
    if let Some(value) = parameters.get("description_column") {
        options.description_column = if value.is_null() {
            None
        } else {
            Some(
                value
                    .as_str()
                    .ok_or("description_column must be a string or null")?
                    .to_owned(),
            )
        };
    }
    Ok(options)
}

fn sequence_table_delimiter_option(
    parameters: &serde_json::Value,
) -> WorkerResult<Option<SequenceTableDelimiter>> {
    match optional_parameter_string(parameters, "delimiter")? {
        Some("csv") => Ok(Some(SequenceTableDelimiter::Csv)),
        Some("tsv" | "tab") => Ok(Some(SequenceTableDelimiter::Tsv)),
        Some(value) => Err(format!("delimiter must be csv or tsv, got {value:?}").into()),
        None => Ok(None),
    }
}

fn optional_parameter_u64(parameters: &serde_json::Value, key: &str) -> WorkerResult<Option<u64>> {
    match parameters.get(key) {
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer").into()),
        None => Ok(None),
    }
}

fn optional_parameter_usize(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Option<usize>> {
    optional_parameter_u64(parameters, key)?
        .map(|value| {
            usize::try_from(value)
                .map_err(|_| format!("{key} exceeds this platform's size limit").into())
        })
        .transpose()
}

fn optional_parameter_u8(parameters: &serde_json::Value, key: &str) -> WorkerResult<Option<u8>> {
    optional_parameter_u64(parameters, key)?
        .map(|value| u8::try_from(value).map_err(|_| format!("{key} must be 0..255").into()))
        .transpose()
}

fn optional_parameter_bool(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Option<bool>> {
    match parameters.get(key) {
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a boolean").into()),
        None => Ok(None),
    }
}

fn optional_parameter_string<'a>(
    parameters: &'a serde_json::Value,
    key: &str,
) -> WorkerResult<Option<&'a str>> {
    match parameters.get(key) {
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("{key} must be a string").into()),
        None => Ok(None),
    }
}

fn optional_parameter_percentage(
    parameters: &serde_json::Value,
    key: &str,
) -> WorkerResult<Option<f64>> {
    match parameters.get(key) {
        Some(value) => {
            let percent = value
                .as_f64()
                .ok_or_else(|| format!("{key} must be a number"))?;
            if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                return Err(format!("{key} must be between 0 and 100").into());
            }
            Ok(Some(percent))
        }
        None => Ok(None),
    }
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
    fn v2_table_export_rejects_an_undeclared_extra_input_role() {
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
        assert!(
            result.diagnostics[0]
                .message
                .contains("does not accept input role metadata")
        );
        assert_eq!(
            fs::read_to_string(&protected).expect("declared input remains readable"),
            r#"{"protected":true}"#
        );
        fs::remove_file(protected).expect("remove protected input");
    }

    #[test]
    fn v2_rejects_duplicate_roles_instead_of_selecting_the_first() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/sequences/tiny.fa")
            .canonicalize()
            .expect("FASTA fixture");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.stats.v1",
            "fasta",
        );
        let mut duplicate = request.inputs[0].clone();
        duplicate.artifact_id = "duplicate-artifact".to_owned();
        duplicate.files[0].file_id = "duplicate-file".to_owned();
        request.inputs.push(duplicate);

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("worker returns a v2 error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("duplicate input role: fasta")
        );
    }

    #[test]
    fn v2_rejects_unknown_parameters() {
        let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/fixtures/sequences/tiny.fa")
            .canonicalize()
            .expect("FASTA fixture");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.stats.v1",
            "fasta",
        );
        request.parameters = serde_json::json!({"max_cylces": 100});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("worker returns a v2 error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("does not accept parameter max_cylces")
        );
    }

    #[test]
    fn v1_executes_all_sequence_transform_capabilities() {
        let input = write_temporary(
            "sequence-transform-v1.fa",
            b">gene description\nATGAAATAA\n>gc\nGCGCGC\n>short\nNN\n",
        );
        let cases = [
            (
                "sequence.extract.v1",
                serde_json::json!({"identifiers": ["gene"], "regions": ["gene:1-3"], "strict": true}),
            ),
            (
                "sequence.filter.v1",
                serde_json::json!({"min_length": 6, "min_gc_percent": 50}),
            ),
            ("sequence.reverse-complement.v1", serde_json::json!({})),
            (
                "sequence.translate.v1",
                serde_json::json!({"frames": [1, -1], "trim_terminal_stop": true}),
            ),
            (
                "sequence.orf.v1",
                serde_json::json!({
                    "min_amino_acids": 2,
                    "include_reverse_strand": false
                }),
            ),
            (
                "sequence.id.normalize.v1",
                serde_json::json!({
                    "prefix": "seq",
                    "start": 1,
                    "width": 3,
                    "keep_description": true
                }),
            ),
        ];

        for (capability, mut parameters) in cases {
            let output = temporary_path(&format!("{capability}.fa"));
            parameters["output"] = serde_json::json!(output);
            let request = sequence_v1_request(&input, capability, parameters);
            let serialized = execute_request(request, Path::new("."))
                .unwrap_or_else(|error| panic!("{capability} failed: {error}"));
            let result: serde_json::Value =
                serde_json::from_str(&serialized).expect("valid v1 sequence result");

            assert_eq!(result["status"], "ok", "{capability}");
            assert_eq!(result["capability"], capability, "{capability}");
            assert!(fs::metadata(&output).expect("sequence output").len() > 0);
            fs::remove_file(output).expect("remove sequence output");
        }
        fs::remove_file(input).expect("remove v1 sequence input");
    }

    #[test]
    fn v2_executes_all_sequence_transforms_with_hashed_fasta_artifacts() {
        let input = write_temporary(
            "sequence-transform-v2.fa",
            b">gene description\nATGAAATAA\n>gc\nGCGCGC\n>short\nNN\n",
        );
        let cases = [
            (
                "sequence.extract.v1",
                serde_json::json!({"identifiers": ["gene"], "regions": ["gene:1-3"], "strict": true}),
            ),
            (
                "sequence.filter.v1",
                serde_json::json!({"max_n_percent": 0}),
            ),
            ("sequence.reverse-complement.v1", serde_json::json!({})),
            (
                "sequence.translate.v1",
                serde_json::json!({"frames": [1, -1], "stop_at_first": false}),
            ),
            (
                "sequence.orf.v1",
                serde_json::json!({
                    "min_amino_acids": 2,
                    "include_reverse_strand": true,
                    "include_partial_3prime": true
                }),
            ),
            (
                "sequence.id.normalize.v1",
                serde_json::json!({
                    "prefix": "seq",
                    "start": 1,
                    "width": 3,
                    "keep_description": true
                }),
            ),
        ];

        for (capability, mut parameters) in cases {
            let output = temporary_path(&format!("{capability}-v2.fa"));
            parameters["output"] = serde_json::json!(output);
            let mut request = artifact_request(
                &input,
                BioDataFormat::Fasta,
                CompressionFormat::None,
                capability,
                "fasta",
            );
            request.parameters = parameters;
            let serialized = execute_request_v2(request, Path::new("."))
                .unwrap_or_else(|error| panic!("{capability} failed: {error}"));
            let result: AnalysisResultV2<serde_json::Value> =
                serde_json::from_str(&serialized).expect("valid v2 sequence result");

            assert_eq!(result.status, JobStatus::Ok, "{capability}");
            assert_eq!(result.capability, capability, "{capability}");
            assert_eq!(result.provenance.input_sha256.len(), 1, "{capability}");
            let expected_input_hash = super::sha256_file(&input).expect("hash sequence input");
            assert_eq!(
                result.provenance.input_sha256.get("input-file"),
                Some(&expected_input_hash),
                "{capability}"
            );
            assert_eq!(result.artifacts.len(), 1, "{capability}");
            let artifact = &result.artifacts[0];
            assert_eq!(artifact.role, "fasta", "{capability}");
            assert_eq!(
                artifact.kind,
                linxira_bio_protocol::OutputArtifactKind::DomainFile
            );
            assert_eq!(artifact.format, Some(BioDataFormat::Fasta));
            assert_eq!(artifact.media_type.as_deref(), Some("text/x-fasta"));
            assert_eq!(PathBuf::from(&artifact.path), output, "{capability}");
            assert_eq!(
                artifact.size_bytes,
                Some(fs::metadata(&output).expect("sequence output").len()),
                "{capability}"
            );
            let expected_output_hash = super::sha256_file(&output).expect("hash sequence output");
            assert_eq!(
                artifact.sha256.as_deref(),
                Some(expected_output_hash.as_str()),
                "{capability}"
            );
            fs::remove_file(output).expect("remove v2 sequence output");
        }
        fs::remove_file(input).expect("remove v2 sequence input");
    }

    #[test]
    fn v2_executes_sequence_merge_split_and_table_conversion() {
        let first = write_temporary("sequence-merge-first.fa", b">one\nACGT\n>two\nNN\n");
        let second = write_temporary("sequence-merge-second.fa", b">three\nGG\n");

        let merged_output = temporary_path("sequence-merged.fa");
        let mut merge_request = artifact_request(
            &first,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.merge.v1",
            "fasta",
        );
        merge_request.inputs[0].cardinality = InputCardinality::Batch;
        merge_request.inputs[0].files.push(ArtifactFile {
            file_id: "input-file-2".to_owned(),
            path: second.to_string_lossy().into_owned(),
            role: None,
            format: BioDataFormat::Fasta,
            compression: CompressionFormat::None,
            size_bytes: fs::metadata(&second).expect("second input metadata").len(),
            modified_at: None,
            sha256: None,
        });
        merge_request.parameters = serde_json::json!({"output": merged_output});
        let serialized =
            execute_request_v2(merge_request, Path::new(".")).expect("merge executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid merge result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.merge.v1");
        assert_eq!(result.result["input_files"], 2);
        assert_eq!(result.result["output_records"], 3);
        assert_eq!(result.provenance.input_sha256.len(), 2);
        assert_eq!(result.artifacts[0].format, Some(BioDataFormat::Fasta));
        assert!(fs::metadata(&merged_output).expect("merged output").len() > 0);

        let split_directory = temporary_path("sequence-split-output");
        let mut split_request = artifact_request(
            &merged_output,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.split.v1",
            "fasta",
        );
        split_request.parameters = serde_json::json!({
            "output_directory": split_directory,
            "records_per_file": 2,
            "prefix": "chunk"
        });
        let serialized =
            execute_request_v2(split_request, Path::new(".")).expect("split executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid split result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.split.v1");
        assert_eq!(result.result["output_files"], 2);
        assert_eq!(
            result.artifacts[0].kind,
            linxira_bio_protocol::OutputArtifactKind::Directory
        );
        assert!(split_directory.join("chunk_001.fa").is_file());

        let table_output = temporary_path("sequence-table.tsv");
        let mut table_request = artifact_request(
            &merged_output,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.to-table.v1",
            "fasta",
        );
        table_request.parameters = serde_json::json!({"output": table_output, "delimiter": "tsv"});
        let serialized = execute_request_v2(table_request, Path::new("."))
            .expect("to-table executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid to-table result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.to-table.v1");
        assert_eq!(result.result["output_rows"], 3);
        assert_eq!(
            result.artifacts[0].kind,
            linxira_bio_protocol::OutputArtifactKind::Table
        );
        assert_eq!(result.artifacts[0].format, Some(BioDataFormat::Tsv));

        let roundtrip_output = temporary_path("sequence-table-roundtrip.fa");
        let mut roundtrip_request = artifact_request(
            &table_output,
            BioDataFormat::Tsv,
            CompressionFormat::None,
            "sequence.from-table.v1",
            "table",
        );
        roundtrip_request.parameters =
            serde_json::json!({"output": roundtrip_output, "delimiter": "tsv"});
        let serialized = execute_request_v2(roundtrip_request, Path::new("."))
            .expect("from-table executes through v2");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid from-table result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.capability, "sequence.from-table.v1");
        assert_eq!(result.result["output_records"], 3);
        assert_eq!(result.artifacts[0].format, Some(BioDataFormat::Fasta));
        assert!(
            fs::read_to_string(&roundtrip_output)
                .expect("roundtrip FASTA")
                .contains(">three\nGG\n")
        );

        fs::remove_file(first).expect("remove first merge input");
        fs::remove_file(second).expect("remove second merge input");
        fs::remove_file(merged_output).expect("remove merged output");
        fs::remove_dir_all(split_directory).expect("remove split output");
        fs::remove_file(table_output).expect("remove table output");
        fs::remove_file(roundtrip_output).expect("remove roundtrip output");
    }

    #[test]
    fn sequence_transform_requests_reject_invalid_contracts_and_values() {
        let input = write_temporary("sequence-transform-invalid.fa", b">gene\nATGAAATAA\n");
        let cases = [
            (
                "sequence.extract.v1",
                serde_json::json!({"identifiers": "gene"}),
                "identifiers must be an array",
            ),
            (
                "sequence.filter.v1",
                serde_json::json!({"min_gc_percent": 101}),
                "must be between 0 and 100",
            ),
            (
                "sequence.translate.v1",
                serde_json::json!({"frames": [0]}),
                "unsupported translation frame 0",
            ),
            (
                "sequence.orf.v1",
                serde_json::json!({"min_amino_acids": 0}),
                "must be at least 1",
            ),
            (
                "sequence.reverse-complement.v1",
                serde_json::json!({"unexpected": true}),
                "does not accept parameter unexpected",
            ),
        ];

        for (capability, mut parameters, expected_message) in cases {
            let output = temporary_path(&format!("invalid-{capability}.fa"));
            parameters["output"] = serde_json::json!(output);
            let mut request = artifact_request(
                &input,
                BioDataFormat::Fasta,
                CompressionFormat::None,
                capability,
                "fasta",
            );
            request.parameters = parameters;
            let serialized = execute_request_v2(request, Path::new("."))
                .expect("invalid v2 request uses an error envelope");
            let result: AnalysisResultV2<serde_json::Value> =
                serde_json::from_str(&serialized).expect("valid v2 error result");

            assert_eq!(result.status, JobStatus::Error, "{capability}");
            assert!(
                result.diagnostics[0].message.contains(expected_message),
                "{capability}: {}",
                result.diagnostics[0].message
            );
            assert!(result.artifacts.is_empty(), "{capability}");
            assert!(!output.exists(), "{capability}");
        }

        let output = temporary_path("wrong-role.fa");
        let mut wrong_role = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.filter.v1",
            "file",
        );
        wrong_role.parameters = serde_json::json!({"output": output});
        let serialized = execute_request_v2(wrong_role, Path::new("."))
            .expect("wrong role uses an error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid wrong-role result");
        assert_eq!(result.status, JobStatus::Error);
        assert!(
            result.diagnostics[0]
                .message
                .contains("requires input role fasta")
        );

        let mut legacy = sequence_v1_request(
            &input,
            "sequence.reverse-complement.v1",
            serde_json::json!({"output": output, "unexpected": true}),
        );
        legacy
            .inputs
            .insert("extra".to_owned(), input.to_string_lossy().into_owned());
        let error = execute_request(legacy, Path::new("."))
            .expect_err("legacy request rejects extra input roles");
        assert!(
            error
                .to_string()
                .contains("does not accept input role extra")
        );

        fs::remove_file(input).expect("remove invalid sequence input");
    }

    #[test]
    fn v2_sequence_transform_preserves_an_existing_output() {
        let input = write_temporary("sequence-transform-input.fa", b">sequence\nACGT\n");
        let output = write_temporary("sequence-transform-protected.fa", b"protected\n");
        let mut request = artifact_request(
            &input,
            BioDataFormat::Fasta,
            CompressionFormat::None,
            "sequence.reverse-complement.v1",
            "fasta",
        );
        request.parameters = serde_json::json!({"output": output});

        let serialized = execute_request_v2(request, Path::new("."))
            .expect("existing output uses an error envelope");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid overwrite error result");

        assert_eq!(result.status, JobStatus::Error);
        assert!(result.artifacts.is_empty());
        assert!(
            result.diagnostics[0]
                .message
                .contains("refusing to overwrite")
        );
        assert_eq!(
            fs::read_to_string(&output).expect("protected output remains"),
            "protected\n"
        );
        fs::remove_file(input).expect("remove overwrite input");
        fs::remove_file(output).expect("remove protected output");
    }

    #[test]
    fn v2_executes_new_single_input_qc_capabilities() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let cases = [
            (
                root.join("tests/fixtures/alignment-qc/valid.sam"),
                BioDataFormat::Sam,
                "alignment.qc.v1",
                "sam",
                "record_count",
                5,
            ),
            (
                root.join("tests/fixtures/expression-matrix/counts.tsv"),
                BioDataFormat::Tsv,
                "expression.matrix.qc.v1",
                "matrix",
                "feature_count",
                4,
            ),
        ];

        for (path, format, capability, role, field, expected) in cases {
            let request = artifact_request(
                &path.canonicalize().expect("fixture path"),
                format,
                CompressionFormat::None,
                capability,
                role,
            );
            let serialized =
                execute_request_v2(request, Path::new(".")).expect("execute v2 local capability");
            let result: AnalysisResultV2<serde_json::Value> =
                serde_json::from_str(&serialized).expect("valid v2 result");
            assert_eq!(result.status, JobStatus::Ok, "{capability}");
            assert_eq!(result.result[field], expected, "{capability}");
            assert_eq!(result.provenance.input_sha256.len(), 1, "{capability}");
        }
    }

    #[test]
    fn v2_executes_interval_intersection_with_two_hashed_inputs() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let left = root
            .join("tests/fixtures/interval-intersect/left.bed")
            .canonicalize()
            .expect("left fixture");
        let right = root
            .join("tests/fixtures/interval-intersect/right.bed")
            .canonicalize()
            .expect("right fixture");
        let mut request = artifact_request(
            &left,
            BioDataFormat::Bed,
            CompressionFormat::None,
            "interval.intersect.v1",
            "left-bed",
        );
        request.inputs.push(InputArtifact {
            artifact_id: "right-artifact".to_owned(),
            role: "right-bed".to_owned(),
            cardinality: InputCardinality::Single,
            files: vec![ArtifactFile {
                file_id: "right-file".to_owned(),
                path: right.to_string_lossy().into_owned(),
                role: None,
                format: BioDataFormat::Bed,
                compression: CompressionFormat::None,
                size_bytes: fs::metadata(&right).expect("right metadata").len(),
                modified_at: None,
                sha256: None,
            }],
            dataset_id: None,
        });

        let serialized =
            execute_request_v2(request, Path::new(".")).expect("execute v2 intersection");
        let result: AnalysisResultV2<serde_json::Value> =
            serde_json::from_str(&serialized).expect("valid v2 result");
        assert_eq!(result.status, JobStatus::Ok);
        assert_eq!(result.result["overlap_pair_count"], 3);
        assert_eq!(result.provenance.input_sha256.len(), 2);
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

    fn sequence_v1_request(
        path: &Path,
        capability: &str,
        parameters: serde_json::Value,
    ) -> JobRequest {
        JobRequest {
            schema_version: SCHEMA_VERSION.to_owned(),
            job_id: "sequence-transform-v1-test".to_owned(),
            capability: capability.to_owned(),
            inputs: BTreeMap::from([("fasta".to_owned(), path.to_string_lossy().into_owned())]),
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
