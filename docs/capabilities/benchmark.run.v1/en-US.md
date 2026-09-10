# Benchmark Run

## Purpose

Execute one capability repeatedly through every registered backend on the
current machine, measure wall-time and peak memory with a disclosed
methodology (warmup run, timed repeats, median/min/max/IQR, `/usr/bin/time -v`
on Linux), and verify that all backends return numerically consistent
results. The report feeds `runtime-preferences.json`, the server benchmark
matrix, and the public benchmark page.

## Inputs

- A capability id (for example `sequence.stats.v1`).
- One or more inputs, either bound by role (`fasta=INPUT.fasta`) or as bare
  paths that are assigned to the capability's roles in order.
- Optional capability parameters as a JSON object (`--parameters`).

## Parameters

- `--backends rust,python,r`: which backends to execute (default `rust`).
  Backends without a registered benchmark pack are skipped with a warning.
- `--repeat N`: timed repetitions after one untimed warmup run (default 3).
- `--output DIR`: directory for the report files (default `benchmark-results`).
- `--dataset-class CLASS`: data-class label such as `srr`, `annotation`,
  `matrix`, `structure`, `sequence`, `sample-table`, or `other`.
- `--parameters JSON`: capability parameters passed through to the worker.
- `--json`: print the report inside the standard result envelope.

## Outputs

- `DIR/<capability>.benchmark.json`: the benchmark report (schema
  `benchmark-report.schema.json`) with per-backend runs, medians, min/max,
  IQR, peak RSS, consistency verdict, field-level findings, and the full
  environment disclosure (OS, kernel, CPU, memory, engine/Python/R versions,
  container flag, page-cache and timing-precision labels).
- `DIR/<capability>.benchmark.md`: the same report as a human-readable
  summary table.
- With `--json`, stdout carries exactly one result envelope.

## Examples

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=tests/fixtures/sequences/tiny.fa --backends rust --repeat 5 --output benchmark-results --dataset-class sequence --json
```

```bash
linxira-bio benchmark run interval.intersect.v1 left-bed=a.bed right-bed=b.bed --backends rust --repeat 3 --output benchmark-results --dataset-class annotation
```

## Interpretation

- `median_wall_ms` is the headline number; the first (warmup) run is never
  recorded, so the page cache is warm.
- Large `iqr_wall_ms` relative to the median means the machine was not quiet;
  re-run before trusting the numbers.
- `precision: high` means every timed run was wrapped in `/usr/bin/time -v`
  (wall, CPU, peak RSS, disk I/O). `precision: degraded` means an in-process
  clock was used (currently Windows) and no RSS was captured. Never mix the
  two in one comparison.
- `consistency: consistent` requires every numeric field to match within a
  relative tolerance of 1e-6 across backends and every string to be equal;
  `findings` lists the exact field paths that differ.
- `speedup` and `memory_saving` remain `null` until a native (Python/R)
  benchmark pack exists for the capability.

## Caveats

- Benchmarks measure one capability on one input on one machine; they do not
  generalize across data classes.
- `benchmark.run.v1` executes through the worker subprocess and inherits its
  exit contract: failures are recorded per run instead of aborting the batch.
- Do not compare reports sampled on different machines or tool versions;
  the environment disclosure exists to prevent that.

## Runtime Dependencies

- The `linxira-bio-worker` binary (found via `LINXIRA_BIO_WORKER`, next to the
  CLI executable, or on `PATH`).
- `/usr/bin/time` (GNU time) on Linux for high-precision metrics; optional.
- No network access.

## Citations

- Benchmark methodology, consistency tolerance, and disclosure requirements:
  repository `ROADMAP.md` §7 (M2).
- Report schema: `schemas/benchmark-report.schema.json`.

## Troubleshooting

- `the worker binary linxira-bio-worker was not found`: install the SDK or
  point `LINXIRA_BIO_WORKER` at the binary.
- `skipping backend "python": no benchmark pack is registered yet (M2-T3)`:
  the native pack for this capability is not available; only `rust` ran.
- `consistency: inconsistent`: inspect `findings` for the differing field
  paths; verify both backends consumed the same absolute input paths.
- Failed runs with empty metrics: check the truncated stderr summary in the
  run record; common causes are missing input files or missing native tools.
