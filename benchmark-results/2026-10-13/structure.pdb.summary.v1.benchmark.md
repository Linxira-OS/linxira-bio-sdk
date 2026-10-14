# Benchmark: structure.pdb.summary.v1

- dataset_class: structure
- consistency: Consistent
- sampled_at: 2026-10-13T06:02:04Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 30.0 | 30.0 | 30.0 | 0.0 | 29.7 |
| python | 10 | 700.0 | 690.0 | 700.0 | 10.0 | 66.4 |
| r | 10 | 20555.0 | 19980.0 | 20690.0 | 135.0 | 162.8 |

speedup (python/rust median wall): 23.33x
memory saving (1 - rust/python median peak RSS): 55.2%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 76.0 | 624.0 | 56.6 |
| r | 5311.6 | 15243.4 | 119.2 |

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
