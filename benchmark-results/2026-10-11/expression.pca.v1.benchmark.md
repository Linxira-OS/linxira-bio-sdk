# Benchmark: expression.pca.v1

- dataset_class: matrix
- consistency: Consistent
- sampled_at: 2026-10-12T00:05:15Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 20.0 | 20.0 | 30.0 | 10.0 | 8.5 |
| python | 10 | 4010.0 | 3980.0 | 4620.0 | 140.0 | 53.4 |
| r | 10 | 2770.0 | 2760.0 | 3280.0 | 15.0 | 81.9 |

speedup (python/rust median wall): 200.50x
memory saving (1 - rust/python median peak RSS): 84.0%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 3845.9 | 164.1 | 53.4 |
| r | 2530.5 | 239.5 | 80.5 |

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
