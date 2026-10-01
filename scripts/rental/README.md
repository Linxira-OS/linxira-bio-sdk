# Cloud-rental reproduction sources (2026-10-02 rounds)

Reproduction material for `docs/engine-evals/gpu-rental-2026-10-02.md`,
`gpu-rental-round2-s4000-2026-10-02.md`, and the round-3 follow-up. These
files stay in the repository for reproducibility; they are **not** part of
any release bundle and are never shipped to end users.

## Single-source dual-vendor kernels

`bench_suite.mu` compiles with both vendor toolchains from one file:

    # MUSA (MTT S4000, mcc 3.1.0)
    mcc -O3 bench_suite.mu -o bench_mu -lmusart
    # CUDA (RTX 4080 SUPER, nvcc 11.8+)
    cp bench_suite.mu bench_suite.cu && nvcc -O3 -arch=sm_89 bench_suite.cu -o bench_cu

Kernels: device copy (DRAM rw), STREAM-triad, 256-bin histogram
(shared-memory + atomics), Pearson v2 (4000x1000 -> 499500 pairs).
Each is median-of-3 event-timed and correctness-checked; exit code 0 iff
all checks pass. `pearson_bandwidth.mu` is the 2000x500 variant used for
the first bandwidth-twin data point.

## Docking study

`dock-study.sh` (128-core vGPU instance) and `dock-study-s4000.sh`
(15-core S4000) sweep `--exhaustiveness {8,16,32,64} x seeds {42,43,44}`
plus a `--cpu` thread sweep at fixed seed, through the
`chemistry.dock.v1` CLI. `analyze_dock.py` computes heavy-atom in-place
RMSD of each best mode against the official 1iep reference pose and
writes `rmsd_vs_official.tsv` + `summary.tsv`.

Prerequisites on the rented box: repo cloned and built
(`cargo build --release -p linxira-bio-cli`), vina probed by name, and
the rental bundle extracted at `/root/autodl-tmp/bundle` (built by
`scripts/prepare-rental-bundle.py`). Timing uses `date +%s.%N` because
the containers ship without `/usr/bin/time`.
