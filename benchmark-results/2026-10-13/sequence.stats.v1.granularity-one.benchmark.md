# Benchmark: sequence.stats.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-13T06:06:07Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 820.0 | 810.0 | 820.0 | 0.0 | 5.9 |
| python | 10 | 1860.0 | 1860.0 | 1990.0 | 30.0 | 422.6 |
| r | 10 | 1820.0 | 1810.0 | 2070.0 | 15.0 | 321.1 |

speedup (python/rust median wall): 2.27x
memory saving (1 - rust/python median peak RSS): 98.6%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 1529.8 | 330.2 | 422.6 |
| r | 319.2 | 1500.8 | 319.7 |

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
