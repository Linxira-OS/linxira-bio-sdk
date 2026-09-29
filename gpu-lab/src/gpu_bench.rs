//! M5-G2: wgpu/WGSL implementations of the baseline kernels, compared against
//! the CPU f64 audit tier under the declared tolerance contract.
//!
//! - quality-histogram: u8 qualities packed 4-per-u32, per-workgroup local
//!   256-bin table, atomically merged into a global table. Integer-exact.
//! - pearson-column-pairs: f32 column-pair dot products (WGSL has no f64);
//!   the f32-vs-f64 delta is the precision-contract measurement.
//! - kmer-count: NOT ported — hash-table structure does not map to WGSL;
//!   deferred to vendor stacks (G3) which have dynamic memory. Recorded here.

use crate::bench::{pearson_input, pearson_pairs, pearson_pairs_scalar, quality_values};
use serde::Serialize;
use std::borrow::Cow;
use std::time::Instant;
use wgpu::util::DeviceExt;

const HISTOGRAM_SHADER: &str = r#"
struct Params {
    word_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<storage, read> words: array<u32>;
@group(0) @binding(1) var<storage, read_write> histogram: array<atomic<u32>>;
@group(0) @binding(2) var<uniform> params: Params;

var<workgroup> local_histogram: array<atomic<u32>, 256>;

@compute
@workgroup_size(256)
fn main(
    @builtin(local_invocation_index) local_index: u32,
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) group_count: vec3<u32>,
) {
    if (local_index == 0u) {
        for (var i = 0u; i < 256u; i++) {
            atomicStore(&local_histogram[i], 0u);
        }
    }
    workgroupBarrier();

    // Grid-stride over packed words (4 qualities per u32, little-endian lanes).
    // Grid-stride over packed words (4 qualities per u32): each invocation
    // owns words with index (group.x * 256 + local_index) stepping by
    // (num_workgroups * 256), so every word is read exactly once.
    var word_index = group.x * 256u + local_index;
    let stride = group_count.x * 256u;
    loop {
        if (word_index >= params.word_count) {
            break;
        }
        let packed = words[word_index];
        atomicAdd(&local_histogram[(packed) & 0xFFu], 1u);
        atomicAdd(&local_histogram[(packed >> 8u) & 0xFFu], 1u);
        atomicAdd(&local_histogram[(packed >> 16u) & 0xFFu], 1u);
        atomicAdd(&local_histogram[(packed >> 24u) & 0xFFu], 1u);
        word_index = word_index + stride;
    }
    workgroupBarrier();

    if (local_index == 0u) {
        for (var i = 0u; i < 256u; i++) {
            let count = atomicLoad(&local_histogram[i]);
            if (count > 0u) {
                atomicAdd(&histogram[i], count);
            }
        }
    }
}
"#;

const PEARSON_SHADER: &str = r#"
struct Params {
    rows: u32,
    pair_count: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<storage, read> matrix: array<f32>; // column-major
@group(0) @binding(1) var<storage, read> pairs: array<u32>;  // pairs[2*i], pairs[2*i+1]
@group(0) @binding(2) var<storage, read_write> numerators: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair = gid.x;
    if (pair >= params.pair_count) {
        return;
    }
    let a = pairs[pair * 2u];
    let b = pairs[pair * 2u + 1u];
    var acc = 0.0;
    for (var row = 0u; row < params.rows; row++) {
        let va = matrix[a * params.rows + row];
        let vb = matrix[b * params.rows + row];
        acc = fma(va, vb, acc);
    }
    numerators[pair] = acc;
}
"#;

#[derive(Serialize)]
struct PhaseTiming {
    upload_ms: f64,
    compute_ms: f64,
    readback_ms: f64,
    total_ms: f64,
}

#[derive(Serialize)]
struct GpuKernelReport {
    kernel: String,
    precision: String,
    gpu: PhaseTiming,
    cpu_f64_audit_ms: f64,
    cpu_f64_checksum: String,
    gpu_checksum: String,
    delta: Option<f64>,
    tolerance: Option<f64>,
    pass: bool,
    note: Option<String>,
}

#[derive(Serialize)]
pub struct GpuBenchReport {
    adapter: String,
    backend: String,
    kernels: Vec<GpuKernelReport>,
    deferred: Vec<String>,
}

struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

pub fn run_gpu_benchmarks() -> Result<GpuBenchReport, String> {
    let descriptor = wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    };
    let instance = wgpu::Instance::new(descriptor);
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
        .ok_or("no non-CPU adapter available")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .map_err(|error| format!("request_device failed: {error}"))?;
    let context = GpuContext { device, queue };

    let seed = 0x5EED_5EED_5EED_5EEDu64;
    let mut kernels = Vec::new();
    kernels.push(run_histogram_gpu(&context, seed));
    kernels.push(run_pearson_gpu(&context, seed));

    Ok(GpuBenchReport {
        adapter: info.name,
        backend: format!("{:?}", info.backend),
        kernels,
        deferred: vec![
            "kmer-count: hash-table structure does not map to WGSL; deferred to vendor stacks (M5-G3)"
                .to_owned(),
        ],
    })
}

fn run_histogram_gpu(context: &GpuContext, seed: u64) -> GpuKernelReport {
    let qualities = quality_values(seed);
    let total = qualities.len();
    assert_eq!(total % 4, 0, "quality count must pack evenly into u32 words");
    let words: Vec<u32> = qualities
        .chunks(4)
        .map(|chunk| {
            u32::from(chunk[0])
                | (u32::from(chunk[1]) << 8)
                | (u32::from(chunk[2]) << 16)
                | (u32::from(chunk[3]) << 24)
        })
        .collect();

    // CPU f64/integer reference over the same values.
    let cpu_start = Instant::now();
    let mut reference = vec![0u64; 256];
    for quality in &qualities {
        reference[usize::from(*quality)] += 1;
    }
    let cpu_ms = cpu_start.elapsed().as_secs_f64() * 1000.0;

    let device = &context.device;
    let _queue = &context.queue;

    let word_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-histogram-words"),
        size: (words.len() * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let histogram_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-histogram-out"),
        size: 256 * 4,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let params_bytes: [u8; 16] = (words.len() as u32).to_le_bytes().into_iter()
        .chain([0u8; 12].into_iter())
        .collect::<Vec<u8>>()
        .try_into()
        .expect("16 param bytes");
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("gpu-lab-histogram-params"),
        contents: &params_bytes,
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("gpu-lab-histogram-shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(HISTOGRAM_SHADER)),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gpu-lab-histogram-layout"),
        entries: &[
            buffer_entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
            buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
            buffer_entry(2, wgpu::BufferBindingType::Uniform),
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu-lab-histogram-bind-group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: word_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: histogram_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("gpu-lab-histogram-pipeline-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("gpu-lab-histogram-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let workgroups = 512u32;
    let timing = timed_phases(
        context,
        |queue| {
            queue.write_buffer(&word_buffer, 0, bytemuck::cast_slice(&words));
            queue.write_buffer(&histogram_buffer, 0, &[0u8; 1024]);
        },
        |encoder| {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        },
        &histogram_buffer,
        256 * 4,
    );

    let gpu_histogram: Vec<u32> = timing.output;
    let mut exact = gpu_histogram.len() == 256;
    if exact {
        for (bin, count) in gpu_histogram.iter().enumerate() {
            if u64::from(*count) != reference[bin] {
                exact = false;
                break;
            }
        }
    }
    GpuKernelReport {
        kernel: "quality-histogram".to_owned(),
        precision: "u32 (integer-exact)".to_owned(),
        gpu: PhaseTiming {
            upload_ms: timing.upload_ms,
            compute_ms: timing.compute_ms,
            readback_ms: timing.readback_ms,
            total_ms: timing.total_ms,
        },
        cpu_f64_audit_ms: cpu_ms,
        cpu_f64_checksum: histogram_checksum(&reference),
        gpu_checksum: histogram_checksum_u32(&gpu_histogram),
        delta: None,
        tolerance: Some(0.0),
        pass: exact,
        note: Some(format!(
            "bases={total} packed_words={} workgroups={workgroups}",
            words.len()
        )),
    }
}

fn run_pearson_gpu(context: &GpuContext, seed: u64) -> GpuKernelReport {
    let input = pearson_input(seed);
    let pairs = pearson_pairs(input.cols);
    let pair_count = pairs.len();

    // CPU f64 audit tier over the same fixed-seed input.
    let cpu_start = Instant::now();
    let cpu_sum_r = pearson_pairs_scalar(&input.columns, &input.squares);
    let cpu_ms = cpu_start.elapsed().as_secs_f64() * 1000.0;

    // f32 upload view (column-major, f64 -> f32 conversion is the measured
    // quantization step of the precision contract).
    let mut matrix_f32 = Vec::with_capacity(input.cols * input.rows);
    for column in &input.columns {
        matrix_f32.extend(column.iter().map(|value| *value as f32));
    }
    let pairs_flat: Vec<u32> = pairs
        .iter()
        .flat_map(|(a, b)| [*a, *b])
        .collect();
    let _squares_f32: Vec<f32> = input.squares.iter().map(|value| *value as f32).collect();

    let device = &context.device;
    let _queue = &context.queue;

    let matrix_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-pearson-matrix"),
        size: (matrix_f32.len() * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let pairs_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-pearson-pairs"),
        size: (pairs_flat.len() * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let numerators_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-pearson-out"),
        size: (pair_count * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let mut params = Vec::with_capacity(16);
    params.extend_from_slice(&(input.rows as u32).to_le_bytes());
    params.extend_from_slice(&(pair_count as u32).to_le_bytes());
    params.extend_from_slice(&[0u8; 8]);
    let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("gpu-lab-pearson-params"),
        contents: &params,
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("gpu-lab-pearson-shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(PEARSON_SHADER)),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gpu-lab-pearson-layout"),
        entries: &[
            buffer_entry(0, wgpu::BufferBindingType::Storage { read_only: true }),
            buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: true }),
            buffer_entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
            buffer_entry(3, wgpu::BufferBindingType::Uniform),
        ],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu-lab-pearson-bind-group"),
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: matrix_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: pairs_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: numerators_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params_buffer.as_entire_binding(),
            },
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("gpu-lab-pearson-pipeline-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("gpu-lab-pearson-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let timing = timed_phases(
        context,
        |queue| {
            queue.write_buffer(&matrix_buffer, 0, bytemuck::cast_slice(&matrix_f32));
            queue.write_buffer(&pairs_buffer, 0, bytemuck::cast_slice(&pairs_flat));
        },
        |encoder| {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(pair_count.div_ceil(64) as u32, 1, 1);
        },
        &numerators_buffer,
        (pair_count * 4) as u64,
    );

    let numerators_f32: Vec<f32> = timing.output;
    // Host-side completion in the same nested-loop order as the CPU reference.
    let mut gpu_sum_r = 0.0f64;
    let mut index = 0usize;
    for a in 0..input.cols {
        for b in (a + 1)..input.cols {
            let numerator = f64::from(numerators_f32[index]);
            let denominator = (input.squares[a] * input.squares[b]).sqrt();
            if denominator > 0.0 {
                gpu_sum_r += numerator / denominator;
            }
            index += 1;
        }
    }
    let tolerance = 1e-5;
    let delta = (gpu_sum_r - cpu_sum_r).abs();
    GpuKernelReport {
        kernel: "pearson-column-pairs".to_owned(),
        precision: "f32 (recommended) vs f64 (audit)".to_owned(),
        gpu: PhaseTiming {
            upload_ms: timing.upload_ms,
            compute_ms: timing.compute_ms,
            readback_ms: timing.readback_ms,
            total_ms: timing.total_ms,
        },
        cpu_f64_audit_ms: cpu_ms,
        cpu_f64_checksum: format!("sum-r={cpu_sum_r:.9}"),
        gpu_checksum: format!("sum-r={gpu_sum_r:.9}"),
        delta: Some(delta),
        tolerance: Some(tolerance),
        pass: delta <= tolerance,
        note: Some(format!(
            "rows={} cols={} pairs={pair_count}; f64->f32 input quantization + fma accumulation",
            input.rows, input.cols
        )),
    }
}

fn buffer_entry(binding: u32, ty: wgpu::BufferBindingType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

struct PhaseOutput<T> {
    upload_ms: f64,
    compute_ms: f64,
    readback_ms: f64,
    total_ms: f64,
    output: Vec<T>,
}

fn timed_phases<T: bytemuck::Pod>(
    context: &GpuContext,
    upload: impl FnOnce(&wgpu::Queue),
    encode: impl FnOnce(&mut wgpu::CommandEncoder),
    output_buffer: &wgpu::Buffer,
    output_bytes: u64,
) -> PhaseOutput<T> {
    let start = Instant::now();
    upload(&context.queue);
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    encode(&mut encoder);
    let readback = context.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-readback"),
        size: output_bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(output_buffer, 0, &readback, 0, output_bytes);
    let submit_start = Instant::now();
    context.queue.submit(Some(encoder.finish()));
    context
        .device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .expect("device poll");
    let compute_ms = submit_start.elapsed().as_secs_f64() * 1000.0;

    let readback_start = Instant::now();
    let slice = readback.slice(..);
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
        .expect("device poll for map");
    receiver.recv().expect("map callback").expect("buffer map");
    let view = slice.get_mapped_range().expect("mapped range");
    let output: Vec<T> = bytemuck::cast_slice(&view).to_vec();
    drop(view);
    readback.unmap();
    let readback_ms = readback_start.elapsed().as_secs_f64() * 1000.0;
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    PhaseOutput {
        upload_ms: total_ms - compute_ms - readback_ms,
        compute_ms,
        readback_ms,
        total_ms,
        output,
    }
}

fn histogram_checksum(histogram: &[u64]) -> String {
    histogram
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(bin, count)| format!("{bin}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn histogram_checksum_u32(histogram: &[u32]) -> String {
    histogram
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(bin, count)| format!("{bin}:{count}"))
        .collect::<Vec<_>>()
        .join(",")
}
