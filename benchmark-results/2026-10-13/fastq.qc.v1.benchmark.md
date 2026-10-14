# Benchmark: fastq.qc.v1

- dataset_class: other
- consistency: Consistent
- sampled_at: 2026-10-13T08:25:43Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 1560.0 | 1560.0 | 1710.0 | 0.0 | 6.1 |
| python | 10 | 50910.0 | 50290.0 | 51170.0 | 490.0 | 40.7 |
| r | 10 | 699795.0 | 697870.0 | 700800.0 | 545.0 | 80.5 |

speedup (python/rust median wall): 32.63x
memory saving (1 - rust/python median peak RSS): 85.1%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 50353.8 | 556.2 | 40.5 |
| r | 699066.9 | 728.1 | 79.2 |

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
