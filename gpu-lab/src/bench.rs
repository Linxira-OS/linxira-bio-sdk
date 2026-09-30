//! Deterministic synthetic workloads and CPU baseline kernels for M5-G1.
//! All data is generated from a fixed seed so future GPU/SIMD runs can compare
//! wall time, peak RSS, and result checksums against this exact input.
//!
//! SIMD policy: x86-64 only (AMD and Intel via runtime feature detection) —
//! AVX-512F and AVX2+FMA tiers for f64 kernels, scalar fallback otherwise.
//! ARM is deliberately not adapted; non-x86-64 hosts run scalar only and the
//! report says so.

use serde::Serialize;
use std::arch::x86_64::{
    __m256d, __m512d, _mm_add_pd, _mm256_extractf128_pd, _mm256_fmadd_pd, _mm256_loadu_pd,
    _mm512_fmadd_pd, _mm512_loadu_pd, _mm512_reduce_add_pd,
};
use std::collections::HashMap;
use std::time::Instant;

/// Deterministic PRNG so every run synthesizes identical inputs.
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
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
}

#[derive(Serialize)]
pub struct ImplResult {
    pub name: String,
    pub wall_ms: f64,
    pub checksum: String,
}

#[derive(Serialize)]
pub struct KernelBaseline {
    pub kernel: String,
    pub parameters: String,
    pub implementations: Vec<ImplResult>,
    pub selected: String,
    pub detected: String,
    pub peak_rss_mb: Option<f64>,
}

#[derive(Serialize)]
pub struct BenchReport {
    pub seed: u64,
    pub simd_policy: String,
    pub kernels: Vec<KernelBaseline>,
}

pub fn run_benchmarks() -> BenchReport {
    let seed = 0x5EED_5EED_5EED_5EEDu64;
    let mut kernels = Vec::new();
    kernels.push(run_pearson_bench(&pearson_input(seed)));
    kernels.push(bench_kmer(seed));
    kernels.push(bench_quality_histogram(seed));
    BenchReport {
        seed,
        simd_policy: "x86-64 only (AMD+Intel, runtime detect): avx512f -> avx2+fma -> scalar; no ARM adaptation".to_owned(),
        kernels,
    }
}

// ---------------------------------------------------------------------------
// Pearson column-pair correlations (f64 reference + SIMD tiers)
// ---------------------------------------------------------------------------

/// Fixed-seed Pearson input (column-major, pre-centered) shared by CPU tiers
/// and the GPU comparison so every implementation consumes identical data.
pub struct PearsonInput {
    pub rows: usize,
    pub cols: usize,
    pub columns: Vec<Vec<f64>>,
    pub squares: Vec<f64>,
}

pub fn pearson_input(seed: u64) -> PearsonInput {
    let rows = 2000usize;
    let cols = 500usize;
    let mut rng = SplitMix64(seed ^ 0x01);
    let columns = pearson_columns(rows, cols, &mut rng);
    let squares: Vec<f64> = columns
        .iter()
        .map(|column| column.iter().map(|value| value * value).sum())
        .collect();
    PearsonInput {
        rows,
        cols,
        columns,
        squares,
    }
}

fn pearson_columns(rows: usize, cols: usize, rng: &mut SplitMix64) -> Vec<Vec<f64>> {
    // Column-major layout with per-column pre-centering: every implementation
    // (scalar, AVX2, AVX-512, GPU) consumes the exact same contiguous input,
    // so the comparison isolates instruction-set effects, not memory layout.
    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(cols);
    for _ in 0..cols {
        let mut column = Vec::with_capacity(rows);
        let mut total = 0.0;
        for _ in 0..rows {
            let value = rng.next_f64();
            total += value;
            column.push(value);
        }
        let mean = total / rows as f64;
        for value in &mut column {
            *value -= mean;
        }
        columns.push(column);
    }
    columns
}

/// (a, b) pair list in the same nested-loop order as the CPU implementations.
pub fn pearson_pairs(cols: usize) -> Vec<(u32, u32)> {
    let mut pairs = Vec::with_capacity(cols * (cols - 1) / 2);
    for a in 0..cols {
        for b in (a + 1)..cols {
            pairs.push((a as u32, b as u32));
        }
    }
    pairs
}

/// Fixed-seed synthetic qualities shared with the GPU histogram kernel.
pub fn quality_values(seed: u64) -> Vec<u8> {
    let total = 2_000_000usize * 150usize;
    let mut rng = SplitMix64(seed ^ 0x03);
    (0..total)
        .map(|_| {
            let value = 20.0 + 20.0 * rng.next_f64();
            value.clamp(0.0, 93.0) as u8
        })
        .collect()
}

pub fn run_pearson_bench(input: &PearsonInput) -> KernelBaseline {
    let rows = input.rows;
    let cols = input.cols;
    let columns = &input.columns;
    let squares = &input.squares;

    let mut implementations = Vec::new();

    let scalar = timed(|| pearson_pairs_scalar(columns, squares));
    implementations.push(ImplResult {
        name: "scalar".to_owned(),
        wall_ms: scalar.0,
        checksum: format!("sum-r={:.9}", scalar.1),
    });

    let (tier, has_avx2_fma, has_avx512) = simd_tier();
    if has_avx2_fma {
        let avx2 = timed(|| unsafe { pearson_pairs_avx2(columns, squares) });
        implementations.push(ImplResult {
            name: "avx2+fma".to_owned(),
            wall_ms: avx2.0,
            checksum: format!("sum-r={:.9}", avx2.1),
        });
    }
    if has_avx512 {
        let avx512 = timed(|| unsafe { pearson_pairs_avx512(columns, squares) });
        implementations.push(ImplResult {
            name: "avx512f".to_owned(),
            wall_ms: avx512.0,
            checksum: format!("sum-r={:.9}", avx512.1),
        });
    }

    let selected = if has_avx512 {
        "avx512f".to_owned()
    } else if has_avx2_fma {
        "avx2+fma".to_owned()
    } else {
        "scalar".to_owned()
    };

    KernelBaseline {
        kernel: "pearson-column-pairs".to_owned(),
        parameters: format!(
            "rows={rows} cols={cols} pairs={} layout=column-major-precentered",
            cols * (cols - 1) / 2
        ),
        implementations,
        selected,
        detected: tier,
        peak_rss_mb: peak_rss_mb(),
    }
}

/// f64 sequential reference (the audit tier for GPU f32 comparison).
pub fn pearson_pairs_scalar(columns: &[Vec<f64>], squares: &[f64]) -> f64 {
    let cols = columns.len();
    let rows = columns.first().map(Vec::len).unwrap_or(0);
    let mut sum_r = 0.0;
    for a in 0..cols {
        let column_a = &columns[a];
        for b in (a + 1)..cols {
            let column_b = &columns[b];
            let mut num = 0.0;
            for row in 0..rows {
                num += column_a[row] * column_b[row];
            }
            let denominator = (squares[a] * squares[b]).sqrt();
            if denominator > 0.0 {
                sum_r += num / denominator;
            }
        }
    }
    sum_r
}

/// num accumulates in 4 lanes; the final horizontal add differs from the
/// sequential scalar order by normal floating-point reassociation (~1e-12
/// relative on this workload) — accepted under the tolerance-based
/// consistency rule and recorded per implementation.
#[target_feature(enable = "avx2,fma")]
unsafe fn pearson_pairs_avx2(columns: &[Vec<f64>], squares: &[f64]) -> f64 {
    let cols = columns.len();
    let rows = columns.first().map(Vec::len).unwrap_or(0);
    let mut sum_r = 0.0;
    for a in 0..cols {
        let column_a = &columns[a];
        for b in (a + 1)..cols {
            let column_b = &columns[b];
            let mut num = unsafe { std::arch::x86_64::_mm256_setzero_pd() };
            let mut row = 0usize;
            while row + 4 <= rows {
                let va = unsafe { _mm256_loadu_pd(column_a.as_ptr().add(row)) };
                let vb = unsafe { _mm256_loadu_pd(column_b.as_ptr().add(row)) };
                num = unsafe { _mm256_fmadd_pd(va, vb, num) };
                row += 4;
            }
            let mut tail = 0.0;
            while row < rows {
                tail += column_a[row] * column_b[row];
                row += 1;
            }
            let mut partial = unsafe { horizontal_add_256(num) };
            partial += tail;
            let denominator = (squares[a] * squares[b]).sqrt();
            if denominator > 0.0 {
                sum_r += partial / denominator;
            }
        }
    }
    sum_r
}

#[target_feature(enable = "avx512f")]
unsafe fn pearson_pairs_avx512(columns: &[Vec<f64>], squares: &[f64]) -> f64 {
    let cols = columns.len();
    let rows = columns.first().map(Vec::len).unwrap_or(0);
    let mut sum_r = 0.0;
    for a in 0..cols {
        let column_a = &columns[a];
        for b in (a + 1)..cols {
            let column_b = &columns[b];
            let mut num: __m512d = unsafe { std::arch::x86_64::_mm512_setzero_pd() };
            let mut row = 0usize;
            while row + 8 <= rows {
                let va = unsafe { _mm512_loadu_pd(column_a.as_ptr().add(row)) };
                let vb = unsafe { _mm512_loadu_pd(column_b.as_ptr().add(row)) };
                num = unsafe { _mm512_fmadd_pd(va, vb, num) };
                row += 8;
            }
            let mut tail = 0.0;
            while row < rows {
                tail += column_a[row] * column_b[row];
                row += 1;
            }
            let mut partial = unsafe { _mm512_reduce_add_pd(num) };
            partial += tail;
            let denominator = (squares[a] * squares[b]).sqrt();
            if denominator > 0.0 {
                sum_r += partial / denominator;
            }
        }
    }
    sum_r
}

unsafe fn horizontal_add_256(value: __m256d) -> f64 {
    unsafe {
        let hi128 = _mm256_extractf128_pd(value, 1);
        let lo128 = std::arch::x86_64::_mm256_castpd256_pd128(value);
        let sum128 = _mm_add_pd(hi128, lo128);
        // Permute swaps the two lanes, so adding yields the total in both.
        let permuted = std::arch::x86_64::_mm_permute_pd(sum128, 0b01);
        let total = _mm_add_pd(sum128, permuted);
        std::arch::x86_64::_mm_cvtsd_f64(total)
    }
}

#[cfg(target_arch = "x86_64")]
fn simd_tier() -> (String, bool, bool) {
    let has_avx2_fma =
        std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma");
    let has_avx512 = std::arch::is_x86_feature_detected!("avx512f");
    let mut tier = String::from("x86-64:");
    if has_avx512 {
        tier.push_str("+avx512f");
    }
    if has_avx2_fma {
        tier.push_str("+avx2+fma");
    }
    if !has_avx512 && !has_avx2_fma {
        tier.push_str("baseline-only");
    }
    (tier, has_avx2_fma, has_avx512)
}

#[cfg(not(target_arch = "x86_64"))]
fn simd_tier() -> (String, bool, bool) {
    (
        "non-x86-64:scalar-only(no ARM adaptation)".to_owned(),
        false,
        false,
    )
}

/// Median wall of 3 repeated runs (ledger policy); the deterministic body
/// returns the same result each time.
fn timed(mut body: impl FnMut() -> f64) -> (f64, f64) {
    let mut walls = [0.0f64; 3];
    let mut result = 0.0;
    for run in 0..3 {
        let start = Instant::now();
        result = body();
        walls[run] = start.elapsed().as_secs_f64() * 1000.0;
    }
    walls.sort_by(|a, b| a.total_cmp(b));
    (walls[1], result)
}

// ---------------------------------------------------------------------------
// k-mer counting (hash-bound; SIMD kept out deliberately, recorded as such)
// ---------------------------------------------------------------------------

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
        parameters: format!("reads={read_count} len={read_length} k={k} distinct={distinct}"),
        implementations: vec![ImplResult {
            name: "scalar".to_owned(),
            wall_ms,
            checksum: format!("total={total_kmers} distinct={distinct}"),
        }],
        selected: "scalar".to_owned(),
        detected: "hash-bound;SIMD-not-applicable".to_owned(),
        peak_rss_mb: peak_rss_mb(),
    }
}

// ---------------------------------------------------------------------------
// Quality histogram (bandwidth-bound integer kernel)
// ---------------------------------------------------------------------------

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
        implementations: vec![ImplResult {
            name: "scalar".to_owned(),
            wall_ms,
            checksum,
        }],
        selected: "scalar".to_owned(),
        detected: "bandwidth-bound;SIMD-deferred".to_owned(),
        peak_rss_mb: peak_rss_mb(),
    }
}

fn peak_rss_mb() -> Option<f64> {
    memory_stats::memory_stats().map(|stats| stats.physical_mem as f64 / 1024.0 / 1024.0)
}
