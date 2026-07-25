use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fs, process};

static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

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
        assert!(stdout.contains("linxira-bio sequence reverse-complement"));
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
fn processes_fastq_reads_as_json() {
    let root = workspace_root();
    let fixture = root.join("tests/fixtures/fastq-transform/reads.fastq");
    let temp = temporary_directory("fastq-transform");
    let trim_output = temp.join("trimmed.fastq");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["fastq", "trim"])
        .arg(&fixture)
        .arg(&trim_output)
        .args([
            "--min-quality",
            "20",
            "--min-length",
            "4",
            "--quality-encoding",
            "phred+33",
            "--json",
        ])
        .output()
        .expect("run FASTQ trim");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid FASTQ trim JSON");
    assert_eq!(result["capability"], "fastq.trim.v1");
    assert_eq!(result["result"]["output_read_count"], 2);
    assert_eq!(result["result"]["discarded_read_count"], 1);
    assert_eq!(result["result"]["quality_trimmed_bases"], 6);
    assert_eq!(
        fs::read_to_string(&trim_output).expect("trimmed FASTQ"),
        "@trim\nACGT\n+\nIIII\n@adapter\nTTTTAGATCGGA\n+\nIIIIIIIIIIII\n"
    );

    let adapter_output = temp.join("adapter-trimmed.fastq");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["fastq", "adapter-trim"])
        .arg(&fixture)
        .arg(&adapter_output)
        .args([
            "--adapter",
            "AGATCGGA",
            "--min-overlap",
            "4",
            "--min-length",
            "1",
            "--json",
        ])
        .output()
        .expect("run FASTQ adapter trim");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid FASTQ adapter JSON");
    assert_eq!(result["capability"], "fastq.adapter.v1");
    assert_eq!(result["result"]["output_read_count"], 3);
    assert_eq!(result["result"]["adapter_trimmed_bases"], 8);
    assert_eq!(
        fs::read_to_string(&adapter_output).expect("adapter-trimmed FASTQ"),
        "@trim\nACGTAC\n+\nIIII!!\n@adapter\nTTTT\n+\nIIII\n@drop\nACGT\n+\n!!!!\n"
    );
    fs::remove_dir_all(temp).expect("remove FASTQ transform directory");
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
fn reports_render_ready_pdb_summary_as_json() {
    let fixture = workspace_root().join("tests/fixtures/structure-pdb-summary/alphafold-style.pdb");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["structure", "pdb"])
        .arg(fixture)
        .args(["--alphafold-plddt", "--json"])
        .output()
        .expect("run PDB summary");

    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(result["capability"], "structure.pdb.summary.v1");
    assert_eq!(result["result"]["atom_count"], 4);
    assert_eq!(result["result"]["atoms"][0]["position"]["x"], 11.104);
    assert_eq!(result["result"]["alphafold_confidence"]["mean_plddt"], 70.0);
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

#[test]
fn summarizes_sam_alignment_quality_as_json() {
    let input = workspace_root().join("tests/fixtures/alignment-qc/valid.sam");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["alignment", "qc"])
        .arg(input)
        .arg("--json")
        .output()
        .expect("run alignment QC");

    assert!(output.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid alignment result");
    assert_eq!(result["capability"], "alignment.qc.v1");
    assert_eq!(result["result"]["record_count"], 5);
    assert_eq!(result["result"]["mapped_record_count"], 4);
}

#[test]
fn intersects_bed_intervals_as_json() {
    let root = workspace_root();
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["interval", "intersect"])
        .arg(root.join("tests/fixtures/interval-intersect/left.bed"))
        .arg(root.join("tests/fixtures/interval-intersect/right.bed"))
        .arg("--json")
        .output()
        .expect("run BED intersection");

    assert!(output.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid intersection result");
    assert_eq!(result["capability"], "interval.intersect.v1");
    assert_eq!(result["result"]["overlap_pair_count"], 3);
    assert_eq!(result["result"]["total_overlap_bases"], 12);

    let temp = temporary_directory("interval-ops");
    let merge_input = temp.join("merge.bed");
    let merge_output = temp.join("merged.bed");
    fs::write(&merge_input, b"chr1\t0\t5\nchr1\t5\t10\nchr1\t12\t14\n").expect("write merge BED");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["interval", "merge"])
        .arg(&merge_input)
        .arg(&merge_output)
        .arg("--json")
        .output()
        .expect("run BED merge");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid merge result");
    assert_eq!(result["capability"], "interval.merge.v1");
    assert_eq!(result["result"]["output_interval_count"], 2);
    assert_eq!(
        fs::read_to_string(&merge_output).expect("merged BED"),
        "chr1\t0\t10\nchr1\t12\t14\n"
    );

    let subtract_output = temp.join("subtracted.bed");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["interval", "subtract"])
        .arg(root.join("tests/fixtures/interval-intersect/left.bed"))
        .arg(root.join("tests/fixtures/interval-intersect/right.bed"))
        .arg(&subtract_output)
        .arg("--json")
        .output()
        .expect("run BED subtraction");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid subtraction result");
    assert_eq!(result["capability"], "interval.subtract.v1");
    assert_eq!(result["result"]["output_interval_count"], 3);
    assert!(
        fs::metadata(&subtract_output)
            .expect("subtracted BED")
            .len()
            > 0
    );
    fs::remove_dir_all(temp).expect("remove interval ops directory");
}

#[test]
fn summarizes_expression_matrix_as_json() {
    let input = workspace_root().join("tests/fixtures/expression-matrix/counts.tsv");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["expression", "matrix-qc"])
        .arg(input)
        .arg("--json")
        .output()
        .expect("run expression matrix QC");

    assert!(output.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid expression result");
    assert_eq!(result["capability"], "expression.matrix.qc.v1");
    assert_eq!(result["result"]["feature_count"], 4);
    assert_eq!(result["result"]["sample_count"], 3);
    assert_eq!(result["result"]["missing_value_count"], 1);
}

#[test]
fn manipulates_delimited_tables_as_json() {
    let root = workspace_root();
    let temp = temporary_directory("table-manipulate");
    let output_path = temp.join("selected.csv");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["table", "manipulate"])
        .arg(root.join("tests/fixtures/expression-matrix/counts.tsv"))
        .arg(&output_path)
        .args([
            "--select-column",
            "gene_id",
            "--select-column",
            "sample_b",
            "--filter-column",
            "sample_b",
            "--filter-op",
            "contains",
            "--filter-value",
            "5",
            "--output-delimiter",
            "csv",
            "--json",
        ])
        .output()
        .expect("run table manipulation");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid table manipulation result");
    assert_eq!(result["capability"], "table.manipulate.v1");
    assert_eq!(result["result"]["input_rows"], 4);
    assert_eq!(result["result"]["output_rows"], 1);
    assert_eq!(result["result"]["filtered_rows"], 3);
    assert_eq!(
        fs::read_to_string(&output_path).expect("manipulated table"),
        "gene_id,sample_b\ngene_2,5\n"
    );
    fs::remove_dir_all(temp).expect("remove table manipulation directory");
}

#[test]
fn executes_all_sequence_transform_commands_as_json() {
    let root = temporary_directory("sequence-transforms");
    let input = root.join("input.fa");
    fs::write(
        &input,
        b">gene description\nATGAAATAA\n>gc\nGCGCGC\n>short\nNN\n",
    )
    .expect("write sequence transform input");

    let extract_output = root.join("extract.fa");
    let extract = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "extract"])
        .arg(&input)
        .arg(&extract_output)
        .args(["--id", "gene", "--region", "gene:1-3", "--strict", "--json"])
        .output()
        .expect("run sequence extraction");
    assert!(
        extract.status.success(),
        "{}",
        String::from_utf8_lossy(&extract.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&extract.stdout).expect("valid extraction result");
    assert_eq!(result["capability"], "sequence.extract.v1");
    assert_eq!(result["result"]["output_records"], 2);
    assert_eq!(result["result"]["emitted_region_count"], 1);
    assert_eq!(
        fs::read_to_string(&extract_output).expect("extracted FASTA"),
        ">gene description\nATGAAATAA\n>gene:1-3:+\nATG\n"
    );

    let filter_output = root.join("filter.fa");
    let filter = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "filter"])
        .arg(&input)
        .arg(&filter_output)
        .args([
            "--min-length",
            "6",
            "--min-gc-percent",
            "50",
            "--max-n-percent",
            "0",
            "--json",
        ])
        .output()
        .expect("run sequence filtering");
    assert!(
        filter.status.success(),
        "{}",
        String::from_utf8_lossy(&filter.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&filter.stdout).expect("valid filter result");
    assert_eq!(result["capability"], "sequence.filter.v1");
    assert_eq!(result["result"]["output_records"], 1);
    assert_eq!(
        fs::read_to_string(&filter_output).expect("filtered FASTA"),
        ">gc\nGCGCGC\n"
    );

    let reverse_output = root.join("reverse.fa");
    let reverse = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "reverse-complement"])
        .arg(&input)
        .arg(&reverse_output)
        .arg("--json")
        .output()
        .expect("run reverse complement");
    assert!(
        reverse.status.success(),
        "{}",
        String::from_utf8_lossy(&reverse.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&reverse.stdout).expect("valid reverse result");
    assert_eq!(result["capability"], "sequence.reverse-complement.v1");
    assert_eq!(result["result"]["output_records"], 3);
    assert!(
        fs::read_to_string(&reverse_output)
            .expect("reverse-complement FASTA")
            .contains(">gene description\nTTATTTCAT\n")
    );

    let translate_output = root.join("translate.fa");
    let translate = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "translate"])
        .arg(&input)
        .arg(&translate_output)
        .args([
            "--frame",
            "1",
            "--frame",
            "-1",
            "--trim-terminal-stop",
            "--json",
        ])
        .output()
        .expect("run sequence translation");
    assert!(
        translate.status.success(),
        "{}",
        String::from_utf8_lossy(&translate.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&translate.stdout).expect("valid translation result");
    assert_eq!(result["capability"], "sequence.translate.v1");
    assert_eq!(result["result"]["frames"], serde_json::json!([1, -1]));
    assert!(
        fs::read_to_string(&translate_output)
            .expect("translated FASTA")
            .contains(">gene|frame=+1\nMK\n")
    );

    let orf_output = root.join("orf.fa");
    let orf = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "orf"])
        .arg(&input)
        .arg(&orf_output)
        .args(["--min-amino-acids", "2", "--forward-only", "--json"])
        .output()
        .expect("run ORF search");
    assert!(
        orf.status.success(),
        "{}",
        String::from_utf8_lossy(&orf.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&orf.stdout).expect("valid ORF result");
    assert_eq!(result["capability"], "sequence.orf.v1");
    assert_eq!(result["result"]["complete_orfs"], 1);
    assert!(
        fs::read_to_string(&orf_output)
            .expect("ORF FASTA")
            .contains("strand=+ frame=+1 start=1 end=9 complete\nMK\n")
    );

    let normalized_output = root.join("normalized.fa");
    let normalized = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "normalize-ids"])
        .arg(&input)
        .arg(&normalized_output)
        .args(["--prefix", "seq", "--start", "5", "--width", "2", "--json"])
        .output()
        .expect("run ID normalization");
    assert!(
        normalized.status.success(),
        "{}",
        String::from_utf8_lossy(&normalized.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&normalized.stdout).expect("valid normalize result");
    assert_eq!(result["capability"], "sequence.id.normalize.v1");
    assert_eq!(result["result"]["last_index"], 7);
    assert!(
        fs::read_to_string(&normalized_output)
            .expect("normalized FASTA")
            .contains(">seq05 description\nATGAAATAA\n")
    );

    let extra = root.join("extra.fa");
    fs::write(&extra, b">extra\nAC\n").expect("write merge input");
    let merged_output = root.join("merged.fa");
    let merged = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "merge"])
        .arg(&merged_output)
        .arg(&input)
        .arg(&extra)
        .arg("--json")
        .output()
        .expect("run FASTA merge");
    assert!(
        merged.status.success(),
        "{}",
        String::from_utf8_lossy(&merged.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&merged.stdout).expect("valid merge result");
    assert_eq!(result["capability"], "sequence.merge.v1");
    assert_eq!(result["result"]["input_files"], 2);
    assert_eq!(result["result"]["output_records"], 4);

    let split_directory = root.join("split");
    let split = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "split"])
        .arg(&input)
        .arg(&split_directory)
        .args(["--records-per-file", "2", "--prefix", "shard", "--json"])
        .output()
        .expect("run FASTA split");
    assert!(
        split.status.success(),
        "{}",
        String::from_utf8_lossy(&split.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&split.stdout).expect("valid split result");
    assert_eq!(result["capability"], "sequence.split.v1");
    assert_eq!(result["result"]["output_files"], 2);
    assert!(split_directory.join("shard_001.fa").is_file());
    assert!(split_directory.join("shard_002.fa").is_file());

    let table_output = root.join("sequences.tsv");
    let table = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "to-table"])
        .arg(&input)
        .arg(&table_output)
        .args(["--delimiter", "tsv", "--json"])
        .output()
        .expect("run FASTA to table");
    assert!(
        table.status.success(),
        "{}",
        String::from_utf8_lossy(&table.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&table.stdout).expect("valid to-table result");
    assert_eq!(result["capability"], "sequence.to-table.v1");
    assert_eq!(result["result"]["output_rows"], 3);
    assert!(
        fs::read_to_string(&table_output)
            .expect("sequence table")
            .starts_with("id\tdescription\tlength\tsequence\n")
    );

    let from_table_output = root.join("from-table.fa");
    let from_table = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "from-table"])
        .arg(&table_output)
        .arg(&from_table_output)
        .args(["--delimiter", "tsv", "--json"])
        .output()
        .expect("run table to FASTA");
    assert!(
        from_table.status.success(),
        "{}",
        String::from_utf8_lossy(&from_table.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&from_table.stdout).expect("valid from-table result");
    assert_eq!(result["capability"], "sequence.from-table.v1");
    assert_eq!(result["result"]["output_records"], 3);
    assert!(
        fs::read_to_string(&from_table_output)
            .expect("roundtripped FASTA")
            .contains(">gene description\nATGAAATAA\n")
    );

    fs::remove_dir_all(root).expect("remove sequence transform directory");
}

#[test]
fn sequence_transforms_reject_invalid_options_and_existing_outputs() {
    let root = temporary_directory("sequence-transform-errors");
    let input = root.join("input.fa");
    let protected = root.join("protected.fa");
    let invalid_output = root.join("invalid.fa");
    fs::write(&input, b">sequence\nACGT\n").expect("write input FASTA");
    fs::write(&protected, b"do not replace\n").expect("write protected output");

    let overwrite = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "reverse-complement"])
        .arg(&input)
        .arg(&protected)
        .arg("--json")
        .output()
        .expect("run protected reverse complement");
    assert!(!overwrite.status.success());
    assert!(String::from_utf8_lossy(&overwrite.stderr).contains("refusing to overwrite"));
    assert_eq!(
        fs::read_to_string(&protected).expect("protected output remains"),
        "do not replace\n"
    );

    let invalid = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "filter"])
        .arg(&input)
        .arg(&invalid_output)
        .args(["--min-gc-percent", "101", "--json"])
        .output()
        .expect("run invalid sequence filter");
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("between 0 and 100"));
    assert!(!invalid_output.exists());

    fs::remove_dir_all(root).expect("remove sequence transform error directory");
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
}

fn temporary_directory(name: &str) -> PathBuf {
    let ordinal = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "linxira-bio-cli-{name}-{}-{ordinal}",
        process::id()
    ));
    fs::create_dir(&path).expect("create temporary CLI test directory");
    path
}
