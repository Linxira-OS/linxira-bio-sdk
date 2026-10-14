# Benchmark: set.venn.v1

- dataset_class: other
- consistency: Consistent
- sampled_at: 2026-10-13T05:57:16Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 10.0 | 10.0 | 10.0 | 0.0 | 8.3 |
| python | 10 | 150.0 | 150.0 | 150.0 | 0.0 | 40.9 |
| r | 10 | 11035.0 | 10870.0 | 11310.0 | 272.5 | 124.7 |

speedup (python/rust median wall): 15.00x
memory saving (1 - rust/python median peak RSS): 79.8%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 19.9 | 130.1 | 40.9 |
| r | 10809.4 | 225.6 | 123.5 |

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
