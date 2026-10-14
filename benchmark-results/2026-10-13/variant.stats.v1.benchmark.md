# Benchmark: variant.stats.v1

- dataset_class: other
- consistency: Consistent
- sampled_at: 2026-10-13T06:04:41Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 80.0 | 80.0 | 80.0 | 0.0 | 6.2 |
| python | 10 | 710.0 | 710.0 | 720.0 | 10.0 | 39.7 |
| r | 10 | 13455.0 | 13010.0 | 13890.0 | 755.0 | 122.3 |

speedup (python/rust median wall): 8.88x
memory saving (1 - rust/python median peak RSS): 84.4%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 561.3 | 148.7 | 39.4 |
| r | 13206.1 | 248.9 | 121.1 |

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
