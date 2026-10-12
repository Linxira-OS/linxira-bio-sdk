# Benchmark: set.venn.v1

- dataset_class: other
- consistency: Consistent
- sampled_at: 2026-10-12T00:07:20Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 10.0 | 10.0 | 10.0 | 0.0 | 8.3 |
| python | 10 | 180.0 | 180.0 | 190.0 | 7.5 | 47.3 |
| r | 10 | 11075.0 | 10860.0 | 11340.0 | 322.5 | 124.7 |

speedup (python/rust median wall): 18.00x
memory saving (1 - rust/python median peak RSS): 82.5%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 20.3 | 159.7 | 47.3 |
| r | 10851.1 | 223.9 | 123.5 |

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
