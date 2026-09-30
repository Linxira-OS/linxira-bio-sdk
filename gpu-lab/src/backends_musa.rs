//! Stub cudarc-musa backend (Moore Threads). Gated behind `gpu-musa`;
//! activates on the civilian S80 / KUAE rental window (MUSA 5.2 driver ABI).

use crate::backend::{BackendBuffer, BufferRole, GpuBackend, KernelId, PhaseTimings};

pub struct MusaBackend;

impl MusaBackend {
    pub fn new_boxed() -> Box<dyn GpuBackend> {
        Box::new(Self)
    }
}

impl GpuBackend for MusaBackend {
    fn name(&self) -> &'static str {
        "musa"
    }

    fn is_available(&self) -> bool {
        // G3 execution card: probe musa-smi + cudarc-musa device open on the
        // S80 window.
        false
    }

    fn describe(&self) -> String {
        "cudarc-musa (stub; activates on the S80/MUSA 5.2 window)".to_owned()
    }

    fn upload(
        &mut self,
        _label: &str,
        _bytes: &[u8],
        _role: BufferRole,
    ) -> Result<BackendBuffer, String> {
        Err("musa backend not yet wired (M5-G3 S80 window)".to_owned())
    }

    fn dispatch(
        &mut self,
        _kernel: KernelId,
        _buffers: &[BackendBuffer],
        _workgroups: [u32; 3],
    ) -> Result<(), String> {
        Err("musa backend not yet wired (M5-G3 S80 window)".to_owned())
    }

    fn readback(&mut self, _buffer: BackendBuffer, _out: &mut [u8]) -> Result<(), String> {
        Err("musa backend not yet wired (M5-G3 S80 window)".to_owned())
    }

    fn phases(&self) -> PhaseTimings {
        PhaseTimings::default()
    }
}
