// Emulates a container runtime for tests: parses `run --rm -v HOST:CONTAINER[:ro]
// [-e K=V]... IMAGE COMMAND ARGS...`, rewrites command arguments whose paths
// live under a mounted container root back to their host locations, and
// executes COMMAND locally. This validates the worker's container argument
// construction and path remapping without requiring Docker.

use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.first().and_then(|value| value.to_str()) != Some("run") {
        eprintln!("container runtime stub expects `run`");
        return ExitCode::from(64);
    }
    let mut mounts: Vec<(String, String)> = Vec::new();
    let mut environment: Vec<String> = Vec::new();
    let mut index = 1;
    let mut command: Option<OsString> = None;
    let mut command_args: Vec<OsString> = Vec::new();
    while index < arguments.len() {
        match arguments[index].to_str() {
            Some("--rm") => index += 1,
            Some("-v") => {
                let spec = arguments
                    .get(index + 1)
                    .map(|value| value.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // HOST:CONTAINER[:mode] — host paths may contain drive-letter
                // colons on Windows, so split from the right.
                let (host_and_container, _mode) =
                    spec.rsplit_once(':').unwrap_or((spec.as_str(), ""));
                if let Some((host, container)) = host_and_container.rsplit_once(':') {
                    if !host.is_empty() && !container.is_empty() {
                        mounts.push((host.to_owned(), container.to_owned()));
                    }
                }
                index += 2;
            }
            Some("-e") => {
                if let Some(entry) = arguments.get(index + 1) {
                    environment.push(entry.to_string_lossy().into_owned());
                }
                index += 2;
            }
            _ => {
                command = arguments.get(index + 1).cloned();
                command_args = arguments[index + 2..].to_vec();
                break;
            }
        }
    }
    let Some(command) = command else {
        eprintln!("container runtime stub: missing command after image");
        return ExitCode::from(65);
    };
    // Longest container root wins when rewriting a path.
    mounts.sort_by(|left, right| right.1.len().cmp(&left.1.len()));
    let remap = |value: &OsString| -> OsString {
        let text = value.to_string_lossy();
        for (host, container) in &mounts {
            if let Some(relative) = text.strip_prefix(container.as_str()) {
                let host_path = Path::new(host).join(relative.trim_start_matches('/'));
                return host_path.into_os_string();
            }
        }
        value.clone()
    };
    let mut process = Command::new(&command);
    process.args(command_args.iter().map(&remap));
    for entry in &environment {
        if let Some((key, value)) = entry.split_once('=') {
            process.env(key, value);
        }
    }
    match process.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8),
        Err(error) => {
            eprintln!("container runtime stub: {error}");
            ExitCode::from(66)
        }
    }
}
