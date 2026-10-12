# Benchmark: sequence.stats.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-12T00:16:08Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 810.0 | 810.0 | 820.0 | 10.0 | 5.9 |
| python | 10 | 835.0 | 830.0 | 950.0 | 62.5 | 69.7 |
| r | 10 | 1820.0 | 1810.0 | 2120.0 | 7.5 | 321.1 |

speedup (python/rust median wall): 1.03x
memory saving (1 - rust/python median peak RSS): 91.6%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 471.8 | 363.2 | 69.7 |
| r | 317.8 | 1502.2 | 319.8 |

## Environment

- code revision: 89882a5a31714e12e0dee4333d20f2b50b0c99c5
- os: linux 7.2.9-1-cachyos
- distro: CachyOS
- guest cpu: Intel(R) Pentium(R) Gold G7400
- guest memory: 15756 MB
- engine: 1.1.3
- python: Python 3.14.7
- R: R version 4.6.1 (2026-06-24) -- "Happy Hop"
- container: no
- page cache: warm
- timing precision: high
