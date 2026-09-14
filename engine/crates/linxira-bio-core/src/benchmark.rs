//! Benchmark support library (M2-T2): the consistency diff engine, wall-time
//! statistics, GNU `time -v` parsing, and environment disclosure. The
//! orchestration that spawns worker processes lives in the CLI; everything
//! here is pure and deterministic so it stays unit-testable on every
//! platform.

use linxira_bio_protocol::{BenchmarkEnvironment, BenchmarkFinding};
use serde_json::Value;

/// The numeric tolerance of the consistency verdict (methodology §7.1):
/// relative error at or below this value counts as consistent.
pub const CONSISTENCY_TOLERANCE: f64 = 1e-6;

/// A finding carries a `jq`-style field path so diffs are actionable.
pub fn diff_envelopes(reference: &Value, candidate: &Value) -> Vec<BenchmarkFinding> {
    let mut findings = Vec::new();
    diff_value("", reference, candidate, &mut findings);
    findings
}

/// Cross-backend consistency (methodology §7.1) compares what the analysis
/// *produced*, not how it ran: the envelope identity (`schema_version`,
/// `capability`, `status`) and the full `result` object are diffed; the
/// per-run `provenance` (timestamps, engine/software versions, command line),
/// `diagnostics`, `artifacts` (paths and hashes differ per output directory),
/// and `job_id` are excluded so an independent Python or R implementation is
/// judged on its numbers alone.
pub fn diff_result_envelopes(reference: &Value, candidate: &Value) -> Vec<BenchmarkFinding> {
    let mut findings = Vec::new();
    for key in ["schema_version", "capability", "status"] {
        diff_value(
            &format!(".{key}"),
            reference.get(key).unwrap_or(&Value::Null),
            candidate.get(key).unwrap_or(&Value::Null),
            &mut findings,
        );
    }
    diff_value(
        ".result",
        reference.get("result").unwrap_or(&Value::Null),
        candidate.get("result").unwrap_or(&Value::Null),
        &mut findings,
    );
    findings
}

fn diff_value(
    path: &str,
    reference: &Value,
    candidate: &Value,
    findings: &mut Vec<BenchmarkFinding>,
) {
    match (reference, candidate) {
        (Value::Object(reference_map), Value::Object(candidate_map)) => {
            for (key, reference_value) in reference_map {
                let child_path = format!("{path}.{key}");
                match candidate_map.get(key) {
                    Some(candidate_value) => {
                        diff_value(&child_path, reference_value, candidate_value, findings);
                    }
                    None => findings.push(BenchmarkFinding {
                        field: child_path,
                        detail: "missing in candidate".to_owned(),
                    }),
                }
            }
            for key in candidate_map.keys() {
                if !reference_map.contains_key(key) {
                    findings.push(BenchmarkFinding {
                        field: format!("{path}.{key}"),
                        detail: "missing in reference".to_owned(),
                    });
                }
            }
        }
        (Value::Array(reference_items), Value::Array(candidate_items)) => {
            if reference_items.len() != candidate_items.len() {
                findings.push(BenchmarkFinding {
                    field: path.to_owned(),
                    detail: format!(
                        "array length differs: {} vs {}",
                        reference_items.len(),
                        candidate_items.len()
                    ),
                });
                return;
            }
            for (index, (reference_item, candidate_item)) in
                reference_items.iter().zip(candidate_items).enumerate()
            {
                diff_value(
                    &format!("{path}[{index}]"),
                    reference_item,
                    candidate_item,
                    findings,
                );
            }
        }
        (Value::Number(reference_number), Value::Number(candidate_number)) => {
            let reference_value = reference_number.as_f64().unwrap_or_default();
            let candidate_value = candidate_number.as_f64().unwrap_or_default();
            if !numbers_consistent(reference_value, candidate_value) {
                findings.push(BenchmarkFinding {
                    field: path.to_owned(),
                    detail: format!("{reference_value} vs {candidate_value}"),
                });
            }
        }
        _ => {
            if reference != candidate {
                findings.push(BenchmarkFinding {
                    field: path.to_owned(),
                    detail: format!("{reference} vs {candidate}"),
                });
            }
        }
    }
}

/// Relative-error comparison with exact zero handling: both-zero is
/// consistent, one-zero is not; otherwise |a-b|/max(|a|,|b|) ≤ tolerance.
fn numbers_consistent(reference: f64, candidate: f64) -> bool {
    if reference == candidate {
        return true;
    }
    let scale = reference.abs().max(candidate.abs());
    if scale == 0.0 {
        return false;
    }
    (reference - candidate).abs() / scale <= CONSISTENCY_TOLERANCE
}

/// Median of an unsorted sample; the middle element for odd counts and the
/// mean of the two middle elements for even counts.
pub fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|left, right| left.partial_cmp(right).expect("finite wall times"));
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

/// Interquartile range (75th minus 25th percentile, linear interpolation).
pub fn iqr(values: &mut [f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    values.sort_by(|left, right| left.partial_cmp(right).expect("finite wall times"));
    Some(percentile(values, 0.75) - percentile(values, 0.25))
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let position = fraction * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let weight = position - lower as f64;
        sorted[lower] * (1.0 - weight) + sorted[upper] * weight
    }
}

/// Metrics parsed from `/usr/bin/time -v` verbose output (Linux high
/// precision mode).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TimeVerboseMetrics {
    pub wall_ms: f64,
    pub user_ms: f64,
    pub system_ms: f64,
    pub peak_rss_kb: Option<u64>,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
}

/// GNU time's wall-clock label contains colons itself, and the elapsed value
/// (`m:ss.mm`) does too, so labels are matched as full prefixes instead of
/// splitting on colons.
pub fn parse_time_verbose(output: &str) -> Option<TimeVerboseMetrics> {
    let mut metrics = TimeVerboseMetrics::default();
    let mut saw_wall = false;
    for line in output.lines() {
        if let Some(value) = label_value(line, "Elapsed (wall clock) time (h:mm:ss or m:ss)") {
            if let Some(seconds) = parse_elapsed(value) {
                metrics.wall_ms = seconds * 1000.0;
                saw_wall = true;
            }
        } else if let Some(value) = label_value(line, "User time (seconds)") {
            if let Ok(seconds) = value.parse::<f64>() {
                metrics.user_ms = seconds * 1000.0;
            }
        } else if let Some(value) = label_value(line, "System time (seconds)") {
            if let Ok(seconds) = value.parse::<f64>() {
                metrics.system_ms = seconds * 1000.0;
            }
        } else if let Some(value) = label_value(line, "Maximum resident set size (kbytes)") {
            metrics.peak_rss_kb = value.parse::<u64>().ok();
        } else if let Some(value) = label_value(line, "File system inputs") {
            // The kernel counts 512-byte blocks.
            metrics.disk_read_bytes = value.parse::<u64>().ok().map(|blocks| blocks * 512);
        } else if let Some(value) = label_value(line, "File system outputs") {
            metrics.disk_write_bytes = value.parse::<u64>().ok().map(|blocks| blocks * 512);
        }
    }
    saw_wall.then_some(metrics)
}

/// Matches `LABEL: value` where the label itself may contain colons.
fn label_value<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let rest = line.trim().strip_prefix(label)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim())
}

/// GNU time prints `m:ss.mm` or `h:mm:ss`; both map to seconds.
fn parse_elapsed(value: &str) -> Option<f64> {
    let mut seconds = 0.0;
    for part in value.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}

/// Runtime version probes execute hard-coded binaries with hard-coded
/// arguments only (no shell, no user-controlled program names), keeping the
/// environment disclosure side-effect free.
pub fn environment_snapshot(engine_version: &str) -> BenchmarkEnvironment {
    let kernel = read_kernel();
    let wsl_version = wsl_version_from_kernel(kernel.as_deref());
    let host = wsl_version.as_deref().and_then(read_windows_host_overview);
    BenchmarkEnvironment {
        os: std::env::consts::OS.to_owned(),
        code_revision: Some(env!("LINXIRA_CODE_REVISION").to_owned()),
        gpu: std::env::var("LINXIRA_BIO_GPU")
            .ok()
            .filter(|value| !value.is_empty()),
        memory: std::env::var("LINXIRA_BIO_MEMORY")
            .ok()
            .filter(|value| !value.is_empty()),
        storage: std::env::var("LINXIRA_BIO_STORAGE")
            .ok()
            .filter(|value| !value.is_empty()),
        kernel,
        cpu_model: read_cpu_model(),
        total_memory_mb: read_total_memory_mb(),
        distro: read_distro(),
        wsl_distro: std::env::var("WSL_DISTRO_NAME").ok(),
        wsl_version,
        host_os: host.as_ref().map(|host| host.os.clone()),
        host_model: host.as_ref().map(|host| host.model.clone()),
        host_cpu_model: host.as_ref().map(|host| host.cpu_model.clone()),
        host_logical_processors: host.as_ref().and_then(|host| host.logical_processors),
        host_total_memory_mb: host.as_ref().and_then(|host| host.total_memory_mb),
        engine_version: engine_version.to_owned(),
        python_version: probe_python_version(),
        r_version: first_line_of(
            std::process::Command::new("R")
                .arg("--version")
                .output()
                .ok(),
        ),
        in_container: std::path::Path::new("/.dockerenv").exists(),
        page_cache: "warm".to_owned(),
        precision: precision_label(),
    }
}

/// One-shot view of the Windows host behind a WSL guest, captured over the
/// interop bridge so reports disclose the machine a WSL benchmark really
/// ran on (version, model, CPU, RAM) instead of only the guest allocation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WindowsHostOverview {
    pub os: String,
    pub model: String,
    pub cpu_model: String,
    pub logical_processors: Option<u64>,
    pub total_memory_mb: Option<u64>,
}

/// WSL kernels carry a `microsoft` marker (`…-microsoft-standard-WSL2` for
/// WSL2); plain `Microsoft` markers appear on WSL1 kernels.
fn wsl_version_from_kernel(kernel: Option<&str>) -> Option<String> {
    let kernel = kernel?;
    let lowered = kernel.to_ascii_lowercase();
    if lowered.contains("wsl2") {
        Some("wsl2".to_owned())
    } else if lowered.contains("microsoft") {
        Some("wsl1".to_owned())
    } else {
        None
    }
}

fn read_distro() -> Option<String> {
    let os_release = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in os_release.lines() {
        if let Some(pretty) = line.strip_prefix("PRETTY_NAME=") {
            return Some(pretty.trim().trim_matches('"').to_owned());
        }
    }
    None
}

/// Probes the Windows host via `cmd.exe /c ver` and one PowerShell CIM call.
/// Both fail cleanly to `None` when interop is disabled or the host tools are
/// unavailable, so the snapshot stays usable inside plain containers.
fn read_windows_host_overview(wsl_version: &str) -> Option<WindowsHostOverview> {
    // WSL1 does not ship the full CIM surface over interop; version alone is
    // still disclosed from `ver`.
    let os = std::process::Command::new("cmd.exe")
        .arg("/c")
        .arg("ver")
        .output()
        .ok()
        .and_then(|output| parse_windows_ver(&String::from_utf8_lossy(&output.stdout)))?;
    if wsl_version != "wsl2" {
        return Some(WindowsHostOverview {
            os,
            ..WindowsHostOverview::default()
        });
    }
    let hardware = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "$cs = Get-CimInstance Win32_ComputerSystem; \
             $cpu = Get-CimInstance Win32_Processor | Select-Object -First 1; \
             '{0}|{1}|{2}|{3}|{4}' -f $cs.Manufacturer, $cs.Model, $cpu.Name, \
             $cs.NumberOfLogicalProcessors, [math]::Round($cs.TotalPhysicalMemory / 1MB)",
        ])
        .output()
        .ok()
        .and_then(|output| parse_windows_hardware(&String::from_utf8_lossy(&output.stdout)))
        .unwrap_or_default();
    Some(WindowsHostOverview {
        os,
        model: hardware.0,
        cpu_model: hardware.1,
        logical_processors: hardware.2,
        total_memory_mb: hardware.3,
    })
}

/// `ver` prints a banner like `Microsoft Windows [Version 10.0.x.y]`, but the
/// word inside the brackets is localized and arrives in the OEM codepage
/// (mojibake after lossy UTF-8 decoding). The ASCII build number is the only
/// stable part, so the banner is normalized to `[Version <build>]`.
fn parse_windows_ver(output: &str) -> Option<String> {
    let banner = output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("Microsoft Windows"))?;
    let build = banner.split(['[', ']']).nth(1).and_then(|inner| {
        inner.split_whitespace().find(|token| {
            token.chars().next().is_some_and(|c| c.is_ascii_digit())
                && token.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
    });
    match build {
        Some(build) => Some(format!("Microsoft Windows [Version {build}]")),
        None => Some(
            banner
                .chars()
                .filter(char::is_ascii)
                .collect::<String>()
                .trim()
                .to_owned(),
        ),
    }
}

/// Parses the `manufacturer|model|cpu|logical processors|memory MB` line
/// emitted by the PowerShell probe.
fn parse_windows_hardware(output: &str) -> Option<(String, String, Option<u64>, Option<u64>)> {
    let line = output
        .lines()
        .map(str::trim)
        .find(|line| line.contains('|'))?;
    let fields: Vec<&str> = line.split('|').map(str::trim).collect();
    if fields.len() != 5 {
        return None;
    }
    let model = format!("{} {}", fields[0], fields[1]).trim().to_owned();
    Some((
        model,
        fields[2].to_owned(),
        fields[3].parse::<u64>().ok(),
        fields[4].parse::<u64>().ok(),
    ))
}

/// `high` with an external `/usr/bin/time -v` wrapper, `degraded` with
/// in-process `Instant` only (Windows).
pub fn precision_label() -> String {
    if cfg!(target_os = "linux") {
        "high".to_owned()
    } else {
        "degraded".to_owned()
    }
}

fn probe_python_version() -> Option<String> {
    first_line_of(
        std::process::Command::new("python3")
            .arg("--version")
            .output()
            .ok(),
    )
    .or_else(|| {
        first_line_of(
            std::process::Command::new("python")
                .arg("--version")
                .output()
                .ok(),
        )
    })
    .or_else(|| {
        if cfg!(windows) {
            first_line_of(
                std::process::Command::new("py")
                    .args(["-3", "--version"])
                    .output()
                    .ok(),
            )
        } else {
            None
        }
    })
}

fn first_line_of(output: Option<std::process::Output>) -> Option<String> {
    let output = output?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let text = if stdout.trim().is_empty() {
        String::from_utf8_lossy(&output.stderr)
    } else {
        stdout
    };
    text.lines().next().map(|line| line.trim().to_owned())
}

fn read_kernel() -> Option<String> {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .ok()
        .map(|text| text.trim().to_owned())
}

fn read_cpu_model() -> Option<String> {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    for line in cpuinfo.lines() {
        if let Some(model) = line.strip_prefix("model name") {
            return Some(model.split(':').nth(1)?.trim().to_owned());
        }
    }
    None
}

fn read_total_memory_mb() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb: u64 = rest.trim().trim_end_matches(" kB").parse().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        TimeVerboseMetrics, diff_envelopes, diff_result_envelopes, environment_snapshot, iqr,
        median, parse_time_verbose, parse_windows_hardware, parse_windows_ver,
        wsl_version_from_kernel,
    };
    use serde_json::json;

    #[test]
    fn result_diff_ignores_provenance_and_run_identity_but_not_results() {
        let rust = json!({
            "schema_version": "2", "job_id": "benchmark-a", "capability": "sequence.stats.v1",
            "status": "ok", "result": {"sequence_count": 3, "gc_percent": 60.0},
            "artifacts": [],
            "provenance": {"engine_version": "1.0.1", "started_at": "2026-09-11T00:00:00Z"},
            "diagnostics": []
        });
        let python = json!({
            "schema_version": "2", "job_id": "benchmark-a", "capability": "sequence.stats.v1",
            "status": "ok", "result": {"sequence_count": 3, "gc_percent": 60.00000001},
            "artifacts": [{"artifact_id": "envelope", "path": "/tmp/x/result.json"}],
            "provenance": {"engine_version": "0.1.0", "started_at": "2026-09-11T00:00:07Z",
                           "software": [{"name": "Biopython"}]},
            "diagnostics": [{"code": "benchmark.self_reported", "severity": "info", "message": "{}"}]
        });
        assert!(diff_result_envelopes(&rust, &python).is_empty());
        assert!(
            !diff_envelopes(&rust, &python).is_empty(),
            "the full diff still sees provenance"
        );

        let drift = json!({
            "schema_version": "2", "capability": "sequence.stats.v1", "status": "ok",
            "result": {"sequence_count": 3, "gc_percent": 61.0}
        });
        let findings = diff_result_envelopes(&rust, &drift);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].field, ".result.gc_percent");

        let failed = json!({"schema_version": "2", "capability": "sequence.stats.v1",
                            "status": "error", "result": {}});
        let findings = diff_result_envelopes(&rust, &failed);
        assert!(findings.iter().any(|finding| finding.field == ".status"));
    }

    #[test]
    fn diffs_numbers_within_tolerance_and_flags_outliers() {
        let reference = json!({"sequence_count": 3, "gc_percent": 0.5000001, "name": "tiny"});
        let candidate = json!({"sequence_count": 3, "gc_percent": 0.5000002, "name": "tiny"});
        assert!(diff_envelopes(&reference, &candidate).is_empty());

        let drift = json!({"sequence_count": 4, "gc_percent": 0.5, "name": "tiny"});
        let findings = diff_envelopes(&reference, &drift);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].field, ".sequence_count");
    }

    #[test]
    fn diff_reports_missing_and_nested_fields() {
        let reference = json!({"result": {"records": [{"id": "a"}, {"id": "b"}], "total": 2}});
        let candidate = json!({"result": {"records": [{"id": "a"}], "total": 2}});
        let findings = diff_envelopes(&reference, &candidate);
        assert!(
            findings
                .iter()
                .any(|finding| finding.field == ".result.records"
                    && finding.detail.contains("length"))
        );
    }

    #[test]
    fn zero_against_nonzero_is_inconsistent_but_equal_zeros_are_not() {
        let reference = json!({"x": 0.0, "y": 0.0});
        let candidate = json!({"x": 1.0, "y": 0.0});
        let findings = diff_envelopes(&reference, &candidate);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].field, ".x");
    }

    #[test]
    fn statistics_handle_odd_even_and_small_samples() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&mut []), None);
        assert_eq!(iqr(&mut [1.0, 2.0, 3.0, 4.0, 100.0]), Some(2.0));
        assert_eq!(iqr(&mut [7.0]), None);
    }

    #[test]
    fn parses_gnu_time_verbose_output() {
        let report = "\
\tCommand being timed: \"worker req.json\"\n\
\tUser time (seconds): 0.12\n\
\tSystem time (seconds): 0.34\n\
\tPercent of CPU this job got: 99%\n\
\tElapsed (wall clock) time (h:mm:ss or m:ss): 1:02.50\n\
\tMaximum resident set size (kbytes): 51200\n\
\tFile system inputs: 1024\n\
\tFile system outputs: 8\n";
        let metrics = parse_time_verbose(report).expect("wall line present");
        assert_eq!(
            metrics,
            TimeVerboseMetrics {
                wall_ms: 62_500.0,
                user_ms: 120.0,
                system_ms: 340.0,
                peak_rss_kb: Some(51_200),
                disk_read_bytes: Some(1024 * 512),
                disk_write_bytes: Some(8 * 512),
            }
        );
        assert_eq!(parse_time_verbose("nothing useful"), None);
    }

    #[test]
    fn environment_snapshot_fills_portable_fields() {
        let environment = environment_snapshot("1.2.3");
        assert!(!environment.os.is_empty());
        assert_eq!(environment.engine_version, "1.2.3");
        assert_eq!(environment.page_cache, "warm");
        assert!(environment.precision == "high" || environment.precision == "degraded");
        // Host disclosure only applies inside WSL; elsewhere it stays absent
        // instead of guessing.
        if environment.wsl_version.is_none() {
            assert_eq!(environment.host_os, None);
            assert_eq!(environment.host_model, None);
            assert_eq!(environment.host_total_memory_mb, None);
        }
    }

    #[test]
    fn wsl_version_comes_from_the_kernel_marker() {
        assert_eq!(
            wsl_version_from_kernel(Some("6.6.87.2-microsoft-standard-WSL2")),
            Some("wsl2".to_owned())
        );
        assert_eq!(
            wsl_version_from_kernel(Some("4.4.0-Microsoft")),
            Some("wsl1".to_owned())
        );
        assert_eq!(wsl_version_from_kernel(Some("6.12.4-arch1-1")), None);
        assert_eq!(wsl_version_from_kernel(None), None);
    }

    #[test]
    fn windows_ver_parser_normalizes_localized_banners() {
        assert_eq!(
            parse_windows_ver("\r\nMicrosoft Windows [Version 10.0.26200.1]\r\n"),
            Some("Microsoft Windows [Version 10.0.26200.1]".to_owned())
        );
        // OEM-encoded localized word ("版本") decodes to mojibake; only the
        // ASCII build survives and the banner is normalized.
        assert_eq!(
            parse_windows_ver("\r\nMicrosoft Windows [\u{ffb0}\u{ffb1} 10.0.26200.9168]\r\n"),
            Some("Microsoft Windows [Version 10.0.26200.9168]".to_owned())
        );
        // A banner without a bracketed build degrades to its ASCII content.
        assert_eq!(
            parse_windows_ver("  Microsoft Windows \u{fffd}Pro\r\n"),
            Some("Microsoft Windows Pro".to_owned())
        );
        assert_eq!(parse_windows_ver("anything else"), None);
    }

    #[test]
    fn windows_hardware_parser_splits_the_cim_line() {
        let parsed = parse_windows_hardware(
            "\r\nLENOVO|82X6|AMD Ryzen 9 7945HX with Radeon Graphics|32|64461\r\n",
        );
        assert_eq!(
            parsed,
            Some((
                "LENOVO 82X6".to_owned(),
                "AMD Ryzen 9 7945HX with Radeon Graphics".to_owned(),
                Some(32),
                Some(64461),
            ))
        );
        assert_eq!(parse_windows_hardware("LENOVO|82X6|CPU"), None);
        assert_eq!(parse_windows_hardware(""), None);
    }
}
