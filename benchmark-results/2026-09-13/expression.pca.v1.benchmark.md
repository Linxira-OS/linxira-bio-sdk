# Benchmark: expression.pca.v1

- dataset_class: matrix
- consistency: Consistent
- sampled_at: 2026-09-13T12:39:16Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 5 | 40.0 | 30.0 | 40.0 | 0.0 | 6.5 |
| python | 5 | 300.0 | 290.0 | 330.0 | 20.0 | 44.0 |
| r | 5 | 460.0 | 430.0 | 530.0 | 30.0 | 70.9 |

speedup (python/rust median wall): 7.50x
memory saving (1 - rust/python median peak RSS): 85.1%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 12.9 | 287.1 | 43.8 |
| r | 33.0 | 427.0 | 69.9 |

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
