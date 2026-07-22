use std::path::PathBuf;
use std::process::Command;

#[test]
fn reports_sequence_statistics_as_json() {
    let fixture = workspace_root().join("tests/fixtures/sequences/tiny.fa");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "stats"])
        .arg(fixture)
        .arg("--json")
        .output()
        .expect("run linxira-bio");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");

    assert_eq!(result["capability"], "sequence.stats.v1");
    assert_eq!(result["result"]["sequence_count"], 3);
    assert_eq!(result["result"]["total_bases"], 12);
    assert_eq!(result["result"]["n50"], 6);
    assert_eq!(result["result"]["gc_percent"], 60.0);
}

#[test]
fn exposes_available_and_planned_capabilities() {
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["capabilities", "--json"])
        .output()
        .expect("run linxira-bio");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(stdout.contains("\"sequence.stats.v1\""));
    assert!(stdout.contains("\"protein.af3.server.v1\""));
    assert!(stdout.contains("\"authenticated-browser\""));
}

#[test]
fn audits_registered_environment_tools_as_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["environment", "audit", "--json"])
        .output()
        .expect("run environment audit");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");
    let tools = result["result"]["tools"].as_array().expect("tool checks");

    assert_eq!(result["capability"], "environment.audit.v1");
    assert!(tools.iter().any(|tool| tool["id"] == "python"));
    assert!(tools.iter().any(|tool| tool["id"] == "r"));
    assert!(tools.iter().any(|tool| tool["id"] == "ncbi-blast"));
    assert!(tools.iter().any(|tool| tool["id"] == "diamond"));
    if cfg!(target_os = "windows") {
        assert!(tools.iter().any(|tool| tool["id"] == "wsl-arch"));
        assert!(tools.iter().any(|tool| tool["id"] == "wsl-debian"));
    }
}

#[test]
fn plans_sequence_search_environment_as_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["environment", "plan", "sequence-search", "--json"])
        .output()
        .expect("run environment plan");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");
    let actions = result["result"]["actions"]
        .as_array()
        .expect("installation actions");

    assert_eq!(result["capability"], "environment.plan.v1");
    assert_eq!(result["result"]["profile"], "sequence-search");
    assert!(
        actions
            .iter()
            .any(|action| action["tool_id"] == "ncbi-blast")
    );
    assert!(actions.iter().any(|action| action["tool_id"] == "diamond"));
    assert_eq!(result["result"]["mode"], "managed-user");
    assert_eq!(result["result"]["transaction"]["dry_run"], true);
    assert_eq!(result["result"]["transaction"]["apply_available"], false);
}

#[test]
fn previews_a_project_isolated_environment_as_json() {
    let project_root = workspace_root();
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args([
            "environment",
            "plan",
            "sequence-search",
            "--mode",
            "project-isolated",
            "--project-root",
        ])
        .arg(&project_root)
        .arg("--json")
        .output()
        .expect("run project environment plan");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");

    assert_eq!(result["result"]["mode"], "project-isolated");
    assert!(
        result["result"]["target_root"]
            .as_str()
            .is_some_and(|root| root.contains(".linxira-bio"))
    );
    assert!(
        result["result"]["transaction"]["lock_path"]
            .as_str()
            .is_some_and(|path| path.contains("runtime-lock.json"))
    );
    assert_eq!(result["result"]["transaction"]["preserves_existing"], true);
}

#[test]
fn lists_cataloged_managed_runtimes_as_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["runtime", "catalog", "--json"])
        .output()
        .expect("run runtime catalog");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let catalog: serde_json::Value = serde_json::from_str(&stdout).expect("valid catalog JSON");
    let providers = catalog["providers"].as_array().expect("runtime providers");

    assert_eq!(catalog["default_scope"], "user");
    assert!(
        providers
            .iter()
            .any(|provider| provider["id"] == "python-uv")
    );
    assert!(
        providers
            .iter()
            .any(|provider| provider["id"] == "java-temurin-21")
    );
    assert!(
        providers
            .iter()
            .all(|provider| provider["status"] == "cataloged")
    );
}

#[test]
fn preserves_doctor_v1_json_shape() {
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let doctor: serde_json::Value = serde_json::from_str(&stdout).expect("valid doctor JSON");

    assert_eq!(doctor["schema_version"], "1");
    assert_eq!(doctor["product"], "linxira-bio-sdk");
    assert!(doctor.get("capability").is_none());
    assert!(doctor["tools"].is_array());
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}
