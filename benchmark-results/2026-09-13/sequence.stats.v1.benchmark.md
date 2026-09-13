# Benchmark: sequence.stats.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-09-13T12:38:59Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 5 | 40.0 | 30.0 | 40.0 | 10.0 | 5.9 |
| python | 5 | 330.0 | 300.0 | 340.0 | 20.0 | 43.8 |
| r | 5 | 2110.0 | 1980.0 | 2140.0 | 50.0 | 237.6 |

speedup (python/rust median wall): 8.25x
memory saving (1 - rust/python median peak RSS): 86.5%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 10.7 | 319.3 | 43.8 |
| r | 91.4 | 2018.6 | 232.3 |

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
