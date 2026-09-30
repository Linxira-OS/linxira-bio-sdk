//! wgpu implementation of [`crate::backend::GpuBackend`] — the permanent
//! fallback floor of the backend chain. Buffers are plain wgpu buffers;
//! kernels are the WGSL sources already validated in `gpu_bench`.

use crate::backend::{BackendBuffer, BufferRole, GpuBackend, KernelId, PhaseTimings};
use std::borrow::Cow;
use std::time::Instant;
use wgpu::util::DeviceExt;

const PEARSON_MULTI_WGSL: &str = r#"
struct Params {
    rows: u32,
    pair_count: u32,
    pairs_per_group: u32,
    _pad0: u32,
};
@group(0) @binding(0) var<storage, read> matrix: array<f32>;
@group(0) @binding(1) var<storage, read> pairs: array<u32>;
@group(0) @binding(2) var<storage, read_write> numerators: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;
var<workgroup> partials_multi: array<f32, 256>;
@compute
@workgroup_size(256)
fn main(
    @builtin(local_invocation_index) local_index: u32,
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) group_count: vec3<u32>,
) {
    let lanes_per_pair = 64u;
    let subgroup = local_index / lanes_per_pair;
    let lane = local_index % lanes_per_pair;
    let flat_group = group.x + group.y * group_count.x;
    let pair = flat_group * params.pairs_per_group + subgroup;
    let in_range = pair < params.pair_count;
    var acc = 0.0;
    if (in_range) {
        let a = pairs[pair * 2u];
        let b = pairs[pair * 2u + 1u];
        var row = lane;
        loop {
            if (row >= params.rows) { break; }
            acc = fma(matrix[a * params.rows + row], matrix[b * params.rows + row], acc);
            row = row + lanes_per_pair;
        }
    }
    partials_multi[local_index] = acc;
    workgroupBarrier();
    var stride = 32u;
    loop {
        if (stride == 0u) { break; }
        if (lane < stride) {
            partials_multi[local_index] =
                partials_multi[local_index] + partials_multi[local_index + stride];
        }
        workgroupBarrier();
        stride = stride / 2u;
    }
    if (in_range && lane == 0u) {
        numerators[pair] = partials_multi[local_index];
    }
}
"#;

pub struct WgpuBackend {
    instance: wgpu::Instance,
    context: Option<WgpuContext>,
    phases: PhaseTimings,
}

struct WgpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
    backend: String,
    buffers: std::collections::BTreeMap<u64, wgpu::Buffer>,
    next_buffer_id: u64,
}

impl WgpuBackend {
    pub fn new_boxed() -> Box<dyn GpuBackend> {
        Box::new(Self {
            instance: wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            }),
            context: None,
            phases: PhaseTimings::default(),
        })
    }

    fn ensure_context(&mut self) -> Result<&mut WgpuContext, String> {
        if self.context.is_none() {
            let adapters =
                pollster::block_on(self.instance.enumerate_adapters(wgpu::Backends::all()));
            let adapter = adapters
                .iter()
                .find(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
                .ok_or("wgpu: no non-CPU adapter available")?;
            let info = adapter.get_info();
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    required_limits: adapter.limits(),
                    ..Default::default()
                }))
                .map_err(|error| format!("wgpu request_device failed: {error}"))?;
            self.context = Some(WgpuContext {
                device,
                queue,
                adapter_name: info.name,
                backend: format!("{:?}", info.backend),
                buffers: std::collections::BTreeMap::new(),
                next_buffer_id: 1,
            });
        }
        Ok(self.context.as_mut().expect("context just initialised"))
    }

    fn bind_layout_for_kernel(
        device: &wgpu::Device,
        kernel: KernelId,
    ) -> wgpu::BindGroupLayout {
        let entries: &[wgpu::BindGroupLayoutEntry] = match kernel {
            KernelId::PearsonV3Multi => &[
                crate::gpu_bench::buffer_entry_shared(0, wgpu::BufferBindingType::Storage { read_only: true }),
                crate::gpu_bench::buffer_entry_shared(1, wgpu::BufferBindingType::Storage { read_only: true }),
                crate::gpu_bench::buffer_entry_shared(2, wgpu::BufferBindingType::Storage { read_only: false }),
                crate::gpu_bench::buffer_entry_shared(3, wgpu::BufferBindingType::Uniform),
            ],
            KernelId::QualityHistogram => &[
                crate::gpu_bench::buffer_entry_shared(0, wgpu::BufferBindingType::Storage { read_only: true }),
                crate::gpu_bench::buffer_entry_shared(1, wgpu::BufferBindingType::Storage { read_only: false }),
                crate::gpu_bench::buffer_entry_shared(2, wgpu::BufferBindingType::Uniform),
            ],
        };
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu-lab-backend-layout"),
            entries,
        })
    }

    fn wgsl_source(kernel: KernelId) -> &'static str {
        match kernel {
            KernelId::PearsonV3Multi => PEARSON_MULTI_WGSL,
            KernelId::QualityHistogram => crate::gpu_bench::HISTOGRAM_SHADER_SHARED,
        }
    }
}

impl GpuBackend for WgpuBackend {
    fn name(&self) -> &'static str {
        "wgpu"
    }

    fn is_available(&self) -> bool {
        let adapters = pollster::block_on(self.instance.enumerate_adapters(wgpu::Backends::all()));
        adapters
            .iter()
            .any(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
    }

    fn describe(&self) -> String {
        match &self.context {
            Some(context) => format!("wgpu/{} ({})", context.adapter_name, context.backend),
            None => "wgpu (probed, device not yet initialised)".to_owned(),
        }
    }

    fn upload(
        &mut self,
        label: &str,
        bytes: &[u8],
        role: BufferRole,
    ) -> Result<BackendBuffer, String> {
        let start = Instant::now();
        let context = self.ensure_context()?;
        let mut usage = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        usage |= match role {
            BufferRole::Storage => wgpu::BufferUsages::STORAGE,
            BufferRole::Uniform => wgpu::BufferUsages::UNIFORM,
        };
        let buffer = context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytes,
            usage,
        });
        let id = context.next_buffer_id;
        context.next_buffer_id += 1;
        context.buffers.insert(id, buffer);
        self.phases.upload_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(BackendBuffer {
            id,
            bytes: bytes.len() as u64,
        })
    }

    fn dispatch(
        &mut self,
        kernel: KernelId,
        buffers: &[BackendBuffer],
        workgroups: [u32; 3],
    ) -> Result<(), String> {
        let start = Instant::now();
        let context = self.ensure_context()?;
        let resolved: Vec<&wgpu::Buffer> = buffers
            .iter()
            .map(|handle| {
                context
                    .buffers
                    .get(&handle.id)
                    .ok_or_else(|| format!("buffer {} not owned by wgpu backend", handle.id))
            })
            .collect::<Result<_, _>>()?;
        let layout = Self::bind_layout_for_kernel(&context.device, kernel);
        let entries: Vec<wgpu::BindGroupEntry> = resolved
            .iter()
            .enumerate()
            .map(|(binding, buffer)| wgpu::BindGroupEntry {
                binding: binding as u32,
                resource: buffer.as_entire_binding(),
            })
            .collect();
        let bind_group = context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu-lab-backend-bind-group"),
            layout: &layout,
            entries: &entries,
        });
        let shader = context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu-lab-backend-shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(Self::wgsl_source(kernel))),
        });
        let pipeline_layout =
            context
                .device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("gpu-lab-backend-pipeline-layout"),
                    bind_group_layouts: &[Some(&layout)],
                    immediate_size: 0,
                });
        let pipeline =
            context
                .device
                .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some("gpu-lab-backend-pipeline"),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some("main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups[0], workgroups[1], workgroups[2]);
        }
        context.queue.submit(Some(encoder.finish()));
        context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|error| format!("wgpu poll failed: {error:?}"))?;
        self.phases.compute_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(())
    }

    fn readback(&mut self, buffer: BackendBuffer, out: &mut [u8]) -> Result<(), String> {
        let start = Instant::now();
        let context = self.ensure_context()?;
        let source = context
            .buffers
            .get(&buffer.id)
            .ok_or_else(|| format!("buffer {} not owned by wgpu backend", buffer.id))?;
        if source.size() < buffer.bytes {
            return Err(format!(
                "buffer {} shrank below its recorded size",
                buffer.id
            ));
        }
        let staging = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu-lab-backend-readback"),
            size: buffer.bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(source, 0, &staging, 0, buffer.bytes);
        context.queue.submit(Some(encoder.finish()));
        context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|error| format!("wgpu poll failed: {error:?}"))?;
        let slice = staging.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        context
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|error| format!("wgpu poll failed: {error:?}"))?;
        receiver
            .recv()
            .map_err(|error| format!("map callback dropped: {error}"))?
            .map_err(|error| format!("buffer map failed: {error}"))?;
        let view = slice
            .get_mapped_range()
            .map_err(|error| format!("mapped range failed: {error}"))?;
        let expected = out.len();
        out.copy_from_slice(&view[..expected.min(view.len())]);
        drop(view);
        staging.unmap();
        self.phases.readback_ms = start.elapsed().as_secs_f64() * 1000.0;
        Ok(())
    }

    fn phases(&self) -> PhaseTimings {
        self.phases
    }
}
