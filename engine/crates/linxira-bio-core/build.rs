//! Embeds the source revision into the benchmark environment disclosure so
//! every measured number can be traced to the exact code that produced it.
//! Falls back to "unknown" when the crate is built outside a git tree
//! (vendored source, release archives).
//!
//! Three values are embedded, because a bare commit hash does not answer the
//! question a reader of a published number actually has:
//!   * `LINXIRA_CODE_REVISION` — full commit hash;
//!   * `LINXIRA_CODE_BRANCH`   — the branch it was built from (`HEAD` when detached);
//!   * `LINXIRA_CODE_DESCRIBE` — `git describe --tags --always --dirty`, which names
//!     the nearest release tag when one exists and degrades to the short commit hash
//!     when the project has not published a release yet. A `-dirty` suffix means the
//!     tree had uncommitted changes, so the measurement is not reproducible from the
//!     commit alone and the label says so.

use std::process::Command;

fn git(args: &[&str], minimum_length: usize) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| value.len() >= minimum_length)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn main() {
    // Rebuild the label when the checked-out commit changes, so a stale revision can
    // never be baked into a report. Both files are best-effort: a missing path only
    // means the rerun trigger is absent.
    println!("cargo:rerun-if-changed=../../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../../.git/refs/heads/main");

    println!(
        "cargo:rustc-env=LINXIRA_CODE_REVISION={}",
        git(&["rev-parse", "HEAD"], 7)
    );
    println!(
        "cargo:rustc-env=LINXIRA_CODE_BRANCH={}",
        git(&["rev-parse", "--abbrev-ref", "HEAD"], 1)
    );
    println!(
        "cargo:rustc-env=LINXIRA_CODE_DESCRIBE={}",
        git(&["describe", "--tags", "--always", "--dirty"], 7)
    );
}
