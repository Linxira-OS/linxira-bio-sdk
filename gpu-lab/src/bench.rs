//! Deterministic synthetic workloads and single-threaded CPU baseline kernels
//! for M5-G1. All data is generated from a fixed seed so future GPU runs can
//! compare wall time, peak RSS, and result checksums against this exact input.

use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;

/// Deterministic PCG-style PRNG so every run synthesizes identical inputs.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f64(&mut self) -> f64 {
        // Uniform in [0, 1) with 53-bit resolution.
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
}

#[derive(Serialize)]
pub struct KernelBaseline {
    pub kernel: String,
    pub parameters: String,
    pub wall_ms: f64,
    pub peak_rss_mb: Option<f64>,
    pub checksum: String,
}

#[derive(Serialize)]
pub struct BenchReport {
    pub seed: u64,
    pub kernels: Vec<KernelBaseline>,
}

pub fn run_benchmarks() -> BenchReport {
    let seed = 0x5EED_5EED_5EED_5EEDu64;
    let mut kernels = Vec::new();
    kernels.push(bench_pearson(seed));
    kernels.push(bench_kmer(seed));
    kernels.push(bench_quality_histogram(seed));
    BenchReport { seed, kernels }
}

/// Pearson correlation across all column pairs of a rows x cols f64 matrix.
/// rows = features, cols = samples (e.g. 2000 x 500 -> 124,750 pairs).
fn bench_pearson(seed: u64) -> KernelBaseline {
    let rows = 2000usize;
    let cols = 500usize;
    let mut rng = SplitMix64(seed ^ 0x01);
    let mut matrix = vec![0.0f64; rows * cols];
    for value in &mut matrix {
        *value = rng.next_f64();
    }

    let start = Instant::now();
    let mut col_means = vec![0.0f64; cols];
    for row in 0..rows {
        for col in 0..cols {
            col_means[col] += matrix[row * cols + col];
        }
    }
    for mean in &mut col_means {
        *mean /= rows as f64;
    }
    let mut checksum = 0.0f64;
    for a in 0..cols {
        for b in (a + 1)..cols {
            let mut num = 0.0;
            let mut da = 0.0;
            let mut db = 0.0;
            for row in 0..rows {
                let va = matrix[row * cols + a] - col_means[a];
                let vb = matrix[row * cols + b] - col_means[b];
                num += va * vb;
                da += va * va;
                db += vb * vb;
            }
            let denominator = (da * db).sqrt();
            if denominator > 0.0 {
                checksum += num / denominator;
            }
        }
    }
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    KernelBaseline {
        kernel: "pearson-column-pairs".to_owned(),
        parameters: format!("rows={rows} cols={cols} pairs={}", cols * (cols - 1) / 2),
        wall_ms,
        peak_rss_mb: peak_rss_mb(),
        checksum: format!("sum-r={checksum:.9}"),
    }
}

/// Canonical k-mer counting (k=31) over synthetic 150 bp reads, 2-bit packed.
fn bench_kmer(seed: u64) -> KernelBaseline {
    let read_count = 200_000usize;
    let read_length = 150usize;
    let k = 31usize;
    let mut rng = SplitMix64(seed ^ 0x02);
    let base_bits: [u64; 4] = [0, 1, 2, 3];
    let mut reads = Vec::with_capacity(read_count * read_length);
    for _ in 0..read_count {
        for _ in 0..read_length {
            reads.push(base_bits[(rng.next_u32() as usize) & 3]);
        }
    }

    let start = Instant::now();
    let mut counts: HashMap<u64, u32> = HashMap::new();
    for read_index in 0..read_count {
        let read = &reads[read_index * read_length..(read_index + 1) * read_length];
        let mut packed: u64 = 0;
        for (position, base) in read.iter().enumerate() {
            packed = (packed << 2) | base;
            if position + 1 >= k {
                *counts.entry(packed).or_insert(0) += 1;
            }
        }
    }
    let total_kmers = counts.values().map(|count| u64::from(*count)).sum::<u64>();
    let distinct = counts.len();
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    KernelBaseline {
        kernel: "kmer-count".to_owned(),
        parameters: format!(
            "reads={read_count} len={read_length} k={k} distinct={distinct}"
        ),
        wall_ms,
        peak_rss_mb: peak_rss_mb(),
        checksum: format!("total={total_kmers} distinct={distinct}"),
    }
}

/// Phred quality histogram (256 bins) over synthetic per-base qualities.
fn bench_quality_histogram(seed: u64) -> KernelBaseline {
    let read_count = 2_000_000usize;
    let read_length = 150usize;
    let total = read_count * read_length;
    let mut rng = SplitMix64(seed ^ 0x03);
    let qualities: Vec<u8> = (0..total)
        .map(|_| {
            let value = 20.0 + 20.0 * rng.next_f64();
            value.clamp(0.0, 93.0) as u8
        })
        .collect();

    let start = Instant::now();
    let mut histogram = vec![0u64; 256];
    for quality in &qualities {
        histogram[usize::from(*quality)] += 1;
    }
    let wall_ms = start.elapsed().as_secs_f64() * 1000.0;
    let checksum = histogram
        .iter()
        .enumerate()
        .filter(|(_, count)| **count > 0)
        .map(|(bin, count)| format!("{bin}:{count}"))
        .collect::<Vec<_>>()
        .join(",");
    KernelBaseline {
        kernel: "quality-histogram".to_owned(),
        parameters: format!("bases={total} bins=256"),
        wall_ms,
        peak_rss_mb: peak_rss_mb(),
        checksum,
    }
}

fn peak_rss_mb() -> Option<f64> {
    memory_stats::memory_stats().map(|stats| stats.physical_mem as f64 / 1024.0 / 1024.0)
}
