use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const COUNTS_SHA256: &str = "365ec34ff99a91eaca206014e7cdf7ceba6ee8f96f120ca60b2bb05327996006";
const SAMPLES_SHA256: &str = "daf55d07e3e55c677b2977d47eea333f284daec4db9856a0138cc33e2f9a9244";
const DIFFERENTIAL: &str = "feature_id,base_mean,log2_fold_change,standard_error,statistic,p_value,adjusted_p_value\nGene1,100,1,0.2,5,0.001,0.01\n";
const NORMALIZED: &str = "feature_id,control_1,control_2,treated_1,treated_2\nGene1,50,52,100,104\n";
const DIFFERENTIAL_SHA256: &str =
    "f90e5764acad9024562835aa548dd1977ac325b76ed8eaecd1cb0008b5ae560b";
const NORMALIZED_SHA256: &str =
    "f63b397d6544a76359ff7514371ac7b0713eaefc35c6508ff1e1e72298d13625";

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 5 || !path_ends_with(&arguments[0], "run_deseq2.R") {
        eprintln!("unexpected Rscript arguments");
        return ExitCode::from(64);
    }
    let Some(request_path) = value_after(&arguments, "--request").map(PathBuf::from) else {
        return ExitCode::from(64);
    };
    let Some(result_path) = value_after(&arguments, "--result").map(PathBuf::from) else {
        return ExitCode::from(64);
    };
    if let Some(trace) = env::var_os("LINXIRA_BIO_RSCRIPT_STUB_TRACE") {
        if let Err(error) = fs::write(trace, request_path.to_string_lossy().as_bytes()) {
            eprintln!("cannot write trace: {error}");
            return ExitCode::from(65);
        }
    }
    let request = match fs::read_to_string(&request_path) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("cannot read request: {error}");
            return ExitCode::from(66);
        }
    };
    let Some(job_id) = json_string_field(&request, "job_id") else {
        return ExitCode::from(67);
    };
    let Some(capability) = json_string_field(&request, "capability") else {
        return ExitCode::from(67);
    };
    if !request.contains("\"path\":\"") || request.contains("../expression-matrix") {
        eprintln!("worker did not rewrite relative input paths");
        return ExitCode::from(68);
    }
    let Some(output_directory) = result_path.parent() else {
        return ExitCode::from(69);
    };
    if let Err(error) = fs::create_dir(output_directory) {
        eprintln!("cannot create output directory: {error}");
        return ExitCode::from(70);
    }

    let mode = env::var("LINXIRA_BIO_RSCRIPT_STUB_MODE").unwrap_or_default();
    let lock_sha256 = env::var("LINXIRA_BIO_RSCRIPT_STUB_LOCK_SHA256")
        .unwrap_or_else(|_| "0".repeat(64));
    if mode == "missing-package" {
        let payload = error_envelope(&job_id, &capability);
        return write_result(&result_path, &payload, ExitCode::from(2));
    }

    let differential_path = output_directory.join("differential-expression.csv");
    let normalized_path = output_directory.join("normalized-counts.csv");
    if fs::write(&differential_path, DIFFERENTIAL).is_err()
        || fs::write(&normalized_path, NORMALIZED).is_err()
    {
        return ExitCode::from(71);
    }
    let result_capability = if mode == "wrong-capability" {
        "wrong.capability.v1"
    } else {
        &capability
    };
    let payload = success_envelope(
        &job_id,
        result_capability,
        &differential_path,
        &normalized_path,
        &lock_sha256,
    );
    write_result(&result_path, &payload, ExitCode::SUCCESS)
}

fn success_envelope(
    job_id: &str,
    capability: &str,
    differential: &Path,
    normalized: &Path,
    lock_sha256: &str,
) -> String {
    let diagnostics = if capability == "medical.bulk-rnaseq.v1" {
        r#"[{"code":"research_use_only","severity":"warning","message":"Research use only; no diagnosis or clinical interpretation is provided."}]"#
    } else {
        "[]"
    };
    format!(
        concat!(
            "{{\"schema_version\":\"2\",\"job_id\":{},\"capability\":{},\"status\":\"ok\",",
            "\"result\":{{\"input_features\":4,\"analyzed_features\":4,\"filtered_features\":0,",
            "\"samples\":4,\"significant_features\":1,\"alpha\":0.05,\"min_total_count\":10,",
            "\"intended_use\":\"research-use-only\",\"clinical_use\":false,",
            "\"contrast\":{{\"level\":\"treated\",\"reference\":\"control\"}},",
            "\"effective_parameters\":{{\"feature_id_column\":\"gene_id\",",
            "\"sample_id_column\":\"sample_id\",\"condition_column\":\"condition\",",
            "\"reference_level\":\"control\",\"contrast_level\":\"treated\",",
            "\"alpha\":0.05,\"min_total_count\":10}}}},",
            "\"artifacts\":[",
            "{{\"artifact_id\":\"differential-expression\",\"role\":\"differential-expression\",",
            "\"kind\":\"table\",\"path\":{},\"format\":\"csv\",\"media_type\":\"text/csv\",",
            "\"size_bytes\":117,\"sha256\":\"{}\"}},",
            "{{\"artifact_id\":\"normalized-counts\",\"role\":\"normalized-counts\",",
            "\"kind\":\"table\",\"path\":{},\"format\":\"csv\",\"media_type\":\"text/csv\",",
            "\"size_bytes\":71,\"sha256\":\"{}\"}}],",
            "\"provenance\":{{\"engine_version\":\"stub-1\",\"execution_mode\":\"local-cpu\",",
            "\"software\":[],\"input_sha256\":{{\"counts\":\"{}\",",
            "\"sample_metadata\":\"{}\"}},\"dependency_lock_sha256\":\"{}\"}},",
            "\"diagnostics\":{}}}"
        ),
        json_quote(job_id),
        json_quote(capability),
        json_quote(&differential.to_string_lossy()),
        DIFFERENTIAL_SHA256,
        json_quote(&normalized.to_string_lossy()),
        NORMALIZED_SHA256,
        COUNTS_SHA256,
        SAMPLES_SHA256,
        lock_sha256,
        diagnostics
    )
}

fn error_envelope(job_id: &str, capability: &str) -> String {
    let diagnostics = if capability == "medical.bulk-rnaseq.v1" {
        r#"[{"code":"research_use_only","severity":"warning","message":"Research use only; no diagnosis or clinical interpretation is provided."},{"code":"workflow_failed","severity":"error","message":"dependency DESeq2 is not installed in the project library"}]"#
    } else {
        r#"[{"code":"workflow_failed","severity":"error","message":"dependency DESeq2 is not installed in the project library"}]"#
    };
    format!(
        concat!(
            "{{\"schema_version\":\"2\",\"job_id\":{},\"capability\":{},",
            "\"status\":\"error\",\"result\":{{}},\"artifacts\":[],",
            "\"provenance\":{{\"engine_version\":\"stub-1\",",
            "\"execution_mode\":\"local-cpu\",\"software\":[],\"input_sha256\":{{}}}},",
            "\"diagnostics\":{}}}"
        ),
        json_quote(job_id),
        json_quote(capability),
        diagnostics
    )
}

fn write_result(path: &Path, payload: &str, exit: ExitCode) -> ExitCode {
    match fs::write(path, payload) {
        Ok(()) => {
            println!("{payload}");
            exit
        }
        Err(error) => {
            eprintln!("cannot write result: {error}");
            ExitCode::from(72)
        }
    }
}

fn value_after<'a>(arguments: &'a [OsString], flag: &str) -> Option<&'a OsString> {
    arguments
        .iter()
        .position(|value| value == flag)
        .and_then(|index| arguments.get(index + 1))
}

fn path_ends_with(value: &OsString, expected: &str) -> bool {
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(expected))
}

fn json_string_field(document: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\":\"");
    let tail = document.get(document.find(&marker)? + marker.len()..)?;
    let mut escaped = false;
    let mut value = String::new();
    for character in tail.chars() {
        if escaped {
            value.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(value);
        } else {
            value.push(character);
        }
    }
    None
}

fn json_quote(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}
