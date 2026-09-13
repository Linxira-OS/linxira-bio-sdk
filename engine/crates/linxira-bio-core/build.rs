//! Embeds the source revision into the benchmark environment disclosure so
//! every measured number can be traced to the exact code that produced it.
//! Falls back to "unknown" when the crate is built outside a git tree
//! (vendored source, release archives).

use std::process::Command;

fn main() {
    let revision = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| value.len() >= 7)
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=LINXIRA_CODE_REVISION={revision}");
}
