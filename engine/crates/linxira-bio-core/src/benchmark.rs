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
    BenchmarkEnvironment {
        os: std::env::consts::OS.to_owned(),
        kernel: read_kernel(),
        cpu_model: read_cpu_model(),
        total_memory_mb: read_total_memory_mb(),
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
        TimeVerboseMetrics, diff_envelopes, environment_snapshot, iqr, median, parse_time_verbose,
    };
    use serde_json::json;

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
    }
}
