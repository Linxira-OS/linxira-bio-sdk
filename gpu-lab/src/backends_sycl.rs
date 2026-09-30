//! Stub sycl-rs backend (Intel oneapi-rs). Gated behind `gpu-sycl`; the
//! Windows integration attempt is recorded in the engine-evals ledger —
//! upstream only tests Linux, so a Windows failure report is itself the
//! contribution (M5-G3, proposal §2.1).

use crate::backend::{BackendBuffer, BufferRole, GpuBackend, KernelId, PhaseTimings};

pub struct SyclBackend;

impl SyclBackend {
    pub fn new_boxed() -> Box<dyn GpuBackend> {
        Box::new(Self)
    }
}

impl GpuBackend for SyclBackend {
    fn name(&self) -> &'static str {
        "sycl"
    }

    fn is_available(&self) -> bool {
        // G3 execution card: probe a Level Zero device via sycl-rs queue
        // construction on the local Arc 130T.
        false
    }

    fn describe(&self) -> String {
        "sycl-rs (stub; Windows attempt recorded in the ledger, Linux is upstream-tested)"
            .to_owned()
    }

    fn upload(
        &mut self,
        _label: &str,
        _bytes: &[u8],
        _role: BufferRole,
    ) -> Result<BackendBuffer, String> {
        Err("sycl backend not yet wired (M5-G3 local attempt)".to_owned())
    }

    fn dispatch(
        &mut self,
        _kernel: KernelId,
        _buffers: &[BackendBuffer],
        _workgroups: [u32; 3],
    ) -> Result<(), String> {
        Err("sycl backend not yet wired (M5-G3 local attempt)".to_owned())
    }

    fn readback(&mut self, _buffer: BackendBuffer, _out: &mut [u8]) -> Result<(), String> {
        Err("sycl backend not yet wired (M5-G3 local attempt)".to_owned())
    }

    fn phases(&self) -> PhaseTimings {
        PhaseTimings::default()
    }
}
