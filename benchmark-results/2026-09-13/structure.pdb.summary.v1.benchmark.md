# Benchmark: structure.pdb.summary.v1

- dataset_class: structure
- consistency: Consistent
- sampled_at: 2026-09-13T12:38:37Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 5 | 40.0 | 40.0 | 40.0 | 0.0 | 6.0 |
| python | 5 | 320.0 | 300.0 | 330.0 | 30.0 | 43.8 |
| r | 5 | 410.0 | 390.0 | 430.0 | 20.0 | 70.8 |

speedup (python/rust median wall): 8.00x
memory saving (1 - rust/python median peak RSS): 86.3%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 12.3 | 307.7 | 43.8 |
| r | 32.6 | 377.4 | 70.1 |

## Environment

- os: linux 6.18.33.2-microsoft-standard-WSL2
- distro: Arch Linux (WSL distro: arch-linux-current, wsl2)
- guest cpu: Intel(R) Core(TM) Ultra 5 225H
- guest memory: 7940 MB
- host os (Windows): Microsoft Windows [Version 10.0.26200.9168]
- host model: XIAOMI REDMI Book Pro 14 2025
- host cpu: Intel(R) Core(TM) Ultra 5 225H (14 logical processors)
- host memory: 32189 MB
- engine: 1.0.1
- python: Python 3.14.6
- R: R version 4.6.1 (2026-06-24) -- "Happy Hop"
- container: no
- page cache: warm
- timing precision: high
