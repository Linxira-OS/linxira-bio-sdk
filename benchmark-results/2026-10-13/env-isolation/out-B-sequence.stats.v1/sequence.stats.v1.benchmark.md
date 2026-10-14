# Benchmark: sequence.stats.v1

- dataset_class: sequence
- consistency: Consistent
- sampled_at: 2026-10-14T01:52:36Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| python | 10 | 1840.0 | 1820.0 | 1960.0 | 30.0 | 64.7 |

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 1504.1 | 335.9 | 64.7 |

## Environment

- code revision: d8d60fd49565908d68279205210d87c14a4b5520
- code branch: main
- code label: d8d60fd
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
