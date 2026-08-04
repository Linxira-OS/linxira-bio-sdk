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
        assert!(stdout.contains("linxira-bio workflow packs"));
    }
}

#[test]
fn lists_bundled_workflow_packs_without_claiming_installation() {
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["workflow", "packs", "--json"])
        .output()
        .expect("list workflow packs");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packs: serde_json::Value = serde_json::from_slice(&output.stdout).expect("workflow JSON");
    let packs = packs.as_array().expect("workflow pack array");
    assert!(packs.iter().any(|pack| {
        pack["id"] == "org.linxira.sequence-conversion-biopython"
            && pack["status"] == "cataloged"
            && pack["runtime"] == "python"
    }));
    assert!(packs.iter().any(|pack| {
        pack["id"] == "org.linxira.bulk-expression-deseq2"
            && pack["capability"] == "expression.differential.v1"
            && pack["capability_aliases"]
                == serde_json::json!(["medical.bulk-rnaseq.v1", "expression.deseq2.v1"])
            && pack["status"] == "cataloged"
            && pack["runtime"] == "r"
    }));
}

#[test]
fn workflow_runner_rejects_a_request_for_the_wrong_capability_before_starting_runtime() {
    let root = workspace_root();
    let temporary = temporary_directory("workflow-wrong-capability");
    let request = temporary.join("request.json");
    let result = temporary.join("result.json");
    fs::write(
        &request,
        serde_json::json!({
            "schema_version": "2",
            "job_id": "workflow-wrong-capability",
            "capability": "sequence.stats.v1",
            "inputs": [],
            "execution": {"mode": "local-cpu"},
            "parameters": {}
        })
        .to_string(),
    )
    .expect("write workflow request");

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args([
            "workflow",
            "run",
            "org.linxira.sequence-conversion-biopython",
        ])
        .arg(&request)
        .arg(&result)
        .env("LINXIRA_BIO_WORKFLOW_ROOT", root.join("workflows"))
        .output()
        .expect("run workflow validation");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("schema v2 request for the selected capability")
    );
    assert!(!result.exists());
    fs::remove_dir_all(temporary).expect("remove workflow test directory");
}

#[test]
fn workflow_runner_returns_the_packed_python_result_envelope() {
    let root = workspace_root();
    let temporary = temporary_directory("workflow-python-envelope");
    let input = root.join("tests/fixtures/sequences/tiny.fa");
    let request = temporary.join("request.json");
    let output_directory = temporary.join("converted");
    let result = output_directory.join("result.json");
    let input_size = fs::metadata(&input).expect("input metadata").len();
    fs::write(
        &request,
        serde_json::json!({
            "schema_version": "2",
            "job_id": "workflow-python-envelope",
            "capability": "sequence.convert.biopython.v1",
            "inputs": [{
                "artifact_id": "sequences-artifact",
                "role": "sequences",
                "cardinality": "single",
                "files": [{
                    "file_id": "sequences-file",
                    "path": input,
                    "format": "fasta",
                    "compression": "none",
                    "size_bytes": input_size
                }]
            }],
            "execution": {"mode": "local-cpu"},
            "parameters": {
                "output_directory": output_directory,
                "output_filename": "converted.fasta",
                "output_format": "fasta"
            }
        })
        .to_string(),
    )
    .expect("write workflow request");

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args([
            "workflow",
            "run",
            "org.linxira.sequence-conversion-biopython",
        ])
        .arg(&request)
        .arg(&result)
        .env("LINXIRA_BIO_WORKFLOW_ROOT", root.join("workflows"))
        .output()
        .expect("run packed Python workflow");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("workflow JSON");
    assert_eq!(result["schema_version"], "2");
    assert_eq!(result["job_id"], "workflow-python-envelope");
    assert_eq!(result["capability"], "sequence.convert.biopython.v1");
    assert!(matches!(result["status"].as_str(), Some("ok" | "error")));
    fs::remove_dir_all(temporary).expect("remove workflow test directory");
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
fn runs_set_and_protein_analysis_as_json() {
    let root = workspace_root();
    let set_input = root.join("tests/fixtures/set-analysis/sets.tsv");
    for (command, capability) in [("venn", "set.venn.v1"), ("upset", "set.upset.v1")] {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .args(["set", command])
            .arg(&set_input)
            .args(["--include-items", "--json"])
            .output()
            .expect("run set analysis");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid set JSON");
        assert_eq!(result["capability"], capability);
        assert_eq!(result["result"]["set_count"], 3);
        assert_eq!(result["result"]["union_size"], 6);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["protein", "properties"])
        .arg(root.join("tests/fixtures/protein/proteins.fa"))
        .arg("--json")
        .output()
        .expect("run protein properties");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid protein JSON");
    assert_eq!(result["capability"], "protein.properties.v1");
    assert_eq!(result["result"]["sequence_count"], 2);
    assert!(result["result"]["records"][0]["molecular_weight_da"].is_number());
    assert!(result["result"]["records"][1]["molecular_weight_da"].is_null());
    assert_eq!(result["warnings"].as_array().map(Vec::len), Some(1));
}

#[test]
fn runs_coordinate_structure_analysis_as_json() {
    let root = workspace_root().join("tests/fixtures/structure-analysis");

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["structure", "mmcif-summary"])
        .arg(root.join("reference.cif"))
        .arg("--json")
        .output()
        .expect("run mmCIF summary");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("mmCIF JSON");
    assert_eq!(result["capability"], "structure.mmcif.summary.v1");
    assert_eq!(result["result"]["atom_count"], 5);
    assert_eq!(result["warnings"].as_array().map(Vec::len), Some(1));

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["structure", "sequence"])
        .arg(root.join("reference.pdb"))
        .arg("--json")
        .output()
        .expect("run structure sequence extraction");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("sequence JSON");
    assert_eq!(result["capability"], "structure.sequence.extract.v1");
    assert_eq!(result["result"]["chains"][0]["sequence"], "AGSV");

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["structure", "contact-map"])
        .arg(root.join("reference.pdb"))
        .args(["--cutoff", "6", "--json"])
        .output()
        .expect("run contact map");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("contact JSON");
    assert_eq!(result["capability"], "structure.contact-map.v1");
    assert_eq!(result["result"]["contact_count"], 6);

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["structure", "geometry"])
        .arg(root.join("reference.pdb"))
        .args(["--atom", "A/1/CA", "--atom", "A/2/CA", "--json"])
        .output()
        .expect("run structure geometry");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("geometry JSON");
    assert_eq!(result["capability"], "structure.geometry.v1");
    assert_eq!(result["result"]["value"], 4.0);

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["structure", "superpose"])
        .arg(root.join("reference.pdb"))
        .arg(root.join("mobile.pdb"))
        .arg("--json")
        .output()
        .expect("run structure superposition");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("superpose JSON");
    assert_eq!(result["capability"], "structure.superpose.v1");
    assert_eq!(result["result"]["matched_atom_count"], 4);
    assert!(result["result"]["rmsd_after_angstrom"].as_f64().unwrap() < 1e-9);
}

#[test]
fn runs_kmer_epcr_and_variant_transform_capabilities() {
    let workspace = workspace_root();
    let output_root = temporary_directory("sequence-variant-analysis");

    let kmer_output = output_root.join("kmers.tsv");
    let result = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["sequence", "kmer-count"])
        .arg(workspace.join("tests/fixtures/sequence-analysis/reference.fa"))
        .arg(&kmer_output)
        .args(["--k", "3", "--canonical", "--top-n", "5", "--json"])
        .output()
        .expect("run k-mer counting");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("k-mer JSON");
    assert_eq!(json["capability"], "sequence.kmer.count.v1");
    assert_eq!(json["result"]["k"], 3);
    assert!(kmer_output.exists());

    let epcr_output = output_root.join("amplicons.tsv");
    let result = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["primer", "epcr"])
        .arg(workspace.join("tests/fixtures/sequence-analysis/reference.fa"))
        .arg(workspace.join("tests/fixtures/sequence-analysis/primers.tsv"))
        .arg(&epcr_output)
        .args(["--max-amplicon", "100", "--json"])
        .output()
        .expect("run ePCR");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("ePCR JSON");
    assert_eq!(json["capability"], "primer.epcr.v1");
    assert!(json["result"]["amplicon_count"].as_u64().unwrap() > 0);

    let filtered_output = output_root.join("filtered.vcf");
    let input_vcf = workspace.join("tests/fixtures/variant-transform/variants.vcf");
    let result = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["variant", "filter"])
        .arg(&input_vcf)
        .arg(&filtered_output)
        .args([
            "--min-qual",
            "20",
            "--pass-only",
            "--min-info-dp",
            "10",
            "--json",
        ])
        .output()
        .expect("run VCF filter");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("filter JSON");
    assert_eq!(json["capability"], "variant.filter.v1");
    assert_eq!(json["result"]["output_records"], 2);

    let normalized_output = output_root.join("normalized.vcf");
    let result = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["variant", "normalize"])
        .arg(&input_vcf)
        .arg(workspace.join("tests/fixtures/variant-transform/reference.fa"))
        .arg(&normalized_output)
        .arg("--json")
        .output()
        .expect("run VCF normalization");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&result.stdout).expect("normalize JSON");
    assert_eq!(json["capability"], "variant.normalize.v1");
    assert_eq!(json["result"]["left_aligned_records"], 1);
    assert!(
        fs::read_to_string(&normalized_output)
            .unwrap()
            .contains("chr1\t1\tins1\tA\tAA")
    );

    fs::remove_dir_all(output_root).expect("remove analysis directory");
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
fn runs_annotation_statistics_and_sequence_extraction() {
    let root = workspace_root();
    let annotation = root.join("tests/fixtures/annotation/genes.gff3");
    let reference = root.join("tests/fixtures/annotation/reference.fa");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["annotation", "stats"])
        .arg(&annotation)
        .arg("--json")
        .output()
        .expect("run annotation stats");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid annotation stats JSON");
    assert_eq!(result["capability"], "annotation.gxf.stats.v1");
    assert_eq!(result["result"]["record_count"], 10);
    assert_eq!(result["result"]["feature_type_counts"]["gene"], 2);

    let temp = temporary_directory("annotation-extract");
    let normalized = temp.join("normalized.gff3");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["annotation", "normalize"])
        .arg(&annotation)
        .arg(&normalized)
        .args(["--sort", "--json"])
        .output()
        .expect("run annotation normalization");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid normalization JSON");
    assert_eq!(result["capability"], "annotation.gxf.normalize.v1");
    assert_eq!(result["result"]["output_record_count"], 10);
    assert!(
        fs::read_to_string(&normalized)
            .expect("normalized GFF3")
            .starts_with("##gff-version 3\n")
    );

    let positions = temp.join("positions.tsv");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["annotation", "positions"])
        .arg(&annotation)
        .arg(&positions)
        .args(["--feature-type", "gene", "--json"])
        .output()
        .expect("run annotation positions");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid positions JSON");
    assert_eq!(result["capability"], "annotation.gene-position.v1");
    assert_eq!(result["result"]["output_record_count"], 2);
    assert!(
        fs::read_to_string(&positions)
            .expect("gene positions")
            .contains("g1\tGene1\tchr1\t2\t12")
    );

    let fasta = temp.join("exons.fa");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["annotation", "extract"])
        .arg(&annotation)
        .arg(&reference)
        .arg(&fasta)
        .args(["--feature-type", "exon", "--json"])
        .output()
        .expect("run annotation extraction");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid annotation extraction JSON");
    assert_eq!(result["capability"], "annotation.sequence.extract.v1");
    assert_eq!(result["result"]["output_sequence_count"], 2);
    let extracted = fs::read_to_string(fasta).expect("annotation FASTA output");
    assert!(extracted.contains(">t1 feature=exon"));
    assert!(extracted.contains("CGTACGT"));
    fs::remove_dir_all(temp).expect("remove annotation extraction directory");
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

    let duplicate_input = temp.join("duplicates.fastq");
    fs::write(
        &duplicate_input,
        "@one:AAAA\nACGT\n+\nIIII\n@copy:AAAA\nACGT\n+\nIIII\n@other:CCCC\nACGT\n+\nIIII\n",
    )
    .expect("write duplicate FASTQ");
    let deduplicated_output = temp.join("deduplicated.fastq");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["fastq", "deduplicate"])
        .arg(&duplicate_input)
        .arg(&deduplicated_output)
        .args(["--header-umi-delimiter", ":", "--json"])
        .output()
        .expect("run FASTQ deduplicate");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid FASTQ deduplicate JSON");
    assert_eq!(result["capability"], "fastq.deduplicate.v1");
    assert_eq!(result["result"]["output_read_count"], 2);
    assert_eq!(result["result"]["duplicate_read_count"], 1);
    fs::remove_dir_all(temp).expect("remove FASTQ transform directory");
}

#[test]
fn runs_closest_interval_and_preranked_gsea_as_json() {
    let root = workspace_root();
    let temp = temporary_directory("closest-gsea");
    let closest = temp.join("closest.tsv");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["interval", "closest"])
        .arg(root.join("tests/fixtures/interval-intersect/left.bed"))
        .arg(root.join("tests/fixtures/interval-intersect/right.bed"))
        .arg(&closest)
        .arg("--json")
        .output()
        .expect("run closest intervals");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("closest JSON");
    assert_eq!(result["capability"], "interval.closest.v1");
    assert_eq!(result["result"]["matched_query_count"], 3);
    assert!(closest.exists());

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["enrichment", "gsea"])
        .arg(root.join("tests/fixtures/functional/ranked.tsv"))
        .arg(root.join("tests/fixtures/functional/gene-sets.tsv"))
        .args([
            "--min-set-size",
            "2",
            "--max-set-size",
            "4",
            "--permutations",
            "50",
            "--seed",
            "42",
            "--json",
        ])
        .output()
        .expect("run preranked GSEA");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("GSEA JSON");
    assert_eq!(result["capability"], "enrichment.gsea.v1");
    assert_eq!(result["result"]["tested_gene_set_count"], 3);
    assert_eq!(result["result"]["seed"], 42);
    fs::remove_dir_all(temp).expect("remove closest GSEA directory");
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
fn runs_variant_compare_interval_closest_and_preranked_gsea() {
    let root = workspace_root();
    let temporary = temporary_directory("new-local-analyses");

    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["variant", "compare"])
        .arg(root.join("tests/fixtures/variant-compare/left.vcf"))
        .arg(root.join("tests/fixtures/variant-compare/right.vcf"))
        .arg("--json")
        .output()
        .expect("run variant comparison");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("variant comparison JSON");
    assert_eq!(result["capability"], "variant.compare.v1");
    assert_eq!(result["result"]["shared_count"], 3);
    assert_eq!(result["result"]["left_only_count"], 2);
    assert_eq!(result["result"]["right_only_count"], 1);
    assert_eq!(result["result"]["sample_genotypes_compared"], false);

    let query = temporary.join("query.bed");
    let target = temporary.join("target.bed");
    let closest = temporary.join("closest.tsv");
    fs::write(&query, "chr1\t10\t20\nchr2\t1\t2\n").expect("write query BED");
    fs::write(&target, "chr1\t22\t30\n").expect("write target BED");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["interval", "closest"])
        .arg(&query)
        .arg(&target)
        .arg(&closest)
        .arg("--json")
        .output()
        .expect("run closest interval analysis");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("closest interval JSON");
    assert_eq!(result["capability"], "interval.closest.v1");
    assert_eq!(result["result"]["matched_query_count"], 1);
    assert_eq!(result["result"]["unmatched_query_count"], 1);
    assert!(
        fs::read_to_string(&closest)
            .unwrap()
            .contains("\t2\tdownstream")
    );

    let ranks = temporary.join("ranks.csv");
    let gene_sets = temporary.join("gene-sets.tsv");
    fs::write(&ranks, "gene_id,score\nA,4\nB,3\nC,-2\nD,-3\n").expect("write ranked genes");
    fs::write(
        &gene_sets,
        "term_id\tgene_id\tterm_name\nTOP\tA\tTop genes\nTOP\tB\tTop genes\nBOTTOM\tC\tBottom genes\nBOTTOM\tD\tBottom genes\n",
    )
    .expect("write gene sets");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["enrichment", "gsea"])
        .arg(&ranks)
        .arg(&gene_sets)
        .args([
            "--min-set-size",
            "1",
            "--max-set-size",
            "3",
            "--permutations",
            "32",
            "--seed",
            "7",
            "--json",
        ])
        .output()
        .expect("run preranked GSEA");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).expect("GSEA JSON");
    assert_eq!(result["capability"], "enrichment.gsea.v1");
    assert_eq!(result["result"]["ranked_gene_count"], 4);
    assert_eq!(result["result"]["tested_gene_set_count"], 2);

    fs::remove_dir_all(temporary).expect("remove local analysis test directory");
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
fn runs_expression_normalization_pca_clustering_and_heatmap() {
    let root = workspace_root();
    let input = root.join("tests/fixtures/expression-matrix/analysis.tsv");
    let temp = temporary_directory("expression-analysis");
    let normalized = temp.join("normalized.tsv");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
        .args(["expression", "normalize"])
        .arg(&input)
        .arg(&normalized)
        .args(["--method", "log2-cpm", "--pseudocount", "1", "--json"])
        .output()
        .expect("run expression normalization");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid normalization result");
    assert_eq!(result["capability"], "expression.normalize.v1");
    assert_eq!(result["result"]["method"], "log2-cpm");
    assert!(normalized.exists());

    for (command, capability, expected_field) in [
        ("pca", "expression.pca.v1", "components"),
        ("cluster", "expression.cluster.v1", "samples"),
        ("heatmap", "expression.heatmap.v1", "values"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .args(["expression", command])
            .arg(&input)
            .arg("--json")
            .output()
            .unwrap_or_else(|error| panic!("run {command}: {error}"));
        assert!(
            output.status.success(),
            "{command}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid expression analysis result");
        assert_eq!(result["capability"], capability);
        assert!(result["result"].get(expected_field).is_some());
    }
    fs::remove_dir_all(temp).expect("remove expression analysis directory");
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

#[test]
fn functional_annotation_and_enrichment_commands_emit_versioned_json() {
    let workspace = workspace_root();
    let fixtures = workspace.join("tests/fixtures/functional");
    let output_root = temporary_directory("functional-analysis");
    let go_output = output_root.join("go-associations.tsv");
    let eggnog_output = output_root.join("eggnog-normalized.tsv");

    let normalization_cases = [
        (
            ["annotation", "go"],
            fixtures.join("go-source.tsv"),
            go_output.clone(),
            "annotation.go.normalize.v1",
            "association_count",
            5,
        ),
        (
            ["annotation", "eggnog"],
            fixtures.join("eggnog.tsv"),
            eggnog_output.clone(),
            "annotation.eggnog.normalize.v1",
            "query_count",
            3,
        ),
    ];
    for (command, input, output_path, capability, field, expected) in normalization_cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .args(command)
            .arg(input)
            .arg(&output_path)
            .arg("--json")
            .output()
            .expect("run functional normalization command");
        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid normalization JSON");
        assert_eq!(result["schema_version"], "1");
        assert_eq!(result["capability"], capability);
        assert_eq!(result["result"][field], expected);
        assert!(output_path.is_file());
    }

    for (mode, capability, expected) in [
        ("custom", "enrichment.overrepresentation.v1", 6),
        ("go", "enrichment.go.v1", 3),
        ("kegg", "enrichment.kegg.v1", 2),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .args(["enrichment", mode])
            .arg(fixtures.join("genes.txt"))
            .arg(fixtures.join("associations.tsv"))
            .args(["--include-genes", "--max-terms", "10", "--json"])
            .output()
            .expect("run enrichment command");
        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid enrichment JSON");
        assert_eq!(result["schema_version"], "1");
        assert_eq!(result["capability"], capability);
        assert_eq!(result["result"]["reported_term_count"], expected);
        assert_eq!(result["result"]["query_unmapped_count"], 1);
    }

    fs::remove_dir_all(output_root).expect("remove functional analysis directory");
}

#[test]
fn renders_reusable_scientific_svg_artifacts() {
    let root = workspace_root();
    let output_root = temporary_directory("scientific-svg");
    let cases = [
        (
            vec![
                "annotation".to_owned(),
                "plot".to_owned(),
                root.join("tests/fixtures/annotation/genes.gff3")
                    .to_string_lossy()
                    .into_owned(),
                output_root
                    .join("annotation.svg")
                    .to_string_lossy()
                    .into_owned(),
                "--feature-id".to_owned(),
                "g1".to_owned(),
                "--json".to_owned(),
            ],
            "annotation.structure.visualize.v1",
            output_root.join("annotation.svg"),
            "Annotation structure",
        ),
        (
            vec![
                "protein".to_owned(),
                "domain-plot".to_owned(),
                root.join("tests/fixtures/protein-domains/interproscan.tsv")
                    .to_string_lossy()
                    .into_owned(),
                output_root
                    .join("domains.svg")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "protein.domain.visualize.v1",
            output_root.join("domains.svg"),
            "Protein domain architecture",
        ),
        (
            vec![
                "enrichment".to_owned(),
                "visualize".to_owned(),
                root.join("tests/fixtures/functional/genes.txt")
                    .to_string_lossy()
                    .into_owned(),
                root.join("tests/fixtures/functional/associations.tsv")
                    .to_string_lossy()
                    .into_owned(),
                output_root
                    .join("enrichment.svg")
                    .to_string_lossy()
                    .into_owned(),
                "--kind".to_owned(),
                "go".to_owned(),
                "--style".to_owned(),
                "network".to_owned(),
                "--json".to_owned(),
            ],
            "enrichment.visualize.v1",
            output_root.join("enrichment.svg"),
            "Enrichment term-gene network",
        ),
    ];

    for (arguments, capability, output_path, expected_svg_text) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .args(arguments)
            .output()
            .unwrap_or_else(|error| panic!("run {capability}: {error}"));
        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid visualization JSON");
        assert_eq!(result["capability"], capability);
        assert_eq!(result["result"]["width"], 1_200);
        assert!(result["result"]["glyph_count"].as_u64().unwrap() > 0);
        let svg = fs::read_to_string(&output_path).expect("read generated SVG");
        assert!(svg.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?><svg"));
        assert!(svg.contains(expected_svg_text));
    }

    fs::remove_dir_all(output_root).expect("remove visualization directory");
}

#[test]
fn executes_controlled_native_tool_wrappers_without_a_shell() {
    let root = workspace_root();
    let output_root = temporary_directory("native-tools");
    let stub = compile_native_tool_stub(&root, &output_root);
    let fasta = root.join("tests/fixtures/sequences/tiny.fa");
    let profile = root.join("tests/fixtures/native-tools/profile.hmm");
    let structure = root.join("tests/fixtures/data-inspection/structure.pdb");
    let cases = [
        (
            vec![
                "alignment".to_owned(),
                "bam-cram-qc".to_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("alignment-stats.tsv")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "alignment.bam-cram.qc.v1",
            output_root.join("alignment-stats.tsv"),
            "samtools",
            1,
        ),
        (
            vec![
                "alignment".to_owned(),
                "coverage".to_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("coverage.tsv")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "alignment.coverage.v1",
            output_root.join("coverage.tsv"),
            "samtools",
            1,
        ),
        (
            vec![
                "alignment".to_owned(),
                "short-read".to_owned(),
                fasta.to_string_lossy().into_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("short-read.bam")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "alignment.short-read.v1",
            output_root.join("short-read.bam"),
            "minimap2-samtools",
            2,
        ),
        (
            vec![
                "similarity".to_owned(),
                "blast".to_owned(),
                fasta.to_string_lossy().into_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root.join("blast.tsv").to_string_lossy().into_owned(),
                "--program".to_owned(),
                "blastn".to_owned(),
                "--threads".to_owned(),
                "2".to_owned(),
                "--json".to_owned(),
            ],
            "similarity.blast.local.v1",
            output_root.join("blast.tsv"),
            "ncbi-blast",
            2,
        ),
        (
            vec![
                "similarity".to_owned(),
                "diamond".to_owned(),
                fasta.to_string_lossy().into_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("diamond.tsv")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "similarity.diamond.v1",
            output_root.join("diamond.tsv"),
            "diamond",
            2,
        ),
        (
            vec![
                "similarity".to_owned(),
                "hmmer".to_owned(),
                profile.to_string_lossy().into_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("domains.domtblout")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "similarity.hmmer.v1",
            output_root.join("domains.domtblout"),
            "hmmer",
            1,
        ),
        (
            vec![
                "msa".to_owned(),
                "muscle".to_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("alignment.fa")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "msa.muscle.v1",
            output_root.join("alignment.fa"),
            "muscle",
            1,
        ),
        (
            vec![
                "msa".to_owned(),
                "trimal".to_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("trimmed.fa")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "msa.trimal.v1",
            output_root.join("trimmed.fa"),
            "trimal",
            1,
        ),
        (
            vec![
                "phylogeny".to_owned(),
                "iqtree".to_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root.join("tree.nwk").to_string_lossy().into_owned(),
                "--json".to_owned(),
            ],
            "phylogeny.iqtree.v1",
            output_root.join("tree.nwk"),
            "iqtree",
            1,
        ),
        (
            vec![
                "motif".to_owned(),
                "meme".to_owned(),
                fasta.to_string_lossy().into_owned(),
                output_root
                    .join("motifs.meme")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "motif.meme.v1",
            output_root.join("motifs.meme"),
            "meme",
            1,
        ),
        (
            vec![
                "protein".to_owned(),
                "secondary-structure".to_owned(),
                structure.to_string_lossy().into_owned(),
                output_root
                    .join("structure.dssp")
                    .to_string_lossy()
                    .into_owned(),
                "--json".to_owned(),
            ],
            "protein.secondary-structure.v1",
            output_root.join("structure.dssp"),
            "mkdssp",
            1,
        ),
    ];

    for (arguments, capability, output_path, tool, command_count) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio"))
            .args(arguments)
            .env("LINXIRA_BIO_MAKEBLASTDB", &stub)
            .env("LINXIRA_BIO_BLASTN", &stub)
            .env("LINXIRA_BIO_DIAMOND", &stub)
            .env("LINXIRA_BIO_HMMSEARCH", &stub)
            .env("LINXIRA_BIO_MUSCLE", &stub)
            .env("LINXIRA_BIO_TRIMAL", &stub)
            .env("LINXIRA_BIO_IQTREE", &stub)
            .env("LINXIRA_BIO_MEME", &stub)
            .env("LINXIRA_BIO_MKDSSP", &stub)
            .env("LINXIRA_BIO_SAMTOOLS", &stub)
            .env("LINXIRA_BIO_MINIMAP2", &stub)
            .output()
            .unwrap_or_else(|error| panic!("run {capability}: {error}"));
        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid native-tool JSON");
        assert_eq!(result["capability"], capability);
        assert_eq!(result["result"]["tool"], tool);
        assert_eq!(result["result"]["command_count"], command_count);
        assert!(result["result"]["output_bytes"].as_u64().unwrap() > 0);
        assert!(output_path.is_file());
    }

    fs::remove_dir_all(output_root).expect("remove native-tool directory");
}

fn compile_native_tool_stub(root: &std::path::Path, output_root: &std::path::Path) -> PathBuf {
    let executable = output_root.join(format!("native-tool-stub{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new("rustc")
        .args(["--edition", "2021"])
        .arg(root.join("tests/fixtures/native-tools/stub.rs"))
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("compile native tool stub");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
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
