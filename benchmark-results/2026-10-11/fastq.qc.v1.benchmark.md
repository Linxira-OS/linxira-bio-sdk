# Benchmark: fastq.qc.v1

- dataset_class: other
- consistency: Consistent
- sampled_at: 2026-10-12T09:22:06Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| rust | 10 | 1570.0 | 1570.0 | 1580.0 | 10.0 | 6.2 |
| python | 10 | 49345.0 | 48680.0 | 50610.0 | 745.0 | 46.0 |
| r | 10 | 702190.0 | 700770.0 | 702600.0 | 720.0 | 79.4 |

speedup (python/rust median wall): 31.43x
memory saving (1 - rust/python median peak RSS): 86.6%

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 48753.2 | 591.8 | 45.8 |
| r | 701454.1 | 735.9 | 78.3 |

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
