# Benchmark: sequence.ssr.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-12T07:03:00Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 1600.0 | 1590.0 | 1850.0 | 10.0 | 21.0 |
| python | 10 | 2280.0 | 2280.0 | 2370.0 | 7.5 | 150.7 |
| r | 10 | 2211405.0 | 2209510.0 | 2214490.0 | 2472.5 | 667.5 |

speedup (python/rust median wall): 1.43x
memory saving (1 - rust/python median peak RSS): 86.1%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 1920.6 | 359.4 | 150.7 |
| r | 2209708.4 | 1696.6 | 667.5 |

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
