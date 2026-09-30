//! Stub cutile-rs backend (NVIDIA). Gated behind `gpu-cutile`; becomes real
//! on the Linux cloud-rental window (RTX 4090, sm_89). The probe reports
//! unavailable until the crate is wired and the driver check lands.

use crate::backend::{BackendBuffer, BufferRole, GpuBackend, KernelId, PhaseTimings};

pub struct CutileBackend;

impl CutileBackend {
    pub fn new_boxed() -> Box<dyn GpuBackend> {
        Box::new(Self)
    }
}

impl GpuBackend for CutileBackend {
    fn name(&self) -> &'static str {
        "cutile"
    }

    fn is_available(&self) -> bool {
        // G3 execution card: probe nvidia-smi presence + cutile-rs device
        // open, then report true. Kept conservative until the Linux window.
        false
    }

    fn describe(&self) -> String {
        "cutile-rs (stub; activates on the Linux sm_89 window)".to_owned()
    }

    fn upload(
        &mut self,
        _label: &str,
        _bytes: &[u8],
        _role: BufferRole,
    ) -> Result<BackendBuffer, String> {
        Err("cutile backend not yet wired (M5-G3 Linux window)".to_owned())
    }

    fn dispatch(
        &mut self,
        _kernel: KernelId,
        _buffers: &[BackendBuffer],
        _workgroups: [u32; 3],
    ) -> Result<(), String> {
        Err("cutile backend not yet wired (M5-G3 Linux window)".to_owned())
    }

    fn readback(&mut self, _buffer: BackendBuffer, _out: &mut [u8]) -> Result<(), String> {
        Err("cutile backend not yet wired (M5-G3 Linux window)".to_owned())
    }

    fn phases(&self) -> PhaseTimings {
        PhaseTimings::default()
    }
}
