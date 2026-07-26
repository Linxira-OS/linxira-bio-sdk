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
fn executes_annotation_jobs() {
    let root = workspace_root();
    let request = root.join("tests/fixtures/jobs/annotation-stats.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run annotation statistics worker request");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid annotation statistics result");
    assert_eq!(result["capability"], "annotation.gxf.stats.v1");
    assert_eq!(result["result"]["record_count"], 10);

    let output_path = root.join("target/test-results/annotation-extract-v2.fa");
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("create annotation result directory");
    }
    if output_path.exists() {
        std::fs::remove_file(&output_path).expect("remove stale annotation result");
    }
    let request = root.join("tests/fixtures/jobs/annotation-extract-v2.json");
    let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
        .arg(request)
        .output()
        .expect("run annotation extraction worker request");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid annotation extraction result");
    assert_eq!(result["schema_version"], "2");
    assert_eq!(result["capability"], "annotation.sequence.extract.v1");
    assert_eq!(result["result"]["output_sequence_count"], 2);
    assert_eq!(result["artifacts"][0]["format"], "fasta");
    assert_eq!(
        result["provenance"]["input_sha256"]
            .as_object()
            .map(|hashes| hashes.len()),
        Some(2)
    );
    std::fs::remove_file(output_path).expect("remove annotation result");
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
fn executes_v2_sequence_utility_fixtures() {
    let root = workspace_root();
    let cleanup_files = [
        root.join("target/test-results/sequence-normalize-ids-v2.fa"),
        root.join("target/test-results/sequence-merge-v2.fa"),
        root.join("target/test-results/sequence-to-table-v2.tsv"),
        root.join("target/test-results/sequence-from-table-v2.fa"),
    ];
    let cleanup_directories = [root.join("target/test-results/sequence-split-v2")];
    for path in cleanup_files {
        if path.exists() {
            std::fs::remove_file(&path).expect("remove stale sequence utility output");
        }
    }
    for path in cleanup_directories {
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale sequence split output");
        }
    }

    let cases = [
        (
            "sequence-normalize-ids-v2.json",
            "sequence.id.normalize.v1",
            "output_records",
            3,
        ),
        (
            "sequence-merge-v2.json",
            "sequence.merge.v1",
            "output_records",
            3,
        ),
        (
            "sequence-split-v2.json",
            "sequence.split.v1",
            "output_files",
            2,
        ),
        (
            "sequence-to-table-v2.json",
            "sequence.to-table.v1",
            "output_rows",
            3,
        ),
        (
            "sequence-from-table-v2.json",
            "sequence.from-table.v1",
            "output_records",
            2,
        ),
    ];

    for (fixture, capability, field, expected) in cases {
        let request = root.join("tests/fixtures/jobs").join(fixture);
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(request)
            .output()
            .unwrap_or_else(|error| panic!("run {capability}: {error}"));

        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid utility result");
        assert_eq!(result["schema_version"], "2", "{capability}");
        assert_eq!(result["status"], "ok", "{capability}");
        assert_eq!(result["capability"], capability, "{capability}");
        assert_eq!(result["result"][field], expected, "{capability}");
        assert_eq!(
            result["provenance"]["input_sha256"]
                .as_object()
                .map(|hashes| hashes.len()),
            Some(1),
            "{capability}"
        );
        assert_eq!(
            result["artifacts"].as_array().map(Vec::len),
            Some(1),
            "{capability}"
        );
    }

    for path in [
        root.join("target/test-results/sequence-normalize-ids-v2.fa"),
        root.join("target/test-results/sequence-merge-v2.fa"),
        root.join("target/test-results/sequence-to-table-v2.tsv"),
        root.join("target/test-results/sequence-from-table-v2.fa"),
    ] {
        std::fs::remove_file(path).expect("remove sequence utility output");
    }
    std::fs::remove_dir_all(root.join("target/test-results/sequence-split-v2"))
        .expect("remove split output directory");
}

#[test]
fn executes_v2_kmer_epcr_and_variant_transform_fixtures() {
    let root = workspace_root();
    let cases = [
        (
            "sequence-kmer-count-v2.json",
            "sequence.kmer.count.v1",
            "sequence-kmer-count-v2.tsv",
            "table",
        ),
        (
            "primer-epcr-v2.json",
            "primer.epcr.v1",
            "primer-epcr-v2.tsv",
            "table",
        ),
        (
            "variant-filter-v2.json",
            "variant.filter.v1",
            "variant-filter-v2.vcf",
            "domain-file",
        ),
        (
            "variant-normalize-v2.json",
            "variant.normalize.v1",
            "variant-normalize-v2.vcf",
            "domain-file",
        ),
    ];
    std::fs::create_dir_all(root.join("target/test-results"))
        .expect("create analysis result directory");
    for (_, _, output_name, _) in cases {
        let output_path = root.join("target/test-results").join(output_name);
        if output_path.exists() {
            std::fs::remove_file(output_path).expect("remove stale analysis output");
        }
    }

    for (fixture, capability, output_name, artifact_kind) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(root.join("tests/fixtures/jobs").join(fixture))
            .output()
            .unwrap_or_else(|error| panic!("run {capability}: {error}"));
        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid v2 result");
        assert_eq!(result["schema_version"], "2", "{capability}");
        assert_eq!(result["status"], "ok", "{capability}");
        assert_eq!(result["capability"], capability, "{capability}");
        assert_eq!(
            result["artifacts"][0]["kind"], artifact_kind,
            "{capability}"
        );
        assert!(root.join("target/test-results").join(output_name).exists());
    }

    for (_, _, output_name, _) in cases {
        std::fs::remove_file(root.join("target/test-results").join(output_name))
            .expect("remove analysis output");
    }
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
fn manipulates_tables_through_the_worker() {
    let root = workspace_root();
    let result_dir = root.join("target/test-results");
    std::fs::create_dir_all(&result_dir).expect("create table result directory");
    let outputs = [
        result_dir.join("table-manipulate.tsv"),
        result_dir.join("table-manipulate-v2.tsv"),
    ];
    for path in &outputs {
        if path.exists() {
            std::fs::remove_file(path).expect("remove stale table output");
        }
    }

    let cases = [
        (
            "table-manipulate.json",
            "gene_id\tsample_b\ngene_1\t0\ngene_2\t5\ngene_3\tNA\n",
        ),
        (
            "table-manipulate-v2.json",
            "gene_id\tsample_a\tsample_b\ngene_1\t10\t0\ngene_2\t20\t5\n",
        ),
    ];

    for (index, (fixture, expected_table)) in cases.iter().enumerate() {
        let request = root.join("tests/fixtures/jobs").join(fixture);
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(request)
            .output()
            .unwrap_or_else(|error| panic!("run {fixture}: {error}"));
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid table manipulation result");
        assert_eq!(result["status"], "ok", "{fixture}");
        assert_eq!(result["capability"], "table.manipulate.v1", "{fixture}");
        assert_eq!(result["result"]["input_rows"], 4, "{fixture}");
        assert_eq!(
            std::fs::read_to_string(&outputs[index]).expect("table output"),
            *expected_table,
            "{fixture}"
        );
        if fixture.ends_with("-v2.json") {
            assert_eq!(result["schema_version"], "2", "{fixture}");
            assert_eq!(result["artifacts"][0]["role"], "table", "{fixture}");
            assert_eq!(result["artifacts"][0]["kind"], "table", "{fixture}");
            assert_eq!(result["artifacts"][0]["format"], "tsv", "{fixture}");
            assert_eq!(
                result["artifacts"][0]["media_type"], "text/tab-separated-values",
                "{fixture}"
            );
            assert_eq!(
                result["artifacts"][0]["sha256"].as_str().map(str::len),
                Some(64),
                "{fixture}"
            );
        }
    }

    for path in outputs {
        std::fs::remove_file(path).expect("remove table output");
    }
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
fn executes_fastq_read_processing_jobs() {
    let root = workspace_root();
    let result_dir = root.join("target/test-results");
    std::fs::create_dir_all(&result_dir).expect("create FASTQ result directory");
    let output_paths = [
        result_dir.join("fastq-trim.fastq"),
        result_dir.join("fastq-adapter-trim.fastq"),
        result_dir.join("fastq-trim-v2.fastq"),
        result_dir.join("fastq-adapter-trim-v2.fastq"),
    ];
    for path in &output_paths {
        if path.exists() {
            std::fs::remove_file(path).expect("remove stale FASTQ output");
        }
    }

    let cases = [
        (
            "fastq-trim.json",
            "fastq.trim.v1",
            "quality_trimmed_bases",
            6,
            "@trim\nACGT\n+\nIIII\n@adapter\nTTTTAGATCGGA\n+\nIIIIIIIIIIII\n",
        ),
        (
            "fastq-adapter-trim.json",
            "fastq.adapter.v1",
            "adapter_trimmed_bases",
            8,
            "@trim\nACGTAC\n+\nIIII!!\n@adapter\nTTTT\n+\nIIII\n@drop\nACGT\n+\n!!!!\n",
        ),
        (
            "fastq-trim-v2.json",
            "fastq.trim.v1",
            "quality_trimmed_bases",
            6,
            "@trim\nACGT\n+\nIIII\n@adapter\nTTTTAGATCGGA\n+\nIIIIIIIIIIII\n",
        ),
        (
            "fastq-adapter-trim-v2.json",
            "fastq.adapter.v1",
            "adapter_trimmed_bases",
            8,
            "@trim\nACGTAC\n+\nIIII!!\n@adapter\nTTTT\n+\nIIII\n@drop\nACGT\n+\n!!!!\n",
        ),
    ];

    for (index, (fixture, capability, field, expected, expected_fastq)) in cases.iter().enumerate()
    {
        let request = root.join("tests/fixtures/jobs").join(fixture);
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(request)
            .output()
            .unwrap_or_else(|error| panic!("run {capability}: {error}"));

        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid FASTQ transform result");
        assert_eq!(result["status"], "ok", "{capability}");
        assert_eq!(result["capability"], *capability, "{capability}");
        assert_eq!(result["result"][*field], *expected, "{capability}");
        assert_eq!(
            std::fs::read_to_string(&output_paths[index]).expect("FASTQ output"),
            *expected_fastq,
            "{capability}"
        );
        if fixture.ends_with("-v2.json") {
            assert_eq!(result["schema_version"], "2", "{capability}");
            assert_eq!(result["artifacts"][0]["role"], "fastq", "{capability}");
            assert_eq!(
                result["artifacts"][0]["kind"], "domain-file",
                "{capability}"
            );
            assert_eq!(result["artifacts"][0]["format"], "fastq", "{capability}");
            assert_eq!(
                result["artifacts"][0]["media_type"], "text/x-fastq",
                "{capability}"
            );
            assert_eq!(
                result["artifacts"][0]["sha256"].as_str().map(str::len),
                Some(64),
                "{capability}"
            );
        }
    }

    for path in output_paths {
        std::fs::remove_file(path).expect("remove FASTQ output");
    }
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
fn executes_interval_set_operation_jobs() {
    let root = workspace_root();
    let result_dir = root.join("target/test-results");
    std::fs::create_dir_all(&result_dir).expect("create interval result directory");
    let output_paths = [
        result_dir.join("interval-merge.bed"),
        result_dir.join("interval-subtract.bed"),
        result_dir.join("interval-merge-v2.bed"),
        result_dir.join("interval-subtract-v2.bed"),
    ];
    for path in &output_paths {
        if path.exists() {
            std::fs::remove_file(path).expect("remove stale interval output");
        }
    }

    let cases = [
        (
            "interval-merge.json",
            "interval.merge.v1",
            "output_interval_count",
            2,
            "chr1\t0\t20\nchr2\t5\t12\n",
        ),
        (
            "interval-subtract.json",
            "interval.subtract.v1",
            "output_interval_count",
            3,
            "chr1\t0\t5\nchr1\t15\t20\nchr2\t7\t12\n",
        ),
        (
            "interval-merge-v2.json",
            "interval.merge.v1",
            "output_interval_count",
            2,
            "chr1\t0\t20\nchr2\t5\t12\n",
        ),
        (
            "interval-subtract-v2.json",
            "interval.subtract.v1",
            "output_interval_count",
            3,
            "chr1\t0\t5\nchr1\t15\t20\nchr2\t7\t12\n",
        ),
    ];

    for (index, (fixture, capability, field, expected, expected_bed)) in cases.iter().enumerate() {
        let request = root.join("tests/fixtures/jobs").join(fixture);
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(request)
            .output()
            .unwrap_or_else(|error| panic!("run {capability}: {error}"));

        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid interval set operation result");
        assert_eq!(result["status"], "ok", "{capability}");
        assert_eq!(result["capability"], *capability, "{capability}");
        assert_eq!(result["result"][*field], *expected, "{capability}");
        assert_eq!(
            std::fs::read_to_string(&output_paths[index]).expect("interval BED output"),
            *expected_bed,
            "{capability}"
        );
        if fixture.ends_with("-v2.json") {
            assert_eq!(result["schema_version"], "2", "{capability}");
            assert_eq!(result["artifacts"][0]["role"], "bed", "{capability}");
            assert_eq!(
                result["artifacts"][0]["kind"], "domain-file",
                "{capability}"
            );
            assert_eq!(result["artifacts"][0]["format"], "bed", "{capability}");
            assert_eq!(
                result["artifacts"][0]["media_type"], "text/x-bed",
                "{capability}"
            );
            assert_eq!(
                result["artifacts"][0]["sha256"].as_str().map(str::len),
                Some(64),
                "{capability}"
            );
        }
    }

    for path in output_paths {
        std::fs::remove_file(path).expect("remove interval output");
    }
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

#[test]
fn executes_set_and_protein_analysis_jobs() {
    let root = workspace_root();
    let cases = [
        ("set-venn.json", "set.venn.v1", "set_count", 3),
        ("set-upset.json", "set.upset.v1", "union_size", 6),
        (
            "protein-properties.json",
            "protein.properties.v1",
            "sequence_count",
            2,
        ),
        ("set-venn-v2.json", "set.venn.v1", "set_count", 3),
        ("set-upset-v2.json", "set.upset.v1", "union_size", 6),
        (
            "protein-properties-v2.json",
            "protein.properties.v1",
            "sequence_count",
            2,
        ),
    ];
    for (fixture, capability, field, expected) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(root.join("tests/fixtures/jobs").join(fixture))
            .output()
            .unwrap_or_else(|error| panic!("run {fixture}: {error}"));
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid analysis result");
        assert_eq!(result["status"], "ok", "{fixture}");
        assert_eq!(result["capability"], capability, "{fixture}");
        assert_eq!(result["result"][field], expected, "{fixture}");
        if fixture.ends_with("-v2.json") {
            assert_eq!(
                result["provenance"]["input_sha256"]
                    .as_object()
                    .map(serde_json::Map::len),
                Some(1),
                "{fixture}"
            );
        }
    }
}

#[test]
fn executes_coordinate_structure_analysis_jobs() {
    let root = workspace_root();
    let cases = [
        (
            "structure-mmcif-summary.json",
            "structure.mmcif.summary.v1",
            "atom_count",
            5,
            1,
        ),
        (
            "structure-sequence.json",
            "structure.sequence.extract.v1",
            "total_residues",
            4,
            1,
        ),
        (
            "structure-contact-map.json",
            "structure.contact-map.v1",
            "contact_count",
            6,
            1,
        ),
        (
            "structure-geometry.json",
            "structure.geometry.v1",
            "value",
            45,
            1,
        ),
        (
            "structure-superpose.json",
            "structure.superpose.v1",
            "matched_atom_count",
            4,
            2,
        ),
        (
            "structure-mmcif-summary-v2.json",
            "structure.mmcif.summary.v1",
            "atom_count",
            5,
            1,
        ),
        (
            "structure-sequence-v2.json",
            "structure.sequence.extract.v1",
            "total_residues",
            4,
            1,
        ),
        (
            "structure-contact-map-v2.json",
            "structure.contact-map.v1",
            "contact_count",
            6,
            1,
        ),
        (
            "structure-geometry-v2.json",
            "structure.geometry.v1",
            "value",
            45,
            1,
        ),
        (
            "structure-superpose-v2.json",
            "structure.superpose.v1",
            "matched_atom_count",
            4,
            2,
        ),
    ];
    for (fixture, capability, field, expected_integer_part, expected_hashes) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(root.join("tests/fixtures/jobs").join(fixture))
            .output()
            .unwrap_or_else(|error| panic!("run {fixture}: {error}"));
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid structure result");
        assert_eq!(result["status"], "ok", "{fixture}");
        assert_eq!(result["capability"], capability, "{fixture}");
        let value = result["result"][field]
            .as_f64()
            .unwrap_or_else(|| panic!("{fixture}: numeric result field {field}"));
        assert_eq!(value.round() as i64, expected_integer_part, "{fixture}");
        if fixture.ends_with("-v2.json") {
            assert_eq!(
                result["provenance"]["input_sha256"]
                    .as_object()
                    .map(serde_json::Map::len),
                Some(expected_hashes),
                "{fixture}"
            );
        }
    }
}

#[test]
fn executes_v2_expression_analysis_fixtures() {
    let root = workspace_root();
    let output_path = root.join("target/test-results/expression-normalize-v2.tsv");
    std::fs::create_dir_all(output_path.parent().expect("output parent"))
        .expect("create expression result directory");
    if output_path.exists() {
        std::fs::remove_file(&output_path).expect("remove stale normalized matrix");
    }
    let cases = [
        ("expression-normalize-v2.json", "expression.normalize.v1"),
        ("expression-pca-v2.json", "expression.pca.v1"),
        ("expression-cluster-v2.json", "expression.cluster.v1"),
        ("expression-heatmap-v2.json", "expression.heatmap.v1"),
    ];
    for (fixture, capability) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(root.join("tests/fixtures/jobs").join(fixture))
            .output()
            .unwrap_or_else(|error| panic!("run {capability}: {error}"));
        assert!(
            output.status.success(),
            "{capability}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid expression result");
        assert_eq!(result["schema_version"], "2", "{capability}");
        assert_eq!(result["status"], "ok", "{capability}");
        assert_eq!(result["capability"], capability, "{capability}");
        assert_eq!(
            result["provenance"]["input_sha256"]
                .as_object()
                .map(|hashes| hashes.len()),
            Some(1),
            "{capability}"
        );
    }
    assert!(output_path.exists());
    std::fs::remove_file(output_path).expect("remove normalized expression output");
}

#[test]
fn executes_similarity_domain_density_and_tree_v2_fixtures() {
    let root = workspace_root();
    let tree_output = root.join("target/test-results/phylogeny-tree-v2.nwk");
    std::fs::create_dir_all(tree_output.parent().expect("tree output parent"))
        .expect("create tree output directory");
    if tree_output.exists() {
        std::fs::remove_file(&tree_output).expect("remove stale tree output");
    }

    let cases = [
        (
            "blast-parse-v2.json",
            "similarity.blast.parse.v1",
            "record_count",
            3,
            1,
        ),
        (
            "reciprocal-best-hits-v2.json",
            "similarity.reciprocal.v1",
            "reciprocal_pair_count",
            2,
            2,
        ),
        (
            "protein-domain-parse-v2.json",
            "protein.domain.parse.v1",
            "hit_count",
            3,
            1,
        ),
        (
            "gene-density-v2.json",
            "genome.gene-density.v1",
            "selected_feature_count",
            2,
            1,
        ),
        (
            "phylogeny-tree-v2.json",
            "phylogeny.tree.transform.v1",
            "leaf_count",
            4,
            1,
        ),
    ];

    for (fixture, capability, field, expected, expected_hashes) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(root.join("tests/fixtures/jobs").join(fixture))
            .output()
            .unwrap_or_else(|error| panic!("run {fixture}: {error}"));
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid analysis result");
        assert_eq!(result["schema_version"], "2", "{fixture}");
        assert_eq!(result["status"], "ok", "{fixture}");
        assert_eq!(result["capability"], capability, "{fixture}");
        assert_eq!(result["result"][field], expected, "{fixture}");
        assert_eq!(
            result["provenance"]["input_sha256"]
                .as_object()
                .map(serde_json::Map::len),
            Some(expected_hashes),
            "{fixture}"
        );
    }

    assert!(tree_output.exists());
    let tree_text = std::fs::read_to_string(&tree_output).expect("read transformed tree");
    assert!(tree_text.contains("sampleA"));
    assert_eq!(tree_text.matches(';').count(), 1);
    std::fs::remove_file(tree_output).expect("remove transformed tree output");
}

#[test]
fn executes_functional_annotation_and_enrichment_fixtures() {
    let root = workspace_root();
    let result_dir = root.join("target/test-results");
    std::fs::create_dir_all(&result_dir).expect("create functional result directory");
    let generated_outputs = [
        result_dir.join("go-associations.tsv"),
        result_dir.join("go-associations-v2.tsv"),
        result_dir.join("eggnog-normalized.tsv"),
        result_dir.join("eggnog-normalized-v2.tsv"),
    ];
    for path in &generated_outputs {
        if path.exists() {
            std::fs::remove_file(path).expect("remove stale functional output");
        }
    }

    let cases = [
        (
            "annotation-go.json",
            "annotation.go.normalize.v1",
            "association_count",
            5,
            1,
        ),
        (
            "annotation-go-v2.json",
            "annotation.go.normalize.v1",
            "association_count",
            5,
            1,
        ),
        (
            "annotation-eggnog.json",
            "annotation.eggnog.normalize.v1",
            "query_count",
            3,
            1,
        ),
        (
            "annotation-eggnog-v2.json",
            "annotation.eggnog.normalize.v1",
            "query_count",
            3,
            1,
        ),
        (
            "enrichment-custom.json",
            "enrichment.overrepresentation.v1",
            "reported_term_count",
            6,
            2,
        ),
        (
            "enrichment-custom-v2.json",
            "enrichment.overrepresentation.v1",
            "reported_term_count",
            6,
            2,
        ),
        (
            "enrichment-go.json",
            "enrichment.go.v1",
            "reported_term_count",
            3,
            2,
        ),
        (
            "enrichment-go-v2.json",
            "enrichment.go.v1",
            "reported_term_count",
            3,
            2,
        ),
        (
            "enrichment-kegg.json",
            "enrichment.kegg.v1",
            "reported_term_count",
            2,
            2,
        ),
        (
            "enrichment-kegg-v2.json",
            "enrichment.kegg.v1",
            "reported_term_count",
            2,
            2,
        ),
    ];

    for (fixture, capability, field, expected, expected_hashes) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(root.join("tests/fixtures/jobs").join(fixture))
            .output()
            .unwrap_or_else(|error| panic!("run {fixture}: {error}"));
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid functional result");
        assert_eq!(result["status"], "ok", "{fixture}");
        assert_eq!(result["capability"], capability, "{fixture}");
        assert_eq!(result["result"][field], expected, "{fixture}");
        if fixture.ends_with("-v2.json") {
            assert_eq!(result["schema_version"], "2", "{fixture}");
            assert_eq!(
                result["provenance"]["input_sha256"]
                    .as_object()
                    .map(serde_json::Map::len),
                Some(expected_hashes),
                "{fixture}"
            );
        }
    }

    for path in &generated_outputs {
        assert!(
            path.exists(),
            "expected generated output {}",
            path.display()
        );
        std::fs::remove_file(path).expect("remove functional output");
    }
}

#[test]
fn executes_scientific_visualization_jobs_and_tracks_svg_artifacts() {
    let root = workspace_root();
    let cases = [
        (
            "annotation-structure-visualize.json",
            "annotation.structure.visualize.v1",
            "annotation-structure.svg",
            "Annotation structure",
            false,
            1,
        ),
        (
            "annotation-structure-visualize-v2.json",
            "annotation.structure.visualize.v1",
            "annotation-structure-v2.svg",
            "Annotation structure",
            true,
            1,
        ),
        (
            "enrichment-visualize.json",
            "enrichment.visualize.v1",
            "enrichment-network.svg",
            "Enrichment term-gene network",
            false,
            2,
        ),
        (
            "enrichment-visualize-v2.json",
            "enrichment.visualize.v1",
            "enrichment-network-v2.svg",
            "Enrichment term-gene network",
            true,
            2,
        ),
        (
            "protein-domain-visualize.json",
            "protein.domain.visualize.v1",
            "protein-domains.svg",
            "Protein domain architecture",
            false,
            1,
        ),
        (
            "protein-domain-visualize-v2.json",
            "protein.domain.visualize.v1",
            "protein-domains-v2.svg",
            "Protein domain architecture",
            true,
            1,
        ),
    ];

    std::fs::create_dir_all(root.join("target/test-results"))
        .expect("create visualization result directory");
    for (_, _, output_name, _, _, _) in &cases {
        let output_path = root.join("target/test-results").join(output_name);
        if output_path.exists() {
            std::fs::remove_file(output_path).expect("remove stale visualization output");
        }
    }

    for (fixture, capability, output_name, expected_svg_text, is_v2, input_hashes) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_linxira-bio-worker"))
            .arg(root.join("tests/fixtures/jobs").join(fixture))
            .output()
            .unwrap_or_else(|error| panic!("run {fixture}: {error}"));
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("valid visualization result");
        assert_eq!(result["status"], "ok", "{fixture}");
        assert_eq!(result["capability"], capability, "{fixture}");
        assert_eq!(result["result"]["width"], 1_200, "{fixture}");
        assert!(
            result["result"]["glyph_count"].as_u64().unwrap() > 0,
            "{fixture}"
        );

        let output_path = root.join("target/test-results").join(output_name);
        let svg = std::fs::read_to_string(&output_path).expect("read visualization SVG");
        assert!(svg.contains(expected_svg_text), "{fixture}");
        if is_v2 {
            assert_eq!(result["schema_version"], "2", "{fixture}");
            assert_eq!(result["artifacts"][0]["kind"], "plot", "{fixture}");
            assert_eq!(result["artifacts"][0]["format"], "svg", "{fixture}");
            assert_eq!(
                result["artifacts"][0]["media_type"], "image/svg+xml",
                "{fixture}"
            );
            assert_eq!(
                result["artifacts"][0]["sha256"].as_str().map(str::len),
                Some(64),
                "{fixture}"
            );
            assert_eq!(
                result["provenance"]["input_sha256"]
                    .as_object()
                    .map(serde_json::Map::len),
                Some(input_hashes),
                "{fixture}"
            );
        }
        std::fs::remove_file(output_path).expect("remove visualization output");
    }
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
