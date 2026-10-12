# Benchmark: variant.stats.v1

- dataset_class: other
- consistency: Consistent
- sampled_at: 2026-10-12T00:14:49Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 80.0 | 80.0 | 80.0 | 0.0 | 6.2 |
| python | 10 | 760.0 | 750.0 | 770.0 | 0.0 | 45.9 |
| r | 10 | 13500.0 | 13110.0 | 13950.0 | 737.5 | 122.3 |

speedup (python/rust median wall): 9.50x
memory saving (1 - rust/python median peak RSS): 86.5%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 582.6 | 177.4 | 45.8 |
| r | 13262.9 | 237.1 | 121.1 |

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
