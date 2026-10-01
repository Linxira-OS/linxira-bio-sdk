// pearson_bandwidth.mu — bandwidth-twin kernel, single source for MUSA (mcc)
// and CUDA (nvcc). Mirrors gpu-lab Pearson v2: one 64-thread block per column
// pair, five-accumulator shared-memory tree reduction over rows.
//
// Build (MUSA):  mcc pearson_bandwidth.mu -o pearson_mu -lmusart
// Build (CUDA):  nvcc -O3 -arch=sm_89 pearson_bandwidth.mu -o pearson_cu
// (rename to .cu for nvcc; the file extension is the only difference)
//
// Output: median compute-phase time of 3 runs + effective logical bandwidth.

#ifdef __MUSA__
#include <musa_runtime.h>
#define GPU_CALL(x) musa##x
#define GPU_PREFIX musa
#else
#include <cuda_runtime.h>
#define GPU_CALL(x) cuda##x
#define GPU_PREFIX cuda
#endif
#define GPU_PASTE2(a, b) a##b
#define GPU_PASTE(a, b) GPU_PASTE2(a, b)
#define GPU_OK GPU_PASTE(GPU_PREFIX, Success)
#define GPU_H2D GPU_PASTE(GPU_PREFIX, MemcpyHostToDevice)
#define GPU_D2H GPU_PASTE(GPU_PREFIX, MemcpyDeviceToHost)

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define ROWS 2000
#define COLS 500
#define PAIRS (COLS * (COLS - 1) / 2)
#define BLOCK 64
#define SEED 0x5EED5EED5EED5EEDULL

static unsigned long long xorshift64star(unsigned long long *s) {
    unsigned long long x = *s;
    x ^= x >> 12; x ^= x << 25; x ^= x >> 27;
    *s = x;
    return x * 0x2545F4914F6CDD1DULL;
}

__global__ void pearson_v2(const float *in, float *out, int rows, int cols) {
    int pair = blockIdx.x;
    if (pair >= cols * (cols - 1) / 2) return;
    // column pair from flattened triangle index (matches gpu-lab v2 order)
    int c0 = -1, c1 = -1, remaining = pair;
    for (int c = 0; c < cols - 1; c++) {
        int width = cols - 1 - c;
        if (remaining < width) { c0 = c; c1 = c + 1 + remaining; break; }
        remaining -= width;
    }
    if (c0 < 0) return;

    float sx = 0.f, sy = 0.f, sxy = 0.f, sx2 = 0.f, sy2 = 0.f;
    for (int r = threadIdx.x; r < rows; r += blockDim.x) {
        float x = in[r * cols + c0];
        float y = in[r * cols + c1];
        sx += x; sy += y; sxy += x * y; sx2 += x * x; sy2 += y * y;
    }

    __shared__ float red[5][BLOCK];
    #pragma unroll
    for (int lane = BLOCK / 2; lane > 0; lane >>= 1) {
        red[0][threadIdx.x] = sx;
        red[1][threadIdx.x] = sy;
        red[2][threadIdx.x] = sxy;
        red[3][threadIdx.x] = sx2;
        red[4][threadIdx.x] = sy2;
        __syncthreads();
        if (threadIdx.x < lane) {
            sx += red[0][threadIdx.x + lane];
            sy += red[1][threadIdx.x + lane];
            sxy += red[2][threadIdx.x + lane];
            sx2 += red[3][threadIdx.x + lane];
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

int main() {
    size_t input_floats = (size_t)ROWS * COLS;
    float *hin = (float *)malloc(input_floats * sizeof(float));
    float *hout = (float *)malloc((size_t)PAIRS * sizeof(float));
    unsigned long long s = SEED;
    for (size_t i = 0; i < input_floats; i++)
        hin[i] = (float)((double)xorshift64star(&s) / 18446744073709551616.0) * 2.f - 1.f;

    float *din, *dout;
    if (GPU_CALL(Malloc((void **)&din, input_floats * sizeof(float))) != GPU_OK) { printf("malloc in failed\n"); return 2; }
    if (GPU_CALL(Malloc((void **)&dout, (size_t)PAIRS * sizeof(float))) != GPU_OK) { printf("malloc out failed\n"); return 2; }
    GPU_CALL(Memcpy(din, hin, input_floats * sizeof(float), GPU_H2D));

    #ifdef __MUSA__
    musaEvent_t t0, t1;
    musaEventCreate(&t0); musaEventCreate(&t1);
    #else
    cudaEvent_t t0, t1;
    cudaEventCreate(&t0); cudaEventCreate(&t1);
    #endif

    float ms[3];
    for (int run = 0; run < 3; run++) {
        #ifdef __MUSA__
        musaEventRecord(t0);
        #else
        cudaEventRecord(t0);
        #endif
        pearson_v2<<<PAIRS, BLOCK>>>(din, dout, ROWS, COLS);
        #ifdef __MUSA__
        musaEventRecord(t1);
        musaEventSynchronize(t1);
        musaEventElapsedTime(&ms[run], t0, t1);
        #else
        cudaEventRecord(t1);
        cudaEventSynchronize(t1);
        cudaEventElapsedTime(&ms[run], t0, t1);
        #endif
    }
    for (int a = 0; a < 2; a++) for (int b = a + 1; b < 3; b++)
        if (ms[b] < ms[a]) { float t = ms[a]; ms[a] = ms[b]; ms[b] = t; }

    GPU_CALL(Memcpy(hout, dout, (size_t)PAIRS * sizeof(float), GPU_D2H));

    // CPU spot check on 8 spread pairs (f64 reference, relative tolerance)
    double max_rel = 0.0;
    int checked = 0;
    for (int k = 0; k < 8; k++) {
        int pair = k * (PAIRS / 8);
        int c0 = -1, c1 = -1, remaining = pair;
        for (int c = 0; c < COLS - 1; c++) {
            int width = COLS - 1 - c;
            if (remaining < width) { c0 = c; c1 = c + 1 + remaining; break; }
            remaining -= width;
        }
        double Sx = 0, Sy = 0, Sxy = 0, Sx2 = 0, Sy2 = 0;
        for (int r = 0; r < ROWS; r++) {
            double x = hin[r * COLS + c0], y = hin[r * COLS + c1];
            Sx += x; Sy += y; Sxy += x * y; Sx2 += x * x; Sy2 += y * y;
        }
        double n = ROWS;
        double ref = (n * Sxy - Sx * Sy) / sqrt((n * Sx2 - Sx * Sx) * (n * Sy2 - Sy * Sy));
        double rel = (ref != 0.0) ? fabs((hout[pair] - ref) / ref) : fabs(hout[pair]);
        if (rel > max_rel) max_rel = rel;
        checked++;
    }

    double bytes = (double)PAIRS * ROWS * 2 * sizeof(float);
    printf("median compute = %.3f ms | logical bw = %.1f GB/s | max rel err = %.2e (%d pairs checked) | runs = %.3f/%.3f/%.3f ms\n",
           ms[0], bytes / (ms[0] * 1e-3) / 1e9, max_rel, checked, ms[0], ms[1], ms[2]);

    #ifdef __MUSA__
    musaEventDestroy(t0); musaEventDestroy(t1);
    #else
    cudaEventDestroy(t0); cudaEventDestroy(t1);
    #endif
    GPU_CALL(Free(din)); GPU_CALL(Free(dout));
    free(hin); free(hout);
    return (max_rel < 2e-4 && checked == 8) ? 0 : 1;
}
