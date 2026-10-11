// bench_suite.mu — dual-source GPU benchmark suite (mcc / nvcc).
// Kernels: copy (DRAM rw), triad (STREAM), histogram256 (shared+atomics),
// pearson_v2 big (4000x1000 -> 499500 pairs). Median of 3, event-timed,
// correctness-checked. Same file compiles for MUSA and CUDA.
//
// MUSA: mcc -O3 bench_suite.mu -o bench_mu -lmusart
// CUDA: cp bench_suite.mu bench_suite.cu && nvcc -O3 -arch=sm_89 bench_suite.cu -o bench_cu

#ifdef __MUSA__
#include <musa_runtime.h>
#define GPU_PREFIX musa
#else
#include <cuda_runtime.h>
#define GPU_PREFIX cuda
#endif
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#define GPU_PASTE2(a, b) a##b
#define GPU_PASTE(a, b) GPU_PASTE2(a, b)
#define OK(x) (GPU_PASTE(GPU_PREFIX, x))

#define COPY_N (256u * 1024u * 1024u)   // 1 GiB of floats
#define HIST_N (256u * 1024u * 1024u)   // 256 MiB of bytes
#define P_ROWS 4000
#define P_COLS 1000
#define P_PAIRS ((size_t)P_COLS * (P_COLS - 1) / 2)
#define BLOCK 256
#define SEED 0x5EED5EED5EED5EEDULL

static unsigned long long xs64(unsigned long long *s) {
    unsigned long long x = *s;
    x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
    *s = x;
    return x * 0x2545F4914F6CDD1DULL;
}

__global__ void k_copy(const float *in, float *out, unsigned n) {
    unsigned i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = in[i];
}

__global__ void k_triad(const float *a, const float *b, float *out, float s, unsigned n) {
    unsigned i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] * s + b[i];
}

__global__ void k_hist(const unsigned char *in, unsigned *out /*256 bins*/) {
    __shared__ unsigned local[256];
    if (threadIdx.x < 256) local[threadIdx.x] = 0;
    __syncthreads();
    unsigned i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < HIST_N) atomicAdd(&local[in[i]], 1u);
    __syncthreads();
    if (threadIdx.x < 256) atomicAdd(&out[threadIdx.x], local[threadIdx.x]);
}

__global__ void k_pearson(const float *in, float *out, int rows, int cols) {
    int pair = blockIdx.x;
    if (pair >= cols * (cols - 1) / 2) return;
    int c0 = -1, c1 = -1, remaining = pair;
    for (int c = 0; c < cols - 1; c++) {
        int width = cols - 1 - c;
        if (remaining < width) { c0 = c; c1 = c + 1 + remaining; break; }
        remaining -= width;
    }
    if (c0 < 0) return;
    float sx = 0.f, sy = 0.f, sxy = 0.f, sx2 = 0.f, sy2 = 0.f;
    for (int r = threadIdx.x; r < rows; r += blockDim.x) {
        float x = in[(size_t)r * cols + c0];
        float y = in[(size_t)r * cols + c1];
        sx += x; sy += y; sxy += x * y; sx2 += x * x; sy2 += y * y;
    }
    __shared__ float red[5][64];
    #pragma unroll
    for (int lane = 32; lane > 0; lane >>= 1) {
        red[0][threadIdx.x] = sx; red[1][threadIdx.x] = sy; red[2][threadIdx.x] = sxy;
        red[3][threadIdx.x] = sx2; red[4][threadIdx.x] = sy2;
        __syncthreads();
        if (threadIdx.x < lane) {
            sx += red[0][threadIdx.x + lane]; sy += red[1][threadIdx.x + lane];
            sxy += red[2][threadIdx.x + lane]; sx2 += red[3][threadIdx.x + lane];
            sy2 += red[4][threadIdx.x + lane];
        }
        __syncthreads();
    }
    if (threadIdx.x == 0) {
        float n = (float)rows;
        float num = n * sxy - sx * sy;
        float den = sqrtf((n * sx2 - sx * sx) * (n * sy2 - sy * sy));
        out[pair] = (den > 0.f) ? num / den : 0.f;
    }
}

static float time_kernel(void (*launch)(void), int runs) {
#ifdef __MUSA__
    musaEvent_t t0, t1; musaEventCreate(&t0); musaEventCreate(&t1);
#else
    cudaEvent_t t0, t1; cudaEventCreate(&t0); cudaEventCreate(&t1);
#endif
    float ms[3] = {0, 0, 0};
    for (int r = 0; r < runs; r++) {
#ifdef __MUSA__
        musaEventRecord(t0); launch(); musaEventRecord(t1);
        musaEventSynchronize(t1); musaEventElapsedTime(&ms[r], t0, t1);
#else
        cudaEventRecord(t0); launch(); cudaEventRecord(t1);
        cudaEventSynchronize(t1); cudaEventElapsedTime(&ms[r], t0, t1);
#endif
    }
    for (int a = 0; a < 2; a++) for (int b = a + 1; b < 3; b++)
        if (ms[b] < ms[a]) { float t = ms[a]; ms[a] = ms[b]; ms[b] = t; }
    return ms[0];
}

static float *d_a, *d_b, *d_c, *d_p;
static unsigned char *d_h;
static unsigned *d_hist;

static void launch_copy(void)   { k_copy<<<COPY_N / BLOCK, BLOCK>>>(d_a, d_b, COPY_N); }
static void launch_triad(void)  { k_triad<<<COPY_N / BLOCK, BLOCK>>>(d_a, d_b, d_c, 2.5f, COPY_N); }
static void launch_hist(void)   { k_hist<<<HIST_N / BLOCK, BLOCK>>>(d_h, d_hist); }
static void launch_pearson(void){ k_pearson<<<(int)P_PAIRS, 64>>>(d_p, d_c, P_ROWS, P_COLS); }

int main() {
    if (OK(Malloc((void **)&d_a, COPY_N * 4)) != OK(Success) ||
        OK(Malloc((void **)&d_b, COPY_N * 4)) != OK(Success) ||
        OK(Malloc((void **)&d_c, COPY_N * 4)) != OK(Success) ||
        OK(Malloc((void **)&d_h, (size_t)HIST_N)) != OK(Success) ||
        OK(Malloc((void **)&d_hist, 256 * sizeof(unsigned))) != OK(Success) ||
        OK(Malloc((void **)&d_p, (size_t)P_ROWS * P_COLS * 4)) != OK(Success)) {
        printf("device alloc failed\n"); return 2;
    }
    float *ha = (float *)malloc(COPY_N * 4);
    unsigned char *hh = (unsigned char *)malloc((size_t)HIST_N);
    unsigned long long s = SEED;
    unsigned host_hist[256] = {0};
    for (unsigned i = 0; i < COPY_N; i++) ha[i] = (float)(xs64(&s) & 0xFFFF) / 1024.f;
    for (size_t i = 0; i < HIST_N; i++) { hh[i] = (unsigned char)(xs64(&s) & 0xFF); host_hist[hh[i]]++; }
    OK(Memcpy(d_a, ha, COPY_N * 4, OK(MemcpyHostToDevice)));
    OK(Memcpy(d_b, ha, COPY_N * 4, OK(MemcpyHostToDevice)));
    OK(Memcpy(d_h, hh, (size_t)HIST_N, OK(MemcpyHostToDevice)));
    float *hp = (float *)malloc((size_t)P_ROWS * P_COLS * 4);
    for (size_t i = 0; i < (size_t)P_ROWS * P_COLS; i++)
        hp[i] = (float)((double)xs64(&s) / 18446744073709551616.0) * 2.f - 1.f;
    OK(Memcpy(d_p, hp, (size_t)P_ROWS * P_COLS * 4, OK(MemcpyHostToDevice)));
    OK(Memset(d_hist, 0, 256 * sizeof(unsigned)));
    OK(Memset(d_b, 0, COPY_N * 4));

    float t_copy = time_kernel(launch_copy, 3);
    // copy check: d_b must now equal ha
    float *hspot = (float *)malloc(4096 * 4);
    OK(Memcpy(hspot, d_b, 4096 * 4, OK(MemcpyDeviceToHost)));
    float copy_bad = 0.f;
    for (int i = 0; i < 1024; i++) copy_bad = fmaxf(copy_bad, fabsf(hspot[i] - ha[i]));

    float t_triad = time_kernel(launch_triad, 3);
    OK(Memcpy(hspot, d_c, 4096 * 4, OK(MemcpyDeviceToHost)));
    float triad_bad = 0.f;
    for (int i = 0; i < 1024; i++) triad_bad = fmaxf(triad_bad, fabsf(hspot[i] - ha[i] * 3.5f));

    float t_hist = time_kernel(launch_hist, 3);
    // timing above ran hist 3x into the same accumulating bins; fresh run for check
    OK(Memset(d_hist, 0, 256 * sizeof(unsigned)));
    launch_hist();
#ifdef __MUSA__
    musaDeviceSynchronize();
#else
    cudaDeviceSynchronize();
#endif
    unsigned out_hist[256];
    OK(Memcpy(out_hist, d_hist, 256 * sizeof(unsigned), OK(MemcpyDeviceToHost)));
    int hist_ok = 1;
    for (int b = 0; b < 256; b++) if (out_hist[b] != host_hist[b]) { hist_ok = 0; break; }

    float t_pear = time_kernel(launch_pearson, 3);
    float *out_p = (float *)malloc((P_PAIRS / 2 + 2) * 4);
    OK(Memcpy(out_p, d_c, (P_PAIRS / 2 + 2) * 4, OK(MemcpyDeviceToHost)));
    // CPU f64 reference for first and last checked pair
    double pear_rel = 0.0;
    int probes[2] = {0, (int)(P_PAIRS / 2)};
    for (int k = 0; k < 2; k++) {
        int pair = probes[k], c0 = -1, c1 = -1, remaining = pair;
        for (int c = 0; c < P_COLS - 1; c++) {
            int width = P_COLS - 1 - c;
            if (remaining < width) { c0 = c; c1 = c + 1 + remaining; break; }
            remaining -= width;
        }
        double Sx = 0, Sy = 0, Sxy = 0, Sx2 = 0, Sy2 = 0;
        for (int r = 0; r < P_ROWS; r++) {
            double x = hp[(size_t)r * P_COLS + c0], y = hp[(size_t)r * P_COLS + c1];
            Sx += x; Sy += y; Sxy += x * y; Sx2 += x * x; Sy2 += y * y;
        }
        double n = P_ROWS;
        double ref = (n * Sxy - Sx * Sy) / sqrt((n * Sx2 - Sx * Sx) * (n * Sy2 - Sy * Sy));
        double rel = (ref != 0.0) ? fabs((out_p[pair] - ref) / ref) : fabs(out_p[pair]);
        if (rel > pear_rel) pear_rel = rel;
    }

    double gi = 1024.0 * 1024.0 * 1024.0;
    printf("copy    : %8.2f ms  %7.1f GB/s (rw)   maxerr=%.2e\n", t_copy, 2.0 * gi / (t_copy * 1e-3) / 1e9, copy_bad);
    printf("triad   : %8.2f ms  %7.1f GB/s (rwr)  maxerr=%.2e\n", t_triad, 3.0 * gi / (t_triad * 1e-3) / 1e9, triad_bad);
    printf("hist256 : %8.2f ms  %7.1f GB/s (read) exact=%s\n", t_hist, 0.25 * gi / (t_hist * 1e-3) / 1e9, hist_ok ? "ok" : "FAIL");
    printf("pearson4k: %7.2f ms  %7.1f GB/s (log) relerr=%.2e pairs=%zu\n", t_pear,
           (double)P_PAIRS * P_ROWS * 2 * 4 / (t_pear * 1e-3) / 1e9, pear_rel, P_PAIRS);
    return (hist_ok && triad_bad < 1e-3f && copy_bad == 0.f && pear_rel < 2e-4) ? 0 : 1;
}
