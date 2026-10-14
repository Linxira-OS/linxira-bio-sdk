# Benchmark: sequence.stats.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-13T06:07:49Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 820.0 | 820.0 | 830.0 | 0.0 | 6.2 |
| python | 10 | 1925.0 | 1910.0 | 2080.0 | 25.0 | 43.4 |
| r | 10 | 1920.0 | 1920.0 | 2270.0 | 7.5 | 366.6 |

speedup (python/rust median wall): 2.35x
memory saving (1 - rust/python median peak RSS): 85.6%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 1591.9 | 333.1 | 43.4 |
| r | 423.2 | 1496.8 | 365.3 |

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
