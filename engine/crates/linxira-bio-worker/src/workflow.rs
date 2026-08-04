use super::{WorkerResult, sha256_file, validate_v1_multi_input_contract};
use linxira_bio_protocol::{
    AnalysisResultV2, ArtifactFile, BioDataFormat, CompressionFormat, DiagnosticSeverity,
    InputArtifact, InputCardinality, JobRequest, JobRequestV2, JobStatus, NetworkAccess,
    OutputArtifactKind, WorkflowPackManifest, WorkflowRuntimeKind,
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const BULK_EXPRESSION_PACK_ID: &str = "org.linxira.bulk-expression-deseq2";
const BULK_EXPRESSION_PACK_DIRECTORY: &str = "org.linxira.bulk-expression-deseq2";
const BULK_EXPRESSION_MANIFEST: &str = "manifest.json";
const DIFFERENTIAL_CAPABILITY: &str = "expression.differential.v1";
const MEDICAL_BULK_CAPABILITY: &str = "medical.bulk-rnaseq.v1";
const WORKFLOW_PARAMETERS: &[&str] = &[
    "output_directory",
    "feature_id_column",
    "sample_id_column",
    "condition_column",
    "reference_level",
    "contrast_level",
    "alpha",
    "min_total_count",
];

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

struct VerifiedWorkflowPack {
    root: PathBuf,
    entrypoint: PathBuf,
    dependency_lock_sha256: String,
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
        Self {
            path,
            preserve: false,
        }
    }

    fn preserve(&mut self) {
        self.preserve = true;
    }
}

impl Drop for WorkflowOutputDirectory {
    fn drop(&mut self) {
        if !self.preserve && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub(super) fn execute_bulk_expression_v1(
    base_directory: &Path,
    request: JobRequest,
) -> WorkerResult<String> {
    ensure_supported_capability(&request.capability)?;
    validate_v1_multi_input_contract(
        &request,
        &["counts", "sample_metadata"],
        WORKFLOW_PARAMETERS,
    )?;
    let (request, verified_inputs) = convert_v1_request(request, base_directory)?;
    let prepared = prepare_v2_request(request, Path::new("."), &verified_inputs)?;
    execute_prepared_request(prepared)
}

pub(super) fn execute_bulk_expression_v2(
    base_directory: &Path,
    request: JobRequestV2,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<String> {
    ensure_supported_capability(&request.capability)?;
    let prepared = prepare_v2_request(request, base_directory, verified_inputs)?;
    execute_prepared_request(prepared)
}

fn convert_v1_request(
    request: JobRequest,
    base_directory: &Path,
) -> WorkerResult<(JobRequestV2, BTreeMap<String, String>)> {
    let mut inputs = Vec::with_capacity(2);
    let mut verified_inputs = BTreeMap::new();
    for role in ["counts", "sample_metadata"] {
        let configured = request
            .inputs
            .get(role)
            .ok_or_else(|| format!("{} requires inputs.{role}", request.capability))?;
        let path = canonical_existing_input(base_directory, configured)?;
        let format = table_format_from_path(&path)?;
        let sha256 = sha256_file(&path)?;
        let file_id = format!("input-{role}-1");
        verified_inputs.insert(file_id.clone(), sha256.clone());
        inputs.push(InputArtifact {
            artifact_id: format!("input-{role}"),
            role: role.to_owned(),
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
    mut request: JobRequestV2,
    base_directory: &Path,
    verified_inputs: &BTreeMap<String, String>,
) -> WorkerResult<PreparedWorkflowRequest> {
    if request.inputs.len() != 2 {
        return Err(format!(
            "{} requires exactly two input artifacts",
            request.capability
        )
        .into());
    }

    let mut roles = BTreeSet::new();
    let mut inputs = Vec::with_capacity(2);
    let mut role_hashes = BTreeMap::new();
    for artifact in &mut request.inputs {
        if !matches!(artifact.role.as_str(), "counts" | "sample_metadata") {
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
        if !matches!(file.format, BioDataFormat::Csv | BioDataFormat::Tsv) {
            return Err(format!("input role {} requires csv or tsv format", artifact.role).into());
        }
        if file.compression != CompressionFormat::None {
            return Err("bulk expression workflow does not support compressed inputs".into());
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
    if roles != BTreeSet::from(["counts".to_owned(), "sample_metadata".to_owned()]) {
        return Err("bulk expression workflow requires counts and sample_metadata inputs".into());
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

fn execute_prepared_request(prepared: PreparedWorkflowRequest) -> WorkerResult<String> {
    let pack = load_verified_workflow_pack()?;
    let temporary = TemporaryRequestDirectory::create()?;
    let request_path = temporary.write_request(&prepared.request)?;
    let mut output_directory = WorkflowOutputDirectory::new(prepared.output_directory.clone());
    let result_path = prepared.output_directory.join("result.json");
    let executable = env::var_os("LINXIRA_BIO_WORKFLOW_R")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| OsString::from("Rscript"));
    let process = Command::new(executable)
        .arg(&pack.entrypoint)
        .arg("--request")
        .arg(&request_path)
        .arg("--result")
        .arg(&result_path)
        .current_dir(&pack.root)
        .output()?;

    if !result_path.is_file() {
        return Err(format!(
            "bulk expression workflow exited with {} without a result envelope: {}",
            process.status,
            stderr_summary(&process.stderr)
        )
        .into());
    }
    ensure_inputs_unchanged(&prepared.inputs)?;
    let result: AnalysisResultV2<serde_json::Value> =
        serde_json::from_slice(&fs::read(&result_path)?)?;
    validate_workflow_result(&result, &prepared, &pack)?;
    match result.status {
        JobStatus::Ok if !process.status.success() => {
            return Err(format!(
                "workflow returned an ok envelope after process failure {}",
                process.status
            )
            .into());
        }
        JobStatus::Error if process.status.success() => {
            return Err(
                "workflow returned an error envelope after a successful process exit".into(),
            );
        }
        JobStatus::Ok | JobStatus::Error => {}
    }
    output_directory.preserve();
    Ok(serde_json::to_string(&result)?)
}

fn validate_workflow_result(
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
        JobStatus::Ok => validate_success_result(result, prepared, pack),
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
    if result.provenance.dependency_lock_sha256.as_deref()
        != Some(pack.dependency_lock_sha256.as_str())
    {
        return Err("workflow provenance dependency lock hash is invalid".into());
    }
    if result.artifacts.len() != 2 {
        return Err("bulk expression workflow must produce exactly two artifacts".into());
    }

    let output_root = fs::canonicalize(&prepared.output_directory)?;
    let mut roles = BTreeSet::new();
    for artifact in &result.artifacts {
        if artifact.kind != OutputArtifactKind::Table
            || artifact.format != Some(BioDataFormat::Csv)
            || artifact.media_type.as_deref() != Some("text/csv")
        {
            return Err("bulk expression workflow artifacts must be CSV tables".into());
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
    if roles != BTreeSet::from(["differential-expression", "normalized-counts"]) {
        return Err("bulk expression workflow returned unexpected artifact roles".into());
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

fn load_verified_workflow_pack() -> WorkerResult<VerifiedWorkflowPack> {
    let workflow_root = workflow_root()?;
    let pack_root = safe_pack_path(&workflow_root, BULK_EXPRESSION_PACK_DIRECTORY)?;
    let manifest_path = safe_pack_path(&pack_root, BULK_EXPRESSION_MANIFEST)?;
    let manifest: WorkflowPackManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    if manifest.schema_version != "1"
        || manifest.id != BULK_EXPRESSION_PACK_ID
        || manifest.runtime.kind != WorkflowRuntimeKind::R
        || manifest.network.access != NetworkAccess::None
    {
        return Err("bulk expression workflow manifest identity or policy is invalid".into());
    }
    if manifest.entrypoint.arguments.as_slice()
        != ["--request", "{request}", "--result", "{result}"]
    {
        return Err("bulk expression workflow has unsupported entrypoint arguments".into());
    }
    let dependency_lock_sha256 = verify_workflow_pack_files(&pack_root, &manifest)?;
    let entrypoint = safe_pack_path(&pack_root, &manifest.entrypoint.path)?;
    Ok(VerifiedWorkflowPack {
        root: pack_root,
        entrypoint,
        dependency_lock_sha256,
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
            .join(BULK_EXPRESSION_PACK_DIRECTORY)
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
    if candidate.exists() {
        return Err(format!(
            "refusing to overwrite workflow output directory: {}",
            candidate.display()
        )
        .into());
    }
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
    let output = parent.join(name);
    if output.exists() {
        return Err(format!(
            "refusing to overwrite workflow output directory: {}",
            output.display()
        )
        .into());
    }
    Ok(output)
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

fn ensure_supported_capability(capability: &str) -> WorkerResult<()> {
    if matches!(
        capability,
        DIFFERENTIAL_CAPABILITY | MEDICAL_BULK_CAPABILITY
    ) {
        Ok(())
    } else {
        Err(format!("unsupported bulk expression capability: {capability}").into())
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
    use super::{TemporaryRequestDirectory, verify_workflow_pack_files};
    use linxira_bio_protocol::WorkflowPackManifest;
    use sha2::{Digest, Sha256};
    use std::fs;

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
            "schema_version": "1",
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
