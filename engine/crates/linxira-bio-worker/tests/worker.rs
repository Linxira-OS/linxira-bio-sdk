use std::path::PathBuf;
use std::process::Command;

#[test]
fn executes_sequence_statistics_job() {
    let request = workspace_root().join("tests/fixtures/jobs/sequence-stats.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run worker");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(stdout.contains("\"job_id\":\"fixture-sequence-stats\""));
    assert!(stdout.contains("\"capability\":\"sequence.stats.v1\""));
    assert!(stdout.contains("\"total_bases\":12"));
    assert!(stdout.contains("\"execution_mode\":\"local-cpu\""));
}

#[test]
fn executes_dataset_inspection_job() {
    let request = workspace_root().join("tests/fixtures/jobs/dataset-inspect.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run worker");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");

    assert_eq!(result["job_id"], "fixture-dataset-inspect");
    assert_eq!(result["capability"], "dataset.inspect.v1");
    assert_eq!(result["result"]["format"], "fasta");
    assert_eq!(result["result"]["support"], "supported");
    assert_eq!(result["result"]["preview"]["records_shown"], 1);
    assert_eq!(result["result"]["preview"]["truncated"], true);
}

#[test]
fn executes_artifact_aware_v2_job() {
    let request = workspace_root().join("tests/fixtures/jobs/dataset-inspect-v2.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run v2 worker request");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");

    assert_eq!(result["schema_version"], "2");
    assert_eq!(result["job_id"], "fixture-dataset-inspect-v2");
    assert_eq!(result["result"]["format"], "vcf");
    assert!(result["artifacts"].is_array());
    assert!(result["diagnostics"].is_array());
    assert_eq!(
        result["provenance"]["input_sha256"]["input-file-1"]
            .as_str()
            .map(str::len),
        Some(64)
    );
}

#[test]
fn executes_v2_sequence_transform_with_relative_output_and_hashes() {
    let root = workspace_root();
    let output_path = root.join("target/test-results/sequence-reverse-complement-v2.fa");
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("create sequence result directory");
    }
    if output_path.exists() {
        std::fs::remove_file(&output_path).expect("remove stale sequence result");
    }
    let request = root.join("tests/fixtures/jobs/sequence-reverse-complement-v2.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run v2 sequence transform");

    assert!(output.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid v2 sequence result");
    assert_eq!(result["schema_version"], "2");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["capability"], "sequence.reverse-complement.v1");
    assert_eq!(result["result"]["output_records"], 3);
    assert_eq!(result["artifacts"][0]["role"], "fasta");
    assert_eq!(result["artifacts"][0]["kind"], "domain-file");
    assert_eq!(result["artifacts"][0]["format"], "fasta");
    assert_eq!(result["artifacts"][0]["media_type"], "text/x-fasta");
    assert_eq!(
        result["artifacts"][0]["size_bytes"],
        std::fs::metadata(&output_path)
            .expect("sequence output metadata")
            .len()
    );
    assert_eq!(
        result["artifacts"][0]["sha256"].as_str().map(str::len),
        Some(64)
    );
    assert_eq!(
        result["provenance"]["input_sha256"]["input-fasta-1"],
        "d36ea1364a0451fd99584f0f36307dbc34b818ca4008c24c7705a95855172a1c"
    );
    assert_eq!(
        std::fs::read_to_string(&output_path).expect("reverse-complement FASTA"),
        ">one\nNNACGT\n>two\nCCCC\n>three\nAT\n"
    );
    std::fs::remove_file(output_path).expect("remove v2 sequence result");
}

#[test]
fn returns_structured_v2_validation_errors_from_the_binary() {
    let request = temporary_request_path("v2-validation-error");
    std::fs::write(
        &request,
        r#"{
            "schema_version": "2",
            "job_id": "fixture-v2-validation-error",
            "capability": "sequence.stats.v1",
            "inputs": [{
                "artifact_id": "invalid-single-input",
                "role": "fasta",
                "cardinality": "single",
                "files": []
            }],
            "execution": {"mode": "local-cpu"},
            "parameters": {}
        }"#,
    )
    .expect("write v2 validation fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(&request)
        .output()
        .expect("run invalid v2 worker request");
    std::fs::remove_file(request).expect("remove v2 validation fixture");

    assert!(output.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("structured v2 error result");
    assert_eq!(result["schema_version"], "2");
    assert_eq!(result["job_id"], "fixture-v2-validation-error");
    assert_eq!(result["capability"], "sequence.stats.v1");
    assert_eq!(result["status"], "error");
    assert_eq!(result["result"], serde_json::json!({}));
    assert_eq!(result["artifacts"], serde_json::json!([]));
    assert_eq!(result["provenance"]["execution_mode"], "local-cpu");
    assert_eq!(result["diagnostics"].as_array().map(Vec::len), Some(1));
    assert_eq!(result["diagnostics"][0]["code"], "job-failed");
    assert_eq!(result["diagnostics"][0]["severity"], "error");
    assert!(
        result["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("cardinality"))
    );
}

#[test]
fn malformed_json_remains_a_process_error_without_an_envelope() {
    let request = temporary_request_path("malformed-json");
    std::fs::write(&request, b"{not-json").expect("write malformed request");

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(&request)
        .output()
        .expect("run malformed worker request");
    std::fs::remove_file(request).expect("remove malformed request");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("error:"));
}

#[test]
fn exports_a_table_through_the_worker() {
    let root = workspace_root();
    let output_path = root.join("target/test-results/metrics.csv");
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("create result directory");
    }
    let request = root.join("tests/fixtures/jobs/table-export.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run table export job");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");
    assert_eq!(result["capability"], "table.export.v1");
    assert_eq!(result["result"]["format"], "csv");
    assert!(
        std::fs::metadata(&output_path)
            .expect("output metadata")
            .len()
            > 0
    );
    std::fs::remove_file(output_path).expect("remove exported fixture");
}

#[test]
fn executes_fastq_qc_job() {
    let request = workspace_root().join("tests/fixtures/jobs/fastq-qc.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run FASTQ QC worker job");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid result");
    assert_eq!(result["capability"], "fastq.qc.v1");
    assert_eq!(result["result"]["read_count"], 2);
    assert_eq!(result["result"]["quality_encoding"], "phred+33");
}

#[test]
fn executes_variant_statistics_job() {
    let request = workspace_root().join("tests/fixtures/jobs/variant-stats.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run VCF statistics worker job");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid result");
    assert_eq!(result["capability"], "variant.stats.v1");
    assert_eq!(result["result"]["record_count"], 7);
    assert_eq!(result["result"]["sample_count"], 2);
}

#[test]
fn executes_pdb_structure_summary_job() {
    let request = workspace_root().join("tests/fixtures/jobs/structure-pdb-summary.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run PDB structure summary job");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid result");
    assert_eq!(result["capability"], "structure.pdb.summary.v1");
    assert_eq!(result["result"]["model_count"], 1);
    assert_eq!(result["result"]["alphafold_confidence"]["residue_count"], 2);
    assert_eq!(result["result"]["atoms"][3]["record"], "hetatm");
}

#[test]
fn executes_alignment_qc_job() {
    let request = workspace_root().join("tests/fixtures/jobs/alignment-qc.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run alignment QC job");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid result");
    assert_eq!(result["capability"], "alignment.qc.v1");
    assert_eq!(result["result"]["record_count"], 5);
    assert_eq!(result["result"]["mapped_record_count"], 4);
}

#[test]
fn executes_interval_intersection_job() {
    let request = workspace_root().join("tests/fixtures/jobs/interval-intersect.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run interval intersection job");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid result");
    assert_eq!(result["capability"], "interval.intersect.v1");
    assert_eq!(result["result"]["overlap_pair_count"], 3);
    assert_eq!(result["result"]["right_overlapped_count"], 2);
}

#[test]
fn executes_expression_matrix_qc_job() {
    let request = workspace_root().join("tests/fixtures/jobs/expression-matrix-qc.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run expression matrix QC job");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid result");
    assert_eq!(result["capability"], "expression.matrix.qc.v1");
    assert_eq!(result["result"]["feature_count"], 4);
    assert_eq!(result["result"]["sample_count"], 3);
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}

fn temporary_request_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "linxira-bio-worker-{name}-{}.json",
        std::process::id()
    ))
}
