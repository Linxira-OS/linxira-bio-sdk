# Benchmark: enrichment.overrepresentation.v1

- dataset_class: annotation
- consistency: Consistent
- sampled_at: 2026-10-13T05:58:09Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 30.0 | 30.0 | 30.0 | 0.0 | 15.0 |
| python | 10 | 200.0 | 200.0 | 200.0 | 0.0 | 55.1 |
| r | 10 | 4425.0 | 4410.0 | 5110.0 | 77.5 | 145.4 |

speedup (python/rust median wall): 6.67x
memory saving (1 - rust/python median peak RSS): 72.9%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 62.3 | 137.7 | 55.1 |
| r | 4136.6 | 288.4 | 144.2 |

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
