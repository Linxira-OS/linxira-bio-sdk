//! M5-G3 / §3.1 批次 D 第 2 项：vendor-agnostic GPU host abstraction.
//!
//! One trait, one kernel protocol: buffer layout, dispatch geometry, phased
//! timing, and the consistency contract are defined here; each vendor backend
//! (wgpu now; cutile-rs / cudarc-musa / sycl-rs behind feature flags later)
//! implements upload/dispatch/readback over its own driver FFI. The fallback
//! chain always ends at wgpu so a binary built without any vendor SDK still
//! runs.
//!
//! Vendor backends never compile into CI: each one is gated behind a cargo
//! feature and probes the native SDK at runtime (`is_available`), matching
//! the windows-gnu CI constraint in docs/proposals/m5-g3-vendor-track-plan.md §3.

/// Kernels the abstraction knows how to place. WGSL is the canonical source;
/// vendor backends register their own translation per id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelId {
    /// Pearson v3 "multi" semantics: four 64-lane subgroups per workgroup,
    /// one column pair each, tree reduction per subgroup.
    PearsonV3Multi,
    /// 256-bin quality histogram over u32-packed lanes; integer-exact.
    QualityHistogram,
}

impl KernelId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PearsonV3Multi => "pearson-v3-multi",
            Self::QualityHistogram => "quality-histogram",
        }
    }
}

/// Host/device memory relationship of the backend's device. This decides
/// whether the upload/readback phases of the ledger contract are real copies
/// or no-op mappings, and whether kernels compete with the host for the same
/// physical bandwidth (bandwidth-bound analysis must state the model).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryModel {
    /// Discrete VRAM: upload/readback are real PCIe-style transfers; device
    /// bandwidth is independent of host memory bandwidth.
    Discrete,
    /// Unified memory (Apple Silicon, AMD AI Max 395-class iGPU, NVIDIA
    /// DGX/RTX Spark): zero-copy mapping is the intended fast path — upload
    /// degrades to an address mapping, and host/device share one bandwidth
    /// pool. Linux is the primary unified-memory work environment.
    Unified,
}

impl MemoryModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discrete => "discrete",
            Self::Unified => "unified",
        }
    }
}

/// Intended binding role of an uploaded buffer; backends map it to their
/// native usage flags (wgpu: STORAGE vs UNIFORM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferRole {
    Storage,
    Uniform,
}

/// Opaque device buffer handle owned by one backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendBuffer {
    pub(crate) id: u64,
    pub(crate) bytes: u64,
}

/// Phased wall timings in milliseconds (ledger contract: upload / compute /
/// readback; compute is the median of >=3 runs).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PhaseTimings {
    pub upload_ms: f64,
    pub compute_ms: f64,
    pub readback_ms: f64,
}

/// The host-side contract every backend must satisfy. Implementations are
/// expected to be single-threaded owners (driver contexts are not Sync).
pub trait GpuBackend {
    fn name(&self) -> &'static str;

    /// Runtime probe: does the driver/SDK exist on this machine right now?
    /// Must be cheap and side-effect free; never panics on absence.
    fn is_available(&self) -> bool;

    /// Device-visible description for ledger provenance.
    fn describe(&self) -> String;

    /// Memory model of the probed device; backends may refine this after the
    /// device is opened (the trait allows a cheap constant for stubs).
    fn memory_model(&self) -> MemoryModel {
        MemoryModel::Discrete
    }

    /// Host -> device copy. Returns the backend-owned handle.
    fn upload(
        &mut self,
        label: &str,
        bytes: &[u8],
        role: BufferRole,
    ) -> Result<BackendBuffer, String>;

    /// Launch `kernel` over the declared workgroup grid. Buffers are passed
    /// in the fixed per-kernel slot order (see KernelId).
    fn dispatch(
        &mut self,
        kernel: KernelId,
        buffers: &[BackendBuffer],
        workgroups: [u32; 3],
    ) -> Result<(), String>;

    /// Device -> host copy of the whole buffer.
    fn readback(&mut self, buffer: BackendBuffer, out: &mut [u8]) -> Result<(), String>;

    /// Timings of the most recently completed dispatch.
    fn phases(&self) -> PhaseTimings;
}

/// Probe order from the proposal: vendor stacks first (each behind its
/// feature flag and runtime SDK check), wgpu as the permanent floor.
pub fn backend_chain() -> Vec<Box<dyn GpuBackend>> {
    let mut chain: Vec<Box<dyn GpuBackend>> = Vec::new();
    #[cfg(feature = "gpu-cutile")]
    chain.push(crate::backends_cutile::CutileBackend::new_boxed());
    #[cfg(feature = "gpu-musa")]
    chain.push(crate::backends_musa::MusaBackend::new_boxed());
    #[cfg(feature = "gpu-sycl")]
    chain.push(crate::backends_sycl::SyclBackend::new_boxed());
    chain.push(crate::backends_wgpu::WgpuBackend::new_boxed());
    chain
}

/// First backend whose runtime probe succeeds; falls through to wgpu.
pub fn select_backend() -> Result<Box<dyn GpuBackend>, String> {
    for backend in backend_chain() {
        if backend.is_available() {
            return Ok(backend);
        }
    }
    Err("no GPU backend available (wgpu probe failed)".to_owned())
}

/// Ledger-friendly summary of what a fallback chain would pick.
pub fn probe_chain() -> Vec<(String, bool)> {
    backend_chain()
        .into_iter()
        .map(|backend| (format!("{}: {}", backend.name(), backend.describe()), backend.is_available()))
        .collect()
}
