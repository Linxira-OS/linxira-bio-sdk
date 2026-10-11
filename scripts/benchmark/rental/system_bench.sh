#!/bin/bash
# System bandwidth bench: data-disk dd (seq write/read) + RAM STREAM (C, OMP).
exec > /root/sysbench.log 2>&1
set -x
cd /root/autodl-tmp || exit 8

# --- disk: data disk (/root/autodl-tmp), 2 GiB sequential ---
sync
dd if=/dev/zero of=disktest.bin bs=1M count=2048 conv=fdatasync 2>&1 | tail -1
sync
dd if=disktest.bin of=/dev/null bs=1M 2>&1 | tail -1
rm -f disktest.bin
# root overlay (system disk) quick 1 GiB probe
cd /root && dd if=/dev/zero of=disktest_sys.bin bs=1M count=1024 conv=fdatasync 2>&1 | tail -1
rm -f disktest_sys.bin

# --- RAM: classic STREAM, 4 kernels ---
cat > stream.c <<'CEOF'
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
static double now(void) {
    struct timespec ts; clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec + ts.tv_nsec * 1e-9;
}
#define N 80000000L  /* ~2.3 GiB working set: exceeds any plausible cache */
static double a[N], b[N], c[N];
int main(void) {
    for (long i = 0; i < N; i++) { a[i] = 1.0; b[i] = 2.0; c[i] = 0.0; }
    double bytes[4] = {2, 2, 3, 3};
    const char *names[4] = {"Copy", "Scale", "Add", "Triad"};
    for (int k = 0; k < 4; k++) {
        double best = 0;
        for (int rep = 0; rep < 3; rep++) {
            double t0 = now();
            if (k == 0) for (long i = 0; i < N; i++) c[i] = a[i];
            if (k == 1) for (long i = 0; i < N; i++) b[i] = 3.0 * c[i];
            if (k == 2) for (long i = 0; i < N; i++) c[i] = a[i] + b[i];
            if (k == 3) for (long i = 0; i < N; i++) a[i] = b[i] + 3.0 * c[i];
            double dt = now() - t0;
            double gbs = bytes[k] * (double)N * 8.0 / dt / 1e9;
            if (gbs > best) best = gbs;
        }
        printf("%s: %.1f GB/s\n", names[k], best);
    }
    if (a[N/2] == 12345.0) printf("(parity guard)\n");
    return 0;
}
CEOF
gcc -O3 -march=native stream.c -o stream_seq
echo "--- RAM single-thread ---"
./stream_seq
gcc -O3 -march=native -fopenmp stream.c -o stream_omp
echo "--- RAM OMP nproc ---"
OMP_NUM_THREADS=$(nproc) ./stream_omp
nproc
echo "SYSBENCH-DONE"
