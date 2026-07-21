#![forbid(unsafe_code)]

//! Execute versioned Linxira Bio jobs through one shared local worker API.

use linxira_bio_core::environment::{audit_environment, plan_environment};
use linxira_bio_core::sequence::fasta_stats;
use linxira_bio_protocol::{AnalysisResult, ExecutionMode, JobRequest, SCHEMA_VERSION};
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub type WorkerError = Box<dyn Error + Send + Sync>;
pub type WorkerResult<T> = Result<T, WorkerError>;

pub fn execute_path(request_path: &Path) -> WorkerResult<String> {
    let request_file = File::open(request_path)?;
    let request: JobRequest = serde_json::from_reader(BufReader::new(request_file))?;
    execute_request(
        request,
        request_path.parent().unwrap_or_else(|| Path::new(".")),
    )
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
        "environment.plan.v1" => run_environment_plan(request),
        "sequence.stats.v1" => run_sequence_stats(base_directory, request),
        capability => Err(format!("unsupported capability: {capability}").into()),
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

fn run_environment_plan(request: JobRequest) -> WorkerResult<String> {
    let profile = request
        .parameters
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("full-local");
    let audit = audit_environment()?;
    let plan = plan_environment(profile, &audit)?;
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
    let stats = fasta_stats(BufReader::new(File::open(input_path)?))?;
    let result = AnalysisResult::ok(
        request.job_id,
        request.capability,
        stats,
        ExecutionMode::LocalCpu,
    );
    Ok(serde_json::to_string(&result)?)
}

fn resolve_input(base_directory: &Path, input: &str) -> PathBuf {
    let input_path = PathBuf::from(input);
    if input_path.is_absolute() {
        input_path
    } else {
        base_directory.join(input_path)
    }
}
