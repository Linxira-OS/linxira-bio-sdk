# Benchmark: expression.pca.v1

- dataset_class: matrix
- consistency: Consistent
- sampled_at: 2026-10-13T05:55:13Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 20.0 | 20.0 | 20.0 | 0.0 | 8.5 |
| python | 10 | 3250.0 | 3220.0 | 3800.0 | 42.5 | 47.1 |
| r | 10 | 2745.0 | 2730.0 | 3300.0 | 150.0 | 81.8 |

speedup (python/rust median wall): 162.50x
memory saving (1 - rust/python median peak RSS): 82.0%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 3114.3 | 135.7 | 47.1 |
| r | 2507.3 | 237.7 | 80.5 |

## Environment

- code revision: f3b9f17a13ee79b5875f8ff1fc17b85050f2200b
- code branch: main
- code label: f3b9f17
- os: linux 7.2.9-1-cachyos
- distro: CachyOS
- guest cpu: Intel(R) Pentium(R) Gold G7400
- guest memory: 15756 MB
- engine: 1.1.3
- python: Python 3.12.15
- R: R version 4.6.1 (2026-06-24) -- "Happy Hop"
- container: no
- page cache: warm
- timing precision: high
