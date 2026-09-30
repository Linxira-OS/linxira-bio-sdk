//! D3 (M5-G3 / §3.1 批次 D 第 3 项): sycl-rs Windows integration attempt.
//!
//! The attempt is a feature-gated compile probe (`gpu-sycl-attempt`), not part
//! of any default build: the outcome — success or a minimal reproducer of the
//! failure — is the upstream contribution (oneapi-rs officially tests Linux
//! only). Run with:
//!
//! ```text
//! cargo build -p gpu-lab --features gpu-sycl-attempt
//! ```
//!
//! The result of the 2026-10-01 attempt is recorded in
//! docs/engine-evals/gpu-sycl-attempt-2026-10-01.md.

pub fn attempt_summary() -> &'static str {
    "sycl-rs Windows attempt: see docs/engine-evals/gpu-sycl-attempt-2026-10-01.md"
}
