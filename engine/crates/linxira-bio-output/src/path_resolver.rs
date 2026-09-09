//! Cross-platform path resolution and workspace output layout (M0-T5..T8).
//!
//! Agents hand over paths with quotes, `file://` prefixes, `~`, and mixed
//! separators; every input path entering the engine flows through
//! [`resolve_input_path`]. Outputs land in classified, timestamped
//! workspace directories so repeated runs never overwrite each other.

use crate::{OutputError, OutputResult};
use std::path::{Path, PathBuf};

/// Resolves an agent-supplied input path into a usable file path.
///
/// Strips surrounding quotes and the `file://` prefix, expands `~` and the
/// literal `%USERPROFILE%` marker, rejects NUL bytes, and normalizes
/// backslashes on Windows (where `\` is a separator, never a file-name byte;
/// on POSIX it is a legal file-name byte and stays untouched).
pub fn resolve_input_path(raw: &str) -> OutputResult<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    resolve_input_path_with_home(raw, home.as_deref())
}

/// [`resolve_input_path`] with an explicit home directory, so the expansion
/// rules stay testable on every platform.
pub fn resolve_input_path_with_home(raw: &str, home: Option<&Path>) -> OutputResult<PathBuf> {
    let mut candidate = raw.trim();
    if candidate.is_empty() {
        return Err(OutputError::InvalidPath(
            "the input path is empty".to_owned(),
        ));
    }
    if candidate.contains('\0') {
        return Err(OutputError::InvalidPath(
            "the input path contains a NUL byte".to_owned(),
        ));
    }
    if candidate.len() >= 2 {
        let first = candidate.chars().next().unwrap_or('"');
        let last = candidate.chars().last().unwrap_or('"');
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            candidate = candidate[1..candidate.len() - 1].trim();
        }
    }
    if let Some(rest) = candidate.strip_prefix("file://") {
        let resolved = strip_file_authority(rest);
        return finish_candidate(&resolved, home);
    }
    finish_candidate(candidate, home)
}

fn finish_candidate(candidate: &str, home: Option<&Path>) -> OutputResult<PathBuf> {
    if candidate.starts_with('~') {
        let rest = candidate.trim_start_matches('~');
        if !rest.is_empty() && !rest.starts_with('/') && !rest.starts_with('\\') {
            return Err(OutputError::InvalidPath(format!(
                "only the current user's home is supported, got {candidate:?}"
            )));
        }
        let home = home.ok_or_else(|| {
            OutputError::InvalidPath(
                "cannot expand `~`: no home directory is configured".to_owned(),
            )
        })?;
        let rest = rest.trim_start_matches(['/', '\\']);
        return Ok(join_home(home, rest));
    }
    if let Some(rest) = candidate.strip_prefix("%USERPROFILE%") {
        let home = home.ok_or_else(|| {
            OutputError::InvalidPath(
                "cannot expand %USERPROFILE%: no home directory is configured".to_owned(),
            )
        })?;
        let rest = rest.trim_start_matches(['/', '\\']);
        return Ok(join_home(home, rest));
    }
    Ok(PathBuf::from(normalize_separators(candidate)))
}

/// Strips the authority component after `file://`. On Windows the leading
/// slash of `file:///c:/x` has to go, or the drive letter would be mangled;
/// host-form URLs (`file://server/share`) keep their double-slash prefix.
fn strip_file_authority(rest: &str) -> String {
    if let Some(without_slash) = rest.strip_prefix('/') {
        if cfg!(windows) {
            let bytes = without_slash.as_bytes();
            let is_drive_path = bytes.first().is_some_and(|byte| byte.is_ascii_alphabetic())
                && bytes.get(1) == Some(&b':');
            if is_drive_path {
                return without_slash.to_owned();
            }
        }
        return rest.to_owned();
    }
    format!("//{rest}")
}

fn join_home(home: &Path, rest: &str) -> PathBuf {
    let joined = if rest.is_empty() {
        home.to_path_buf()
    } else {
        home.join(rest)
    };
    if cfg!(windows) {
        PathBuf::from(joined.to_string_lossy().replace('\\', "/"))
    } else {
        joined
    }
}

fn normalize_separators(candidate: &str) -> String {
    if cfg!(windows) {
        candidate.replace('\\', "/")
    } else {
        candidate.to_owned()
    }
}

/// The workspace root for a run: the GUI project directory when a project
/// file is supplied, otherwise the CLI `--workspace` flag, otherwise the
/// current directory.
pub fn workspace_root(cli_flag: Option<&Path>, gui_project: Option<&Path>) -> PathBuf {
    if let Some(project) = gui_project {
        return project
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
    }
    cli_flag
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Maps a capability id onto its classified output directory:
/// `analysis/<domain>/<capability>/`.
pub fn classify_output_dir(capability: &str) -> OutputResult<PathBuf> {
    let capability = capability.trim();
    if capability.is_empty() {
        return Err(OutputError::InvalidSpec(
            "capability must not be empty".to_owned(),
        ));
    }
    let domain = capability.split('.').next().unwrap_or(capability);
    Ok(PathBuf::from("analysis").join(domain).join(capability))
}

/// Suffixes a desired output path with `_YYYYMMDD-HHMMSS`, appending `_2`,
/// `_3`, ... when the suffixed path is already taken, so repeated runs in the
/// same second never overwrite each other.
pub fn timestamp_suffix(existing: &Path) -> OutputResult<PathBuf> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| OutputError::Io(std::io::Error::other(error.to_string())))?;
    let stamp = format_timestamp_unix(now.as_secs());
    let stem = existing
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("output");
    let extension = existing
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    let parent = existing
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let mut counter: u32 = 0;
    loop {
        let suffix = if counter == 0 {
            String::new()
        } else {
            format!("_{counter}")
        };
        let candidate = parent
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(format!("{stem}_{stamp}{suffix}{extension}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
        counter = counter.checked_add(1).ok_or_else(|| {
            OutputError::InvalidSpec("timestamp suffix counter overflow".to_owned())
        })?;
    }
}

/// Renders `YYYYMMDD-HHMMSS` in UTC from a Unix timestamp.
pub fn format_timestamp_unix(unix_seconds: u64) -> String {
    let days = unix_seconds / 86_400;
    let seconds_of_day = unix_seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}",
        hour = seconds_of_day / 3_600,
        minute = (seconds_of_day % 3_600) / 60,
        second = seconds_of_day % 60,
    )
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to (y, m, d).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = (shifted - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_shifted = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_shifted + 2) / 5 + 1) as u32;
    let month = if month_shifted < 10 {
        month_shifted + 3
    } else {
        month_shifted - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::{
        classify_output_dir, format_timestamp_unix, resolve_input_path_with_home, timestamp_suffix,
        workspace_root,
    };
    use std::path::{Path, PathBuf};

    const HOME: &str = "/home/researcher";

    #[test]
    fn expands_home_and_strips_quotes_and_file_scheme() {
        assert_eq!(
            resolve_input_path_with_home("~/data/sample.fa", Some(Path::new(HOME)))
                .expect("~ path"),
            PathBuf::from("/home/researcher/data/sample.fa")
        );
        assert_eq!(
            resolve_input_path_with_home("~", Some(Path::new(HOME))).expect("bare ~"),
            PathBuf::from(HOME)
        );
        assert_eq!(
            resolve_input_path_with_home(
                "\"/data with spaces/sample file.fa\"",
                Some(Path::new(HOME))
            )
            .expect("quoted path"),
            PathBuf::from("/data with spaces/sample file.fa")
        );
        assert_eq!(
            resolve_input_path_with_home("'/data/中文目录/样本.fa'", Some(Path::new(HOME)))
                .expect("quoted unicode path"),
            PathBuf::from("/data/中文目录/样本.fa")
        );
        assert_eq!(
            resolve_input_path_with_home(
                "file:///home/researcher/reads.fastq",
                Some(Path::new(HOME))
            )
            .expect("file:// url"),
            PathBuf::from("/home/researcher/reads.fastq")
        );
        assert_eq!(
            resolve_input_path_with_home("%USERPROFILE%/counts.tsv", Some(Path::new(HOME)))
                .expect("%USERPROFILE% marker"),
            PathBuf::from("/home/researcher/counts.tsv")
        );
    }

    #[cfg(windows)]
    #[test]
    fn strips_the_file_scheme_slash_before_windows_drive_letters() {
        assert_eq!(
            resolve_input_path_with_home("file:///c:/data/x.fa", Some(Path::new(HOME)))
                .expect("windows file url"),
            PathBuf::from("c:/data/x.fa")
        );
        assert_eq!(
            resolve_input_path_with_home("\"C:\\\\data\\\\x.fa\"", Some(Path::new(HOME)))
                .expect("windows quoted path"),
            PathBuf::from("C:/data/x.fa")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn keeps_absolute_posix_file_urls_and_backslash_names_on_posix() {
        assert_eq!(
            resolve_input_path_with_home("file:///opt/data/x.vcf", Some(Path::new(HOME)))
                .expect("posix file url"),
            PathBuf::from("/opt/data/x.vcf")
        );
        // On POSIX a backslash is a legal file-name byte, not a separator.
        assert_eq!(
            resolve_input_path_with_home("data\\name.fa", Some(Path::new(HOME)))
                .expect("posix backslash name"),
            PathBuf::from("data\\name.fa")
        );
    }

    #[test]
    fn rejects_empty_nul_and_tilde_user_paths() {
        assert!(resolve_input_path_with_home("   ", None).is_err());
        let error = resolve_input_path_with_home("data\0.fa", None).expect_err("NUL byte");
        assert!(error.to_string().contains("NUL"));
        let error = resolve_input_path_with_home("~other/data", None).expect_err("~user form");
        assert!(error.to_string().contains("current user's home"));
        assert!(resolve_input_path_with_home("~/x", None).is_err());
    }

    #[test]
    fn workspace_root_prefers_the_gui_project_then_the_cli_flag() {
        assert_eq!(
            workspace_root(
                Some(Path::new("/data/ws")),
                Some(Path::new("/proj/session.linxira"))
            ),
            PathBuf::from("/proj")
        );
        assert_eq!(
            workspace_root(Some(Path::new("/data/ws")), None),
            PathBuf::from("/data/ws")
        );
        let fallback = workspace_root(None, None);
        assert!(
            !fallback.as_os_str().is_empty(),
            "falls back to the current directory"
        );
    }

    #[test]
    fn classifies_output_directories_by_capability_domain() {
        assert_eq!(
            classify_output_dir("expression.differential.v1").expect("classify"),
            PathBuf::from("analysis/expression/expression.differential.v1")
        );
        assert_eq!(
            classify_output_dir("sequence.stats.v1").expect("classify"),
            PathBuf::from("analysis/sequence/sequence.stats.v1")
        );
        assert_eq!(
            classify_output_dir("doctor").expect("classify"),
            PathBuf::from("analysis/doctor/doctor")
        );
        assert!(classify_output_dir("  ").is_err());
    }

    #[test]
    fn timestamp_collisions_get_counter_suffixes() {
        let root = std::env::temp_dir().join(format!(
            "linxira-bio-output-timestamp-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create temporary root");
        let desired = root.join("result.tsv");
        let first = timestamp_suffix(&desired).expect("first suffix");
        let name = first
            .file_name()
            .and_then(|name| name.to_str())
            .expect("utf-8 name")
            .to_owned();
        assert!(name.starts_with("result_"));
        assert!(name.ends_with(".tsv"));

        std::fs::write(&first, "occupied").expect("occupy the first suffixed path");
        let second = timestamp_suffix(&desired).expect("second suffix");
        assert_ne!(first, second, "same-second collisions must not collide");
        assert!(
            second
                .file_name()
                .and_then(|name| name.to_str())
                .expect("utf-8 name")
                .contains("_1")
        );
        std::fs::remove_dir_all(root).expect("clean up");
    }

    #[test]
    fn formats_known_unix_timestamps() {
        assert_eq!(format_timestamp_unix(0), "19700101-000000");
        assert_eq!(format_timestamp_unix(86_399), "19700101-235959");
        assert_eq!(format_timestamp_unix(86_400), "19700102-000000");
        assert_eq!(format_timestamp_unix(1_788_912_000), "20260909-000000");
    }
}
