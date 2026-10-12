# Benchmark: structure.pdb.summary.v1

- dataset_class: structure
- consistency: Consistent
- sampled_at: 2026-10-12T00:12:11Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 30.0 | 30.0 | 30.0 | 0.0 | 28.4 |
| python | 10 | 780.0 | 770.0 | 970.0 | 95.0 | 66.3 |
| r | 10 | 20680.0 | 20150.0 | 20850.0 | 220.0 | 162.8 |

speedup (python/rust median wall): 26.00x
memory saving (1 - rust/python median peak RSS): 57.1%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 83.0 | 697.0 | 62.7 |
| r | 5352.0 | 15328.0 | 119.2 |

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
