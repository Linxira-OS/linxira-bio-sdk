# Benchmark: expression.pca.v1

- dataset_class: matrix
- consistency: Consistent
- sampled_at: 2026-10-14T01:52:14Z
- precision: high (page_cache: warm)

| backend | repeats | median wall (ms) | min | max | IQR | median RSS (MB) |
|---|---|---|---|---|---|---|
| python | 10 | 3250.0 | 3200.0 | 3950.0 | 75.0 | 47.3 |

## Self-reported (in-process) timing

| backend | median analysis wall (ms) | start-up overhead (ms) | median self peak RSS (MB) |
|---|---|---|---|
| python | 3111.6 | 138.4 | 47.3 |

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
