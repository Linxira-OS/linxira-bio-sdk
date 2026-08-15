use super::{WorkerResult, sha256_file, validate_v1_multi_input_contract};
use linxira_bio_protocol::{
    AnalysisResultV2, ArtifactFile, BioDataFormat, CompressionFormat, DiagnosticSeverity,
    ExecutionMode, InputArtifact, InputCardinality, JobRequest, JobRequestV2, JobStatus,
    NetworkAccess, OutputArtifactKind, WorkflowPackManifest, WorkflowResumeConfig,
    WorkflowRuntimeKind, semver_range::core_compatibility_matches,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

const BULK_EXPRESSION_MANIFEST: &str = "manifest.json";
const MEDICAL_BULK_CAPABILITY: &str = "medical.bulk-rnaseq.v1";
const BULK_EXPRESSION_PACK: &str = "org.linxira.bulk-expression-deseq2";
const SEQUENCE_CONVERT_PACK: &str = "org.linxira.sequence-conversion-biopython";
const MEDICAL_SURVIVAL_PACK: &str = "org.linxira.medical-survival";
const CHEMISTRY_DESCRIPTORS_PACK: &str = "org.linxira.chemistry-descriptors-rdkit";

#[derive(Debug, Clone, PartialEq)]
struct WorkflowContract {
    capabilities: Vec<String>,
    pack_id: String,
    pack_directory: String,
    roles: Vec<String>,
    input_formats: Vec<BioDataFormat>,
    parameters: Vec<String>,
    artifact_count: usize,
    artifact_roles: Vec<String>,
    artifact_kind: OutputArtifactKind,
    artifact_formats: Vec<BioDataFormat>,
    artifact_media_type: Option<String>,
    runtime: WorkflowRuntimeKind,
}

/// Release fallback contracts used when a pack manifest does not declare an
/// explicit `contract` (for example a third-party pack predating the
/// declaration). Official packs must declare the contract.
fn bulk_expression_fallback_contract() -> WorkflowContract {
    WorkflowContract {
        capabilities: vec![
            "expression.differential.v1".to_owned(),
            "medical.bulk-rnaseq.v1".to_owned(),
            "expression.deseq2.v1".to_owned(),
        ],
        pack_id: BULK_EXPRESSION_PACK.to_owned(),
        pack_directory: BULK_EXPRESSION_PACK.to_owned(),
        roles: vec!["counts".to_owned(), "sample_metadata".to_owned()],
        input_formats: vec![BioDataFormat::Csv, BioDataFormat::Tsv],
        parameters: vec![
            "output_directory".to_owned(),
            "feature_id_column".to_owned(),
            "sample_id_column".to_owned(),
            "condition_column".to_owned(),
            "reference_level".to_owned(),
            "contrast_level".to_owned(),
            "alpha".to_owned(),
            "min_total_count".to_owned(),
        ],
        artifact_count: 2,
        artifact_roles: vec![
            "differential-expression".to_owned(),
            "normalized-counts".to_owned(),
        ],
        artifact_kind: OutputArtifactKind::Table,
        artifact_formats: vec![BioDataFormat::Csv],
        artifact_media_type: Some("text/csv".to_owned()),
        runtime: WorkflowRuntimeKind::R,
    }
}

fn sequence_convert_fallback_contract() -> WorkflowContract {
    WorkflowContract {
        capabilities: vec!["sequence.convert.biopython.v1".to_owned()],
        pack_id: SEQUENCE_CONVERT_PACK.to_owned(),
        pack_directory: SEQUENCE_CONVERT_PACK.to_owned(),
        roles: vec!["sequences".to_owned()],
        input_formats: vec![
            BioDataFormat::Fasta,
            BioDataFormat::Fastq,
            BioDataFormat::Genbank,
            BioDataFormat::Embl,
        ],
        parameters: vec![
            "output_directory".to_owned(),
            "output_filename".to_owned(),
            "output_format".to_owned(),
        ],
        artifact_count: 1,
        artifact_roles: vec!["converted-sequences".to_owned()],
        artifact_kind: OutputArtifactKind::DomainFile,
        artifact_formats: vec![
            BioDataFormat::Fasta,
            BioDataFormat::Fastq,
            BioDataFormat::Genbank,
            BioDataFormat::Embl,
        ],
        artifact_media_type: None,
        runtime: WorkflowRuntimeKind::Python,
    }
}

#[derive(Debug, Deserialize)]
struct WorkflowCatalogEntry {
    id: String,
    capability: String,
    #[serde(default)]
    capability_aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WorkflowCatalog {
    schema_version: String,
    packs: Vec<WorkflowCatalogEntry>,
}

/// Resolve the workflow-pack catalog: the runtime catalog below the workflow
/// root when present, else the embedded release snapshot.
fn load_workflow_catalog() -> WorkerResult<WorkflowCatalog> {
    const EMBEDDED: &str = include_str!("../../../../workflows/catalog.json");
    let text = match workflow_root() {
        Ok(root) => {
            let candidate = root.join("workflows").join("catalog.json");
            if candidate.is_file() {
                fs::read_to_string(&candidate)?
            } else {
                EMBEDDED.to_owned()
            }
        }
        Err(_) => EMBEDDED.to_owned(),
    };
    let catalog: WorkflowCatalog = serde_json::from_str(&text)?;
    if catalog.schema_version != "1" || catalog.packs.is_empty() {
        return Err("workflow catalog is invalid".into());
    }
    Ok(catalog)
}

/// Load the execution contract for a workflow pack. The pack's manifest
/// `contract` declaration is authoritative; the release fallback is used only
/// when the manifest lacks the declaration.
fn contract_for(
    pack_id: &str,
    pack_directory: &str,
    runtime: WorkflowRuntimeKind,
) -> WorkerResult<WorkflowContract> {
    let fallback = |mut contract: WorkflowContract| -> WorkerResult<WorkflowContract> {
        contract.capabilities = capabilities_for_pack(pack_id)?;
        contract.pack_id = pack_id.to_owned();
        contract.pack_directory = pack_directory.to_owned();
        contract.runtime = runtime;
        Ok(contract)
    };
    let root = workflow_root()?;
    let manifest_path = safe_pack_path(&root, pack_directory)?.join(BULK_EXPRESSION_MANIFEST);
    let manifest: WorkflowPackManifest = serde_json::from_slice(&fs::read(manifest_path)?)?;
    let Some(declared) = manifest.contract.as_ref() else {
        return match pack_id {
            BULK_EXPRESSION_PACK => fallback(bulk_expression_fallback_contract()),
            SEQUENCE_CONVERT_PACK => fallback(sequence_convert_fallback_contract()),
            _ => Err(format!("no workflow contract fallback for pack {pack_id}").into()),
        };
    };
    if declared.inputs.is_empty()
        || declared.outputs.roles.is_empty()
        || declared.parameters.is_empty()
    {
        return Err(format!("workflow pack {pack_id} declares an empty contract").into());
    }
    let mut roles = Vec::with_capacity(declared.inputs.len());
    let mut input_formats = Vec::new();
    for input in &declared.inputs {
        if roles.contains(&input.role) {
            return Err(
                format!("workflow pack {pack_id} repeats input role {}", input.role).into(),
            );
        }
        roles.push(input.role.clone());
        for format in &input.formats {
            if !input_formats.contains(format) {
                input_formats.push(*format);
            }
        }
    }
    Ok(WorkflowContract {
        capabilities: capabilities_for_pack(pack_id)?,
        pack_id: pack_id.to_owned(),
        pack_directory: pack_directory.to_owned(),
        roles,
        input_formats,
        parameters: declared.parameters.clone(),
        artifact_count: declared.outputs.roles.len(),
        artifact_roles: declared.outputs.roles.clone(),
        artifact_kind: declared.outputs.kind,
        artifact_formats: declared.outputs.formats.clone(),
        artifact_media_type: declared.outputs.media_type.clone(),
        runtime,
    })
}

/// Capabilities served by a pack, from the workflow catalog
/// (`capability` plus `capability_aliases`).
fn capabilities_for_pack(pack_id: &str) -> WorkerResult<Vec<String>> {
    let catalog = load_workflow_catalog()?;
    let entry = catalog
        .packs
        .iter()
        .find(|candidate| candidate.id == pack_id)
        .ok_or_else(|| format!("workflow catalog lacks pack {pack_id}"))?;
    let mut capabilities = Vec::with_capacity(entry.capability_aliases.len() + 1);
    capabilities.push(entry.capability.clone());
    for alias in &entry.capability_aliases {
        if !capabilities.contains(alias) {
            capabilities.push(alias.clone());
        }
    }
    Ok(capabilities)
}

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct VerifiedWorkflowPack {
    root: PathBuf,
    entrypoint: PathBuf,
    dependency_lock_sha256: String,
    resume: Option<WorkflowResumeConfig>,
}

/// Completion state written into the workflow output directory when a
/// resume-enabled pack finishes successfully. A later run with identical
/// inputs and still-valid artifacts replays the recorded envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct WorkflowResumeState {
    schema_version: String,
    job_id: String,
    capability: String,
    core_version: String,
    execution_mode: ExecutionMode,
    input_sha256: BTreeMap<String, String>,
    dependency_lock_sha256: String,
    result: AnalysisResultV2<serde_json::Value>,
}

struct PreparedInput {
    path: PathBuf,
    sha256: String,
}

struct PreparedWorkflowRequest {
    request: JobRequestV2,
    output_directory: PathBuf,
    inputs: Vec<PreparedInput>,
    role_hashes: BTreeMap<String, String>,
}

struct TemporaryRequestDirectory {
    path: PathBuf,
}

struct WorkflowOutputDirectory {
    path: PathBuf,
    preserve: bool,
    created: bool,
}

impl TemporaryRequestDirectory {
    fn create() -> WorkerResult<Self> {
        let temporary_root = env::temp_dir();
        for _ in 0..64 {
            let counter = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let candidate = temporary_root.join(format!(
                "linxira-bio-workflow-{}-{counter}",
                std::process::id()
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => return Ok(Self { path: candidate }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not allocate a unique workflow request directory".into())
    }

    fn write_request(&self, request: &JobRequestV2) -> WorkerResult<PathBuf> {
        let path = self.path.join("request.json");
        let bytes = serde_json::to_vec(request)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        Ok(path)
    }
}

impl Drop for TemporaryRequestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl WorkflowOutputDirectory {
    fn new(path: PathBuf) -> Self {
        let created = !path.exists();
        Self {
            path,
            preserve: false,
            created,
        }
    }

    fn preserve(&mut self) {
        self.preserve = true;
    }
}

impl Drop for WorkflowOutputDirectory {
    fn drop(&mut self) {
        if self.created && !self.preserve && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(super) fn execute_bulk_expression_v1(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    let contract = contract_for(
        BULK_EXPRESSION_PACK,
        BULK_EXPRESSION_PACK,
        WorkflowRuntimeKind::R,
    )?;
    execute_workflow_v1(&contract, base_directory, request)
}

pub(super) fn execute_sequence_convert_v1(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    let contract = contract_for(
        SEQUENCE_CONVERT_PACK,
        SEQUENCE_CONVERT_PACK,
        WorkflowRuntimeKind::Python,
    )?;
    execute_workflow_v1(&contract, base_directory, request)
}

fn execute_workflow_v1(
    contract: &WorkflowContract,
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    ensure_supported_capability(contract, &request.capability)?;
    let roles: Vec<&str> = contract.roles.iter().map(String::as_str).collect();
    let parameters: Vec<&str> = contract.parameters.iter().map(String::as_str).collect();
    validate_v1_multi_input_contract(&request, &roles, &parameters)?;
    let (request, verified_inputs) = convert_v1_request(contract, request, base_directory)?;
    let prepared = prepare_v2_request(contract, request, Path::new("."), &verified_inputs)?;
    execute_prepared_request(contract, prepared)
}

pub(super) fn execute_bulk_expression_v2(
    base_directory: &Path,
    request: JobRequestV2,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<String> {
    let contract = contract_for(
        BULK_EXPRESSION_PACK,
        BULK_EXPRESSION_PACK,
        WorkflowRuntimeKind::R,
    )?;
    execute_workflow_v2(&contract, base_directory, request, verified_inputs)
}

pub(super) fn execute_sequence_convert_v2(
    base_directory: &Path,
    request: JobRequestV2,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<String> {
    let contract = contract_for(
        SEQUENCE_CONVERT_PACK,
        SEQUENCE_CONVERT_PACK,
        WorkflowRuntimeKind::Python,
    )?;
    execute_workflow_v2(&contract, base_directory, request, verified_inputs)
}

pub(super) fn execute_medical_survival_v1(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    let contract = contract_for(
        MEDICAL_SURVIVAL_PACK,
        MEDICAL_SURVIVAL_PACK,
        WorkflowRuntimeKind::R,
    )?;
    execute_workflow_v1(&contract, base_directory, request)
}

pub(super) fn execute_medical_survival_v2(
    base_directory: &Path,
    request: JobRequestV2,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<String> {
    let contract = contract_for(
        MEDICAL_SURVIVAL_PACK,
        MEDICAL_SURVIVAL_PACK,
        WorkflowRuntimeKind::R,
    )?;
    execute_workflow_v2(&contract, base_directory, request, verified_inputs)
}

pub(super) fn execute_chemistry_descriptors_v1(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    let contract = contract_for(
        CHEMISTRY_DESCRIPTORS_PACK,
        CHEMISTRY_DESCRIPTORS_PACK,
        WorkflowRuntimeKind::Python,
    )?;
    execute_workflow_v1(&contract, base_directory, request)
}

pub(super) fn execute_chemistry_descriptors_v2(
    base_directory: &Path,
    request: JobRequestV2,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<String> {
    let contract = contract_for(
        CHEMISTRY_DESCRIPTORS_PACK,
        CHEMISTRY_DESCRIPTORS_PACK,
        WorkflowRuntimeKind::Python,
    )?;
    execute_workflow_v2(&contract, base_directory, request, verified_inputs)
}

fn execute_workflow_v2(
    contract: &WorkflowContract,
    base_directory: &Path,
    request: JobRequestV2,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<String> {
    ensure_supported_capability(contract, &request.capability)?;
    let prepared = prepare_v2_request(contract, request, base_directory, verified_inputs)?;
    execute_prepared_request(contract, prepared)
}

fn convert_v1_request(
    contract: &WorkflowContract,
    request: JobRequest,
    base_directory: &Path,
) -> WorkerResult<(JobRequestV2, BTreeMap<String, String>)> {
    let mut inputs = Vec::with_capacity(contract.roles.len());
    let mut verified_inputs = BTreeMap::new();
    for role in &contract.roles {
        let configured = request
            .inputs
            .get(role.as_str())
            .ok_or_else(|| format!("{} requires inputs.{role}", request.capability))?;
        let path = canonical_existing_input(base_directory, configured)?;
        let format = sequence_or_table_format_from_path(&path)?;
        let sha256 = sha256_file(&path)?;
        let file_id = format!("input-{role}-1");
        verified_inputs.insert(file_id.clone(), sha256.clone());
        inputs.push(InputArtifact {
            artifact_id: format!("input-{role}"),
            role: role.to_string(),
            cardinality: InputCardinality::Single,
            files: vec![ArtifactFile {
                file_id,
                path: path.to_string_lossy().into_owned(),
                role: None,
                format,
                compression: CompressionFormat::None,
                size_bytes: fs::metadata(&path)?.len(),
                modified_at: None,
                sha256: Some(sha256),
            }],
            dataset_id: None,
        });
    }

    Ok((
        JobRequestV2 {
            schema_version: "2".to_owned(),
            job_id: request.job_id,
            capability: request.capability,
            inputs,
            execution: request.execution,
            parameters: request.parameters,
        },
        verified_inputs,
    ))
}

fn prepare_v2_request(
    contract: &WorkflowContract,
    mut request: JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<PreparedWorkflowRequest> {
    if request.inputs.len() != contract.roles.len() {
        return Err(format!(
            "{} requires exactly {} input artifacts",
            request.capability,
            contract.roles.len()
        )
        .into());
    }

    let mut roles = BTreeSet::new();
    let mut inputs = Vec::with_capacity(contract.roles.len());
    let mut role_hashes = BTreeMap::new();
    for artifact in &mut request.inputs {
        if !contract.roles.contains(&artifact.role) {
            return Err(format!(
                "{} does not accept input role {}",
                request.capability, artifact.role
            )
            .into());
        }
        if !roles.insert(artifact.role.clone()) {
            return Err(format!("duplicate input role: {}", artifact.role).into());
        }
        if artifact.cardinality != InputCardinality::Single || artifact.files.len() != 1 {
            return Err(format!("input role {} requires exactly one file", artifact.role).into());
        }
        let file = &mut artifact.files[0];
        if !contract.input_formats.contains(&file.format) {
            return Err(format!("input role {} has unsupported format", artifact.role).into());
        }
        if file.compression != CompressionFormat::None {
            return Err("workflow does not support compressed inputs".into());
        }
        let path = canonical_existing_input(base_directory, &file.path)?;
        let actual_sha256 = sha256_file(&path)?;
        let verified_sha256 = verified_inputs
            .get(&file.file_id)
            .ok_or_else(|| format!("input {} was not verified", file.file_id))?;
        if actual_sha256 != *verified_sha256 {
            return Err(format!("input {} changed after verification", file.file_id).into());
        }
        file.path = path.to_string_lossy().into_owned();
        file.size_bytes = fs::metadata(&path)?.len();
        file.sha256 = Some(actual_sha256.clone());
        role_hashes.insert(artifact.role.clone(), actual_sha256.clone());
        inputs.push(PreparedInput {
            path,
            sha256: actual_sha256,
        });
    }
    let expected_roles: BTreeSet<String> = contract.roles.iter().cloned().collect();
    if roles != expected_roles {
        return Err(format!(
            "{} requires {} inputs",
            request.capability,
            contract.roles.join(" and ")
        )
        .into());
    }

    let output_directory = resolve_output_directory(base_directory, &request.parameters)?;
    for input in &inputs {
        if paths_equal(&input.path, &output_directory) {
            return Err("workflow output directory must differ from every input".into());
        }
    }
    request.parameters["output_directory"] =
        serde_json::Value::String(output_directory.to_string_lossy().into_owned());

    Ok(PreparedWorkflowRequest {
        request,
        output_directory,
        inputs,
        role_hashes,
    })
}

fn execute_prepared_request(
    contract: &WorkflowContract,
    prepared: PreparedWorkflowRequest,
) -> WorkerResult<String> {
    let pack = match load_verified_workflow_pack(contract) {
        Ok(pack) => pack,
        Err(error) => {
            return Ok(serde_json::to_string(&AnalysisResultV2::error(
                prepared.request.job_id,
                prepared.request.capability,
                "workflow_failed",
                error.to_string(),
                ExecutionMode::LocalCpu,
            ))?);
        }
    };
    if let Some(resume) = &pack.resume
        && let Some(envelope) = try_replay_resume(contract, &prepared, &pack, resume)?
    {
        return Ok(envelope);
    }
    if prepared.output_directory.exists() {
        return Err(format!(
            "refusing to overwrite workflow output directory: {}",
            prepared.output_directory.display()
        )
        .into());
    }
    if prepared.request.execution.mode == ExecutionMode::Container {
        return execute_prepared_request_in_container(contract, prepared, pack);
    }

    let temporary = TemporaryRequestDirectory::create()?;
    let request_path = temporary.write_request(&prepared.request)?;
    let output_directory = WorkflowOutputDirectory::new(prepared.output_directory.clone());
    let result_path = prepared.output_directory.join("result.json");
    let (variable, fallback) = match contract.runtime {
        WorkflowRuntimeKind::R => ("LINXIRA_BIO_WORKFLOW_R", "Rscript"),
        WorkflowRuntimeKind::Python => (
            "LINXIRA_BIO_WORKFLOW_PYTHON",
            if cfg!(windows) { "python" } else { "python3" },
        ),
        _ => return Err("workflow runtime kind is not implemented by this worker".into()),
    };
    let executable = env::var_os(variable)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from(fallback));
    let process = Command::new(executable)
        .arg(&pack.entrypoint)
        .arg("--request")
        .arg(&request_path)
        .arg("--result")
        .arg(&result_path)
        .current_dir(&pack.root)
        .env("LINXIRA_BIO_CORE_VERSION", env!("CARGO_PKG_VERSION"))
        .output()?;
    let result = read_result_envelope(&result_path, &process)?;
    finalize_workflow_result(
        contract,
        &prepared,
        &pack,
        process.status,
        output_directory,
        result,
    )
}

const CONTAINER_WORKFLOW_MOUNT: &str = "/linxira-bio/workflow";
const CONTAINER_REQUEST_MOUNT: &str = "/linxira-bio/request";
const CONTAINER_OUTPUT_MOUNT: &str = "/linxira-bio/output";
const CONTAINER_INPUT_MOUNT: &str = "/linxira-bio/input";

/// Resolve the container runtime: `LINXIRA_BIO_CONTAINER_RUNTIME` when set,
/// otherwise the first of `docker`/`podman` that answers `--version`.
fn container_runtime() -> WorkerResult<OsString> {
    if let Some(configured) = env::var_os("LINXIRA_BIO_CONTAINER_RUNTIME")
        && !configured.is_empty()
    {
        return Ok(configured);
    }
    for candidate in ["docker", "podman"] {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
        {
            return Ok(OsString::from(candidate));
        }
    }
    Err(
        "container execution requested but no container runtime (docker or podman) is available"
            .into(),
    )
}

fn container_image() -> WorkerResult<OsString> {
    env::var_os("LINXIRA_BIO_CONTAINER_IMAGE")
        .filter(|value| !value.is_empty())
        .ok_or("container execution requires LINXIRA_BIO_CONTAINER_IMAGE".into())
}

fn container_interpreter(kind: WorkflowRuntimeKind) -> OsString {
    if let Some(configured) = env::var_os("LINXIRA_BIO_CONTAINER_INTERPRETER")
        && !configured.is_empty()
    {
        return configured;
    }
    match kind {
        WorkflowRuntimeKind::R => OsString::from("Rscript"),
        _ => OsString::from("python3"),
    }
}

/// Run a workflow pack inside a container: the workflow root, request
/// directory, and every input file parent are mounted read-only; the output
/// parent is mounted read-write. Input and output paths in the container
/// request are rewritten to the fixed container mount layout.
fn execute_prepared_request_in_container(
    contract: &WorkflowContract,
    prepared: PreparedWorkflowRequest,
    pack: VerifiedWorkflowPack,
) -> WorkerResult<String> {
    let runtime = container_runtime()?;
    let image = container_image()?;
    let interpreter = container_interpreter(contract.runtime);
    let workflow_root = workflow_root()?;
    let entrypoint_relative = pack
        .entrypoint
        .strip_prefix(&pack.root)
        .map_err(|_| "container entrypoint must live inside the pack root")?
        .to_path_buf();
    let pack_directory = pack
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("workflow pack root has no directory name")?
        .to_owned();
    let output_name = prepared
        .output_directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("workflow output directory requires a final path component")?
        .to_owned();
    let output_parent = prepared
        .output_directory
        .parent()
        .ok_or("workflow output directory has no parent")?
        .to_path_buf();
    let container_output_dir = format!("{CONTAINER_OUTPUT_MOUNT}/{output_name}");
    let result_container_path = format!("{container_output_dir}/result.json");

    let mut request_value = serde_json::to_value(&prepared.request)?;
    let mut input_mounts = Vec::new();
    let mut input_index = 0;
    if let Some(artifacts) = request_value
        .get_mut("inputs")
        .and_then(serde_json::Value::as_array_mut)
    {
        for artifact in artifacts {
            if let Some(files) = artifact
                .get_mut("files")
                .and_then(serde_json::Value::as_array_mut)
            {
                for file in files {
                    let host = file
                        .get("path")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("container request input lacks a path")?;
                    let host_path = Path::new(host);
                    let parent = host_path
                        .parent()
                        .ok_or("container request input has no parent")?;
                    let name = host_path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .ok_or("container request input has no file name")?;
                    let mount_root = format!("{CONTAINER_INPUT_MOUNT}/{input_index}");
                    input_mounts.push((parent.to_path_buf(), mount_root.clone()));
                    file["path"] = serde_json::Value::String(format!("{mount_root}/{name}"));
                    input_index += 1;
                }
            }
        }
    }
    request_value["parameters"]["output_directory"] =
        serde_json::Value::String(container_output_dir.clone());

    let temporary = TemporaryRequestDirectory::create()?;
    let container_request_path = temporary.path.join("request.json");
    fs::write(&container_request_path, serde_json::to_vec(&request_value)?)?;

    let mut arguments = vec![
        OsString::from("run"),
        OsString::from("--rm"),
        OsString::from("-v"),
        OsString::from(format!(
            "{}:{CONTAINER_WORKFLOW_MOUNT}:ro",
            workflow_root.display()
        )),
        OsString::from("-v"),
        OsString::from(format!(
            "{}:{CONTAINER_REQUEST_MOUNT}:ro",
            temporary.path.display()
        )),
        OsString::from("-v"),
        OsString::from(format!(
            "{}:{CONTAINER_OUTPUT_MOUNT}:rw",
            output_parent.display()
        )),
    ];
    for (parent, mount) in &input_mounts {
        arguments.push(OsString::from("-v"));
        arguments.push(OsString::from(format!("{}:{mount}:ro", parent.display())));
    }
    arguments.push(OsString::from("-e"));
    arguments.push(OsString::from(format!(
        "LINXIRA_BIO_CORE_VERSION={}",
        env!("CARGO_PKG_VERSION")
    )));
    arguments.push(image);
    arguments.push(interpreter);
    arguments.push(OsString::from(format!(
        "{CONTAINER_WORKFLOW_MOUNT}/{pack_directory}/{}",
        entrypoint_relative.display()
    )));
    arguments.push(OsString::from("--request"));
    arguments.push(OsString::from(format!(
        "{CONTAINER_REQUEST_MOUNT}/request.json"
    )));
    arguments.push(OsString::from("--result"));
    arguments.push(OsString::from(result_container_path));

    let process = Command::new(&runtime)
        .args(&arguments)
        .output()
        .map_err(|error| {
            format!(
                "container runtime {} failed to start: {error}",
                runtime.to_string_lossy()
            )
        })?;
    let result_path = prepared.output_directory.join("result.json");
    let mut result_value = read_result_value(&result_path, &process)?;
    remap_container_artifact_paths(
        &mut result_value,
        &container_output_dir,
        &prepared.output_directory,
    )?;
    let mut result: AnalysisResultV2<serde_json::Value> = serde_json::from_value(result_value)?;
    result.provenance.execution_mode = ExecutionMode::Container;
    finalize_workflow_result(
        contract,
        &prepared,
        &pack,
        process.status,
        WorkflowOutputDirectory::new(prepared.output_directory.clone()),
        result,
    )
}

fn read_result_value(
    result_path: &Path,
    process: &std::process::Output,
) -> WorkerResult<serde_json::Value> {
    if !result_path.is_file() {
        return Err(format!(
            "workflow exited with {} without a result envelope: {}",
            process.status,
            stderr_summary(&process.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&fs::read(result_path)?)?)
}

fn read_result_envelope(
    result_path: &Path,
    process: &std::process::Output,
) -> WorkerResult<AnalysisResultV2<serde_json::Value>> {
    let value = read_result_value(result_path, process)?;
    Ok(serde_json::from_value(value)?)
}

/// Rewrite container-layout artifact paths back to their host locations so the
/// recorded envelope validates against the real output directory. Paths that
/// are already host paths (local container-runtime emulation) pass through;
/// the subsequent result validation enforces containment either way.
fn remap_container_artifact_paths(
    result: &mut serde_json::Value,
    container_output_dir: &str,
    host_output_dir: &Path,
) -> WorkerResult<()> {
    if let Some(artifacts) = result
        .get_mut("artifacts")
        .and_then(serde_json::Value::as_array_mut)
    {
        for artifact in artifacts {
            let Some(path) = artifact.get_mut("path") else {
                continue;
            };
            let Some(path_value) = path.as_str() else {
                continue;
            };
            if let Some(relative) = path_value.strip_prefix(container_output_dir) {
                *path =
                    serde_json::Value::String(format!("{}{}", host_output_dir.display(), relative));
            }
        }
    }
    Ok(())
}

/// Validate a produced result envelope, preserve the output directory on
/// success, record resume state, and serialize the envelope.
fn finalize_workflow_result(
    contract: &WorkflowContract,
    prepared: &PreparedWorkflowRequest,
    pack: &VerifiedWorkflowPack,
    process_status: std::process::ExitStatus,
    mut output_directory: WorkflowOutputDirectory,
    result: AnalysisResultV2<serde_json::Value>,
) -> WorkerResult<String> {
    ensure_inputs_unchanged(&prepared.inputs)?;
    validate_workflow_result(contract, &result, prepared, pack)?;
    match result.status {
        JobStatus::Ok if !process_status.success() => {
            return Err(format!(
                "workflow returned an ok envelope after process failure {}",
                process_status
            )
            .into());
        }
        JobStatus::Error if process_status.success() => {
            return Err(
                "workflow returned an error envelope after a successful process exit".into(),
            );
        }
        JobStatus::Ok | JobStatus::Error => {}
    }
    output_directory.preserve();
    if result.status == JobStatus::Ok
        && let Some(resume) = &pack.resume
    {
        let state = WorkflowResumeState {
            schema_version: "1".to_owned(),
            job_id: prepared.request.job_id.clone(),
            capability: prepared.request.capability.clone(),
            core_version: env!("CARGO_PKG_VERSION").to_owned(),
            execution_mode: prepared.request.execution.mode.clone(),
            input_sha256: prepared.role_hashes.clone(),
            dependency_lock_sha256: pack.dependency_lock_sha256.clone(),
            result: result.clone(),
        };
        let state_path = prepared.output_directory.join(&resume.state_file);
        fs::write(&state_path, serde_json::to_vec_pretty(&state)?)?;
    }
    Ok(serde_json::to_string(&result)?)
}

/// Replay a recorded resume state when it matches the current request, inputs,
/// core build, and dependency lock, and every recorded artifact still verifies.
/// Any mismatch (stale state, changed inputs, missing artifacts) falls through
/// to a fresh pack run.
fn try_replay_resume(
    contract: &WorkflowContract,
    prepared: &PreparedWorkflowRequest,
    pack: &VerifiedWorkflowPack,
    resume: &WorkflowResumeConfig,
) -> WorkerResult<Option<String>> {
    let state_path = prepared.output_directory.join(&resume.state_file);
    let bytes = match fs::read(&state_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let state: WorkflowResumeState = match serde_json::from_slice(&bytes) {
        Ok(state) => state,
        Err(_) => return Ok(None),
    };
    if state.schema_version != "1"
        || state.job_id != prepared.request.job_id
        || state.capability != prepared.request.capability
        || state.core_version != env!("CARGO_PKG_VERSION")
        || state.execution_mode != prepared.request.execution.mode
        || state.input_sha256 != prepared.role_hashes
        || state.dependency_lock_sha256 != pack.dependency_lock_sha256
        || state.result.status != JobStatus::Ok
    {
        return Ok(None);
    }
    if validate_workflow_result(contract, &state.result, prepared, pack).is_err() {
        return Ok(None);
    }
    Ok(Some(serde_json::to_string(&state.result)?))
}

fn validate_workflow_result(
    contract: &WorkflowContract,
    result: &AnalysisResultV2<serde_json::Value>,
    prepared: &PreparedWorkflowRequest,
    pack: &VerifiedWorkflowPack,
) -> WorkerResult<()> {
    if result.schema_version != "2"
        || result.job_id != prepared.request.job_id
        || result.capability != prepared.request.capability
    {
        return Err("workflow output identity does not match the request".into());
    }
    if result.capability == MEDICAL_BULK_CAPABILITY
        && !result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "research_use_only"
                && diagnostic.severity == DiagnosticSeverity::Warning
        })
    {
        return Err("medical bulk RNA-seq result lacks the research-use-only warning".into());
    }

    match result.status {
        JobStatus::Ok => validate_success_result(contract, result, prepared, pack),
        JobStatus::Error => {
            if !result.artifacts.is_empty()
                || !result
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            {
                return Err("workflow error envelope lacks an error diagnostic".into());
            }
            Ok(())
        }
    }
}

fn validate_success_result(
    contract: &WorkflowContract,
    result: &AnalysisResultV2<serde_json::Value>,
    prepared: &PreparedWorkflowRequest,
    pack: &VerifiedWorkflowPack,
) -> WorkerResult<()> {
    if result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err("workflow ok envelope contains an error diagnostic".into());
    }
    if result.provenance.input_sha256 != prepared.role_hashes {
        return Err("workflow provenance input hashes do not match verified inputs".into());
    }
    if result.provenance.core_version.as_deref() != Some(env!("CARGO_PKG_VERSION")) {
        return Err("workflow provenance core version does not match this build".into());
    }
    if result.provenance.dependency_lock_sha256.as_deref()
        != Some(pack.dependency_lock_sha256.as_str())
    {
        return Err("workflow provenance dependency lock hash is invalid".into());
    }
    if result.artifacts.len() != contract.artifact_count {
        return Err(format!(
            "workflow must produce exactly {} artifacts",
            contract.artifact_count
        )
        .into());
    }

    let output_root = fs::canonicalize(&prepared.output_directory)?;
    let mut roles = BTreeSet::new();
    for artifact in &result.artifacts {
        if artifact.kind != contract.artifact_kind
            || !artifact
                .format
                .is_some_and(|format| contract.artifact_formats.contains(&format))
            || contract
                .artifact_media_type
                .as_deref()
                .is_some_and(|expected| artifact.media_type.as_deref() != Some(expected))
        {
            return Err("workflow produced an unexpected artifact kind or format".into());
        }
        if !roles.insert(artifact.role.as_str()) {
            return Err(format!("workflow repeats artifact role {}", artifact.role).into());
        }
        let declared_path = Path::new(&artifact.path);
        if !declared_path.is_absolute() {
            return Err("workflow artifact paths must be absolute".into());
        }
        let path = fs::canonicalize(declared_path)?;
        if !path.starts_with(&output_root) || path == output_root {
            return Err(format!(
                "workflow artifact escapes output directory: {}",
                artifact.path
            )
            .into());
        }
        let size = fs::metadata(&path)?.len();
        let hash = sha256_file(&path)?;
        if artifact.size_bytes != Some(size) || artifact.sha256.as_deref() != Some(hash.as_str()) {
            return Err(format!("workflow artifact metadata mismatch: {}", artifact.path).into());
        }
    }
    let expected_roles: BTreeSet<&str> =
        contract.artifact_roles.iter().map(String::as_str).collect();
    if roles != expected_roles {
        return Err(format!(
            "workflow returned unexpected artifact roles: {}",
            roles.into_iter().collect::<Vec<_>>().join(", ")
        )
        .into());
    }
    Ok(())
}

fn ensure_inputs_unchanged(inputs: &[PreparedInput]) -> WorkerResult<()> {
    for input in inputs {
        if sha256_file(&input.path)? != input.sha256 {
            return Err(format!(
                "input changed while the workflow was running: {}",
                input.path.display()
            )
            .into());
        }
    }
    Ok(())
}

fn load_verified_workflow_pack(contract: &WorkflowContract) -> WorkerResult<VerifiedWorkflowPack> {
    let workflow_root = workflow_root()?;
    let pack_root = safe_pack_path(&workflow_root, &contract.pack_directory)?;
    let manifest_path = safe_pack_path(&pack_root, BULK_EXPRESSION_MANIFEST)?;
    let manifest: WorkflowPackManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.schema_version != "2"
        || manifest.id != contract.pack_id
        || manifest.runtime.kind != contract.runtime
        || manifest.network.access != NetworkAccess::None
    {
        return Err(format!(
            "workflow manifest identity or policy is invalid: {}",
            manifest_path.display()
        )
        .into());
    }
    if !core_compatibility_matches(
        &manifest.runtime.core_compatibility,
        env!("CARGO_PKG_VERSION"),
    ) {
        return Err(format!(
            "workflow pack {} requires core {} but this build is {}",
            manifest.id,
            manifest.runtime.core_compatibility,
            env!("CARGO_PKG_VERSION")
        )
        .into());
    }
    if manifest.entrypoint.arguments.as_slice()
        != ["--request", "{request}", "--result", "{result}"]
    {
        return Err("workflow has unsupported entrypoint arguments".into());
    }
    let dependency_lock_sha256 = verify_workflow_pack_files(&pack_root, &manifest)?;
    let entrypoint = safe_pack_path(&pack_root, &manifest.entrypoint.path)?;
    Ok(VerifiedWorkflowPack {
        root: pack_root,
        entrypoint,
        dependency_lock_sha256,
        resume: manifest.resume.clone(),
    })
}

fn workflow_root() -> WorkerResult<PathBuf> {
    if let Some(configured) = env::var_os("LINXIRA_BIO_WORKFLOW_ROOT") {
        if configured.is_empty() {
            return Err("LINXIRA_BIO_WORKFLOW_ROOT must not be empty".into());
        }
        return canonical_workflow_root(Path::new(&configured));
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = env::current_exe()
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join("workflows"));
        candidates.push(directory.join("resources/workflows"));
        candidates.push(directory.join("../share/linxira-bio/workflows"));
    }
    if let Ok(directory) = env::current_dir() {
        candidates.push(directory.join("workflows"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../workflows"));

    for candidate in candidates {
        if let Ok(root) = canonical_workflow_root(&candidate) {
            return Ok(root);
        }
    }
    Err("could not locate bundled workflows; set LINXIRA_BIO_WORKFLOW_ROOT".into())
}

fn canonical_workflow_root(path: &Path) -> WorkerResult<PathBuf> {
    let root = fs::canonicalize(path)?;
    if !root.is_dir()
        || !root
            .join("org.linxira.bulk-expression-deseq2")
            .join(BULK_EXPRESSION_MANIFEST)
            .is_file()
    {
        return Err(format!("workflow root is invalid: {}", path.display()).into());
    }
    Ok(root)
}

fn verify_workflow_pack_files(
    pack_root: &Path,
    manifest: &WorkflowPackManifest,
) -> WorkerResult<String> {
    let mut declared = BTreeSet::new();
    for file in &manifest.files {
        validate_sha256(&file.sha256, &file.path)?;
        if !declared.insert(file.path.as_str()) {
            return Err(format!("workflow manifest repeats file path: {}", file.path).into());
        }
        let path = safe_pack_path(pack_root, &file.path)?;
        if sha256_file(&path)? != file.sha256.to_ascii_lowercase() {
            return Err(format!("workflow file verification failed: {}", file.path).into());
        }
    }
    for required in [
        manifest.entrypoint.path.as_str(),
        manifest.runtime.dependency_lock.path.as_str(),
    ] {
        if !declared.contains(required) {
            return Err(
                format!("workflow manifest does not declare required file: {required}").into(),
            );
        }
    }
    validate_sha256(
        &manifest.runtime.dependency_lock.sha256,
        &manifest.runtime.dependency_lock.path,
    )?;
    let lock_path = safe_pack_path(pack_root, &manifest.runtime.dependency_lock.path)?;
    let actual_lock_hash = sha256_file(&lock_path)?;
    if actual_lock_hash != manifest.runtime.dependency_lock.sha256.to_ascii_lowercase() {
        return Err("workflow dependency lock hash does not match manifest".into());
    }
    Ok(actual_lock_hash)
}

fn validate_sha256(value: &str, context: &str) -> WorkerResult<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("workflow SHA-256 is invalid for {context}").into());
    }
    Ok(())
}

fn safe_pack_path(root: &Path, relative: &str) -> WorkerResult<PathBuf> {
    let candidate = Path::new(relative);
    if candidate.as_os_str().is_empty()
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("workflow path escapes pack root: {relative}").into());
    }
    let canonical_root = fs::canonicalize(root)?;
    let resolved = fs::canonicalize(canonical_root.join(candidate))?;
    if resolved != canonical_root && !resolved.starts_with(&canonical_root) {
        return Err(format!("workflow path escapes pack root: {relative}").into());
    }
    Ok(resolved)
}

fn resolve_output_directory(
    base_directory: &Path,
    parameters: &serde_json::Value,
) -> WorkerResult<PathBuf> {
    let configured = parameters
        .get("output_directory")
        .and_then(serde_json::Value::as_str)
        .ok_or("bulk expression workflow requires string parameters.output_directory")?;
    if configured.trim().is_empty() {
        return Err("parameters.output_directory must not be empty".into());
    }
    let configured = PathBuf::from(configured);
    let candidate = if configured.is_absolute() {
        configured
    } else {
        base_directory.join(configured)
    };
    let name = candidate
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or("workflow output directory requires a final path component")?;
    let parent = candidate
        .parent()
        .ok_or("workflow output directory has no parent")?;
    let parent = fs::canonicalize(parent)?;
    if !parent.is_dir() {
        return Err("workflow output parent is not a directory".into());
    }
    Ok(parent.join(name))
}

fn canonical_existing_input(base_directory: &Path, configured: &str) -> WorkerResult<PathBuf> {
    if configured.trim().is_empty() {
        return Err("workflow input path must not be empty".into());
    }
    let configured = PathBuf::from(configured);
    let candidate = if configured.is_absolute() {
        configured
    } else {
        base_directory.join(configured)
    };
    let path = fs::canonicalize(candidate)?;
    if !path.is_file() {
        return Err(format!("workflow input is not a file: {}", path.display()).into());
    }
    Ok(path)
}

fn table_format_from_path(path: &Path) -> WorkerResult<BioDataFormat> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("csv") => Ok(BioDataFormat::Csv),
        Some("tsv" | "tab") => Ok(BioDataFormat::Tsv),
        _ => Err("bulk expression workflow inputs must use .csv, .tsv, or .tab".into()),
    }
}

fn ensure_supported_capability(contract: &WorkflowContract, capability: &str) -> WorkerResult<()> {
    if contract
        .capabilities
        .iter()
        .any(|candidate| candidate == capability)
    {
        Ok(())
    } else {
        Err(format!("unsupported workflow capability: {capability}").into())
    }
}

fn sequence_or_table_format_from_path(path: &Path) -> WorkerResult<BioDataFormat> {
    if let Ok(format) = table_format_from_path(path) {
        return Ok(format);
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "fa" | "fasta" | "fna" => Ok(BioDataFormat::Fasta),
        "fq" | "fastq" => Ok(BioDataFormat::Fastq),
        "gb" | "gbk" | "genbank" => Ok(BioDataFormat::Genbank),
        "embl" => Ok(BioDataFormat::Embl),
        "sdf" => Ok(BioDataFormat::Sdf),
        _ => Err(
            format!("cannot infer a supported input format from extension: .{extension}").into(),
        ),
    }
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

fn stderr_summary(stderr: &[u8]) -> String {
    let message = String::from_utf8_lossy(stderr)
        .trim()
        .replace(['\r', '\n'], " ");
    if message.is_empty() {
        return "no stderr output".to_owned();
    }
    message.chars().take(2_048).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        BULK_EXPRESSION_PACK, SEQUENCE_CONVERT_PACK, TemporaryRequestDirectory,
        WorkflowRuntimeKind, bulk_expression_fallback_contract, contract_for,
        sequence_convert_fallback_contract, verify_workflow_pack_files,
    };
    use linxira_bio_protocol::WorkflowPackManifest;
    use sha2::{Digest, Sha256};
    use std::fs;

    #[test]
    fn loads_execution_contracts_from_real_pack_manifests() {
        let bulk = contract_for(
            BULK_EXPRESSION_PACK,
            BULK_EXPRESSION_PACK,
            WorkflowRuntimeKind::R,
        )
        .expect("bulk expression contract");
        let fallback = bulk_expression_fallback_contract();
        assert_eq!(bulk.roles, fallback.roles);
        assert_eq!(bulk.parameters, fallback.parameters);
        assert_eq!(bulk.artifact_count, fallback.artifact_count);
        assert_eq!(bulk.artifact_roles, fallback.artifact_roles);
        assert_eq!(bulk.artifact_kind, fallback.artifact_kind);
        assert_eq!(bulk.artifact_formats, fallback.artifact_formats);
        assert_eq!(bulk.artifact_media_type, fallback.artifact_media_type);
        assert!(
            bulk.capabilities
                .iter()
                .any(|capability| capability == "expression.differential.v1")
        );
        assert!(
            bulk.capabilities
                .iter()
                .any(|capability| capability == "medical.bulk-rnaseq.v1")
        );

        let convert = contract_for(
            SEQUENCE_CONVERT_PACK,
            SEQUENCE_CONVERT_PACK,
            WorkflowRuntimeKind::Python,
        )
        .expect("sequence convert contract");
        let fallback = sequence_convert_fallback_contract();
        assert_eq!(convert.roles, fallback.roles);
        assert_eq!(convert.parameters, fallback.parameters);
        assert_eq!(convert.artifact_roles, fallback.artifact_roles);
        assert_eq!(convert.artifact_kind, fallback.artifact_kind);
        assert_eq!(convert.artifact_formats, fallback.artifact_formats);
        assert!(
            convert
                .capabilities
                .iter()
                .any(|capability| capability == "sequence.convert.biopython.v1")
        );
    }

    #[test]
    fn rejects_a_tampered_manifest_declared_workflow_file() {
        let temporary = TemporaryRequestDirectory::create().expect("temporary pack");
        let script = temporary.path.join("run.R");
        let lock = temporary.path.join("dependencies.lock.json");
        fs::write(&script, "original\n").expect("script");
        fs::write(&lock, "{}\n").expect("lock");
        let script_hash = format!("{:x}", Sha256::digest(b"original\n"));
        let lock_hash = format!("{:x}", Sha256::digest(b"{}\n"));
        let manifest: WorkflowPackManifest = serde_json::from_value(serde_json::json!({
            "schema_version": "2",
            "id": "org.linxira.test",
            "version": "1.0.0",
            "publisher": {"name": "Linxira OS"},
            "license": "AGPL-3.0-or-later",
            "entrypoint": {
                "path": "run.R",
                "arguments": ["--request", "{request}", "--result", "{result}"]
            },
            "runtime": {
                "kind": "r",
                "version": ">=4.6.1,<4.7.0",
                "core_compatibility": ">=0.1.0,<1.0.0",
                "dependency_lock": {"path": "dependencies.lock.json", "sha256": lock_hash}
            },
            "input_schema": {},
            "output_schema": {},
            "platforms": ["windows-gnu", "debian", "arch"],
            "network": {"access": "none", "allowed_hosts": []},
            "resources": {"gpu": "none"},
            "files": [
                {"path": "run.R", "sha256": script_hash},
                {"path": "dependencies.lock.json", "sha256": lock_hash}
            ]
        }))
        .expect("manifest");

        verify_workflow_pack_files(&temporary.path, &manifest).expect("verified pack");
        fs::write(&script, "tampered\n").expect("tamper script");
        let error = verify_workflow_pack_files(&temporary.path, &manifest)
            .expect_err("tampered script must fail");
        assert!(
            error
                .to_string()
                .contains("workflow file verification failed")
        );
    }
}
