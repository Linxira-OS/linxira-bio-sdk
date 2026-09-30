//! gpu-lab: M5-G experiment carrier.
//! - `probe`: hardware audit — enumerate wgpu adapters + compute smoke test.
//! - `bench`: M5-G1 deterministic CPU baseline kernels (fixed-seed synthetic data).

mod backend;
mod backends_wgpu;
#[cfg(feature = "gpu-cutile")]
mod backends_cutile;
#[cfg(feature = "gpu-musa")]
mod backends_musa;
#[cfg(feature = "gpu-sycl")]
mod backends_sycl;
#[cfg(feature = "gpu-sycl-attempt")]
mod sycl_attempt;
mod bench;
mod gpu_bench;

use serde::Serialize;
use std::borrow::Cow;

const SMOKE_SHADER: &str = r#"
@group(0) @binding(0)
var<storage, read_write> data: array<f32>;

@compute
@workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= arrayLength(&data)) {
        return;
    }
    data[id.x] = data[id.x] * 2.0 + 1.0;
}
"#;

#[derive(Serialize)]
struct AdapterReport {
    index: usize,
    backend: String,
    name: String,
    vendor: u32,
    device: u32,
    device_type: String,
    max_buffer_size: u64,
    max_compute_invocations_per_workgroup: u32,
    max_compute_workgroup_size_x: u32,
    max_storage_buffers_per_shader_stage: u32,
}

#[derive(Serialize)]
struct SmokeReport {
    adapter_index: usize,
    adapter_name: String,
    element_count: usize,
    correct_elements: usize,
    passed: bool,
    error: Option<String>,
}

#[derive(Serialize)]
struct Report {
    host: String,
    wgpu_generation: String,
    adapters: Vec<AdapterReport>,
    compute_smoke: SmokeReport,
}

fn main() {
    let first = std::env::args().nth(1).unwrap_or_default();
    match first.as_str() {
        "bench" => {
            let report = bench::run_benchmarks();
            println!(
                "{}",
                serde_json::to_string_pretty(&report).expect("serialize bench report")
            );
        }
        "probe" | "" => run_probe(),
        "backend" => {
            for (description, available) in backend::probe_chain() {
                println!("{:>4}  {}", if available { "[x]" } else { "[ ]" }, description);
            }
            match backend::select_backend() {
                Ok(selected) => println!("selected backend: {}", selected.name()),
                Err(error) => {
                    eprintln!("error: {error}");
                    std::process::exit(1);
                }
            }
        }
        "backend-smoke" => {
            // One real kernel through the abstraction: 16 u32 words, every
            // packed byte lane = 1, so the exact histogram must be {1: 64}.
            let mut backend = backend::select_backend().unwrap_or_else(|error| {
                eprintln!("error: {error}");
                std::process::exit(1);
            });
            println!("backend: {}", backend.describe());
            let words: Vec<u8> = [0x01_01_01_01u32; 16]
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect();
            let mut params = Vec::new();
            params.extend_from_slice(&16u32.to_le_bytes());
            params.extend_from_slice(&[0u8; 12]);
            let words_buf = backend.upload("smoke-words", &words, backend::BufferRole::Storage).unwrap_or_else(|error| {
                eprintln!("error: {error}");
                std::process::exit(1);
            });
            let params_buf = backend.upload("smoke-params", &params, backend::BufferRole::Uniform).unwrap_or_else(|error| {
                eprintln!("error: {error}");
                std::process::exit(1);
            });
            // slot order for the histogram kernel: words, histogram, params
            let zeroed = vec![0u8; 256 * 4];
            let hist_buf = backend.upload("smoke-histogram", &zeroed, backend::BufferRole::Storage).unwrap_or_else(|error| {
                eprintln!("error: {error}");
                std::process::exit(1);
            });
            backend
                .dispatch(
                    backend::KernelId::QualityHistogram,
                    &[words_buf, hist_buf, params_buf],
                    [1, 1, 1],
                )
                .unwrap_or_else(|error| {
                    eprintln!("error: {error}");
                    std::process::exit(1);
                });
            let mut out = vec![0u8; 256 * 4];
            backend.readback(hist_buf, &mut out).unwrap_or_else(|error| {
                eprintln!("error: {error}");
                std::process::exit(1);
            });
            let histogram: Vec<u32> = out
                .chunks_exact(4)
                .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
                .collect();
            let expected_bin = histogram.get(1).copied().unwrap_or(0);
            let pass = expected_bin == 64
                && histogram.iter().enumerate().all(|(bin, count)| {
                    bin == 1 || *count == 0
                });
            let phases = backend.phases();
            println!(
                "histogram[1]={expected_bin} (expect 64) pass={pass} phases=upload {:.3}ms compute {:.3}ms readback {:.3}ms",
                phases.upload_ms, phases.compute_ms, phases.readback_ms
            );
            if !pass {
                std::process::exit(1);
            }
        }
        "gpu-bench" => match gpu_bench::run_gpu_benchmarks() {
            Ok(report) => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialize gpu bench report")
                );
            }
            Err(error) => {
                eprintln!("gpu-bench failed: {error}");
                std::process::exit(1);
            }
        },
        other => {
            eprintln!("unknown subcommand {other:?}; expected `probe`, `bench`, `backend`, `backend-smoke`, or `gpu-bench`");
            std::process::exit(2);
        }
    }
}

fn run_probe() {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_owned());
    let descriptor = wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    };
    let instance = wgpu::Instance::new(descriptor);
    let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()));

    let mut adapter_reports = Vec::new();
    let mut smoke_target: Option<(usize, wgpu::Adapter)> = None;
    for (index, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        let limits = adapter.limits();
        adapter_reports.push(AdapterReport {
            index,
            backend: format!("{:?}", info.backend),
            name: info.name.clone(),
            vendor: info.vendor,
            device: info.device,
            device_type: format!("{:?}", info.device_type),
            max_buffer_size: limits.max_buffer_size,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x,
            max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
        });
        if smoke_target.is_none() && info.device_type != wgpu::DeviceType::Cpu {
            smoke_target = Some((index, adapter.clone()));
        }
    }

    let compute_smoke = match smoke_target {
        Some((index, adapter)) => run_compute_smoke(index, &adapter),
        None => SmokeReport {
            adapter_index: usize::MAX,
            adapter_name: "no non-CPU adapter".to_owned(),
            element_count: 0,
            correct_elements: 0,
            passed: false,
            error: Some("no non-CPU adapter enumerated".to_owned()),
        },
    };

    let report = Report {
        host,
        wgpu_generation: "wgpu 30".to_owned(),
        adapters: adapter_reports,
        compute_smoke,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("serialize report")
    );
}

fn run_compute_smoke(index: usize, adapter: &wgpu::Adapter) -> SmokeReport {
    let attempt = pollster::block_on(try_compute_smoke(adapter));
    match attempt {
        Ok((element_count, correct_elements)) => SmokeReport {
            adapter_index: index,
            adapter_name: adapter.get_info().name,
            element_count,
            correct_elements,
            passed: correct_elements == element_count,
            error: None,
        },
        Err(error) => SmokeReport {
            adapter_index: index,
            adapter_name: adapter.get_info().name,
            element_count: 0,
            correct_elements: 0,
            passed: false,
            error: Some(error),
        },
    }
}

async fn try_compute_smoke(adapter: &wgpu::Adapter) -> Result<(usize, usize), String> {
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .map_err(|error| format!("request_device failed: {error}"))?;

    const COUNT: usize = 4096;
    let input: Vec<f32> = (0..COUNT).map(|i| i as f32).collect();
    let expected: Vec<f32> = input.iter().map(|value| value * 2.0 + 1.0).collect();

    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-smoke"),
        size: (COUNT * std::mem::size_of::<f32>()) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&input));

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("gpu-lab-smoke-shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(SMOKE_SHADER)),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("gpu-lab-smoke-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gpu-lab-smoke-bind-group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("gpu-lab-smoke-pipeline-layout"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("gpu-lab-smoke-pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let mut encoder =
        device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(COUNT.div_ceil(64) as u32, 1, 1);
    }
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gpu-lab-smoke-readback"),
        size: buffer.size(),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_buffer_to_buffer(&buffer, 0, &readback, 0, buffer.size());
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .map_err(|error| format!("device poll failed: {error}"))?;
    receiver
        .recv()
        .map_err(|_| "map callback dropped".to_owned())?
        .map_err(|error| format!("buffer map failed: {error}"))?;

    let data = readback
        .slice(..)
        .get_mapped_range()
        .map_err(|error| format!("get mapped range failed: {error}"))?;
    let output: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    readback.unmap();

    let correct = output
        .iter()
        .zip(&expected)
        .filter(|(actual, expected)| (**actual - **expected).abs() < 1e-4)
        .count();
    Ok((COUNT, correct))
}
