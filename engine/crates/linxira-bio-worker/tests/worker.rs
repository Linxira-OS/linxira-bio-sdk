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

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}
