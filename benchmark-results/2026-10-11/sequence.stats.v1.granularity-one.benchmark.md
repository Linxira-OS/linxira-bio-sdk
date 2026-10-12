# Benchmark: sequence.stats.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-12T00:15:29Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 810.0 | 810.0 | 890.0 | 27.5 | 5.9 |
| python | 10 | 850.0 | 840.0 | 860.0 | 7.5 | 428.9 |
| r | 10 | 1820.0 | 1810.0 | 2110.0 | 10.0 | 321.0 |

speedup (python/rust median wall): 1.05x
memory saving (1 - rust/python median peak RSS): 98.6%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 491.5 | 358.5 | 428.9 |
| r | 317.6 | 1502.4 | 319.7 |

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
