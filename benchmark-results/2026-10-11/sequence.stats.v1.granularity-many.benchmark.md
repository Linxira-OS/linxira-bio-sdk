# Benchmark: sequence.stats.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-12T00:16:48Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 820.0 | 820.0 | 830.0 | 0.0 | 6.3 |
| python | 10 | 860.0 | 850.0 | 860.0 | 7.5 | 49.5 |
| r | 10 | 1930.0 | 1920.0 | 2040.0 | 15.0 | 366.6 |

speedup (python/rust median wall): 1.05x
memory saving (1 - rust/python median peak RSS): 87.4%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 496.0 | 364.0 | 49.5 |
| r | 424.9 | 1505.1 | 365.3 |

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
