use std::path::PathBuf;
use std::process::Command;
use std::{fs, process};

#[test]
fn prints_top_level_help_successfully() {
    for flag in ["-h", "--help"] {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .arg(flag)
            .output()
            .expect("run linxira-bio help");

        assert!(output.status.success(), "help flag {flag}");
        assert!(output.stderr.is_empty(), "help flag {flag}");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 help output");
        assert!(stdout.contains("linxira-bio sequence stats"));
        assert!(stdout.contains("linxira-bio export table"));
    }
}

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
fn inspects_a_dataset_as_json() {
    let fixture = workspace_root().join("tests/fixtures/data-inspection/variants.vcf");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["dataset", "inspect"])
        .arg(fixture)
        .arg("--json")
        .output()
        .expect("run dataset inspection");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON result");

    assert_eq!(result["capability"], "dataset.inspect.v1");
    assert_eq!(result["result"]["format"], "vcf");
    assert_eq!(result["result"]["preview"]["kind"], "variant");
}

#[test]
fn exports_result_json_to_csv_and_xlsx() {
    let root = std::env::temp_dir().join(format!("linxira-cli-export-{}", process::id()));
    fs::create_dir_all(&root).expect("create export directory");
    let input = root.join("result.json");
    fs::write(
        &input,
        r#"{"schema_version":"1","result":{"sequence_count":3,"gc_percent":60.0}}"#,
    )
    .expect("write result fixture");

    for extension in ["csv", "xlsx"] {
        let output_path = root.join(format!("result.{extension}"));
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .args(["export", "table"])
            .arg(&input)
            .arg(&output_path)
            .output()
            .expect("run export");
        assert!(output.status.success(), "export {extension}");
        assert!(fs::metadata(output_path).expect("export metadata").len() > 0);
    }

    fs::remove_dir_all(root).expect("remove export directory");
}

#[test]
fn reports_fastq_quality_control_as_json() {
    let fixture = workspace_root().join("tests/fixtures/fastq-qc/valid.fastq");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["fastq", "qc"])
        .arg(fixture)
        .args(["--quality-encoding", "phred+33", "--json"])
        .output()
        .expect("run FASTQ QC");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(result["capability"], "fastq.qc.v1");
    assert_eq!(result["result"]["read_count"], 2);
}

#[test]
fn reports_variant_statistics_as_json() {
    let fixture = workspace_root().join("tests/fixtures/variant-stats/mixed.vcf");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["variant", "stats"])
        .arg(fixture)
        .arg("--json")
        .output()
        .expect("run variant statistics");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(result["capability"], "variant.stats.v1");
    assert_eq!(result["result"]["record_count"], 7);
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
