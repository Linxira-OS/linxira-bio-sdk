# Benchmark: set.venn.v1

- dataset_class: other
- consistency: Consistent
- sampled_at: 2026-09-13T12:39:30Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 5 | 40.0 | 40.0 | 40.0 | 0.0 | 6.3 |
| python | 5 | 310.0 | 290.0 | 330.0 | 20.0 | 43.8 |
| r | 5 | 400.0 | 390.0 | 530.0 | 20.0 | 70.8 |

speedup (python/rust median wall): 7.75x
memory saving (1 - rust/python median peak RSS): 85.7%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 10.5 | 299.5 | 43.7 |
| r | 26.8 | 373.2 | 69.8 |

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
