# Benchmark: sequence.ssr.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-13T15:09:58Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 1590.0 | 1590.0 | 1890.0 | 45.0 | 20.9 |
| python | 10 | 2330.0 | 2330.0 | 2500.0 | 7.5 | 144.4 |
| r | 10 | 2200800.0 | 2199900.0 | 2202630.0 | 875.0 | 667.4 |

speedup (python/rust median wall): 1.47x
memory saving (1 - rust/python median peak RSS): 85.5%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 1994.7 | 335.3 | 144.4 |
| r | 2199172.2 | 1627.8 | 667.4 |

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
