# Benchmark: enrichment.overrepresentation.v1

- dataset_class: annotation
- consistency: Consistent
- sampled_at: 2026-10-12T00:08:13Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 30.0 | 30.0 | 30.0 | 0.0 | 15.0 |
| python | 10 | 230.0 | 220.0 | 230.0 | 10.0 | 61.0 |
| r | 10 | 4480.0 | 4440.0 | 5180.0 | 187.5 | 145.4 |

speedup (python/rust median wall): 7.67x
memory saving (1 - rust/python median peak RSS): 75.4%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 57.7 | 172.3 | 61.0 |
| r | 4180.4 | 299.6 | 144.1 |

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
