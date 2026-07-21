#![forbid(unsafe_code)]

//! Stable job and result contracts shared by CLI, GUI, workers, and agents.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: &str = "1";

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub engine_version: String,
    pub execution_mode: ExecutionMode,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalysisResult<T>
where
    T: Serialize,
{
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

#[cfg(test)]
mod tests {
    use super::{ExecutionMode, JobRequest, SCHEMA_VERSION};

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
}
