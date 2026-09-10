---
name: run-bio-benchmark
description: Benchmark one Linxira Bio capability across execution backends with the local `benchmark.run.v1` capability. Use when comparing the Rust implementation against native Python or R packs, measuring wall-time medians and peak RSS, or verifying cross-backend numerical consistency on the same input.
---

# Run Bio Benchmark

Measure speed and memory of one capability on one machine, and verify that
every backend returns numerically identical results.

## Run

1. Confirm the input exists and matches the capability's expected format with
   `inspect-bio-dataset`.
2. Run the installed CLI (warmup plus timed repeats are executed per backend):

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=INPUT.fasta --backends rust --repeat 3 --output benchmark-results --dataset-class sequence --json
```

When developing inside the source repository, run:

```bash
cargo run -p linxira-bio-cli -- benchmark run sequence.stats.v1 fasta=INPUT.fasta --backends rust --repeat 3 --output benchmark-results --dataset-class sequence --json
```

3. Read the written report
   (`benchmark-results/<capability>.benchmark.json` and `.benchmark.md`).
4. Reject an error result; failed runs are recorded inside the report with
   their stderr summary (truncated to 4 KiB) instead of failing the batch.

## Interpret

- `median_wall_ms` is the headline number; repeats follow one untimed warmup
  run, so the page cache is warm.
- `min_wall_ms`, `max_wall_ms`, and `iqr_wall_ms` expose timing spread; a
  large IQR means the machine was not quiet.
- `median_peak_rss_mb` comes from `/usr/bin/time -v` on Linux
  (`precision: high`). Windows reports `precision: degraded` wall times from
  an in-process clock and no RSS.
- `consistency` compares result envelopes across backends: numeric fields
  must match within a relative tolerance of 1e-6, strings must be equal.
  `findings` lists every differing field path.
- `speedup` and `memory_saving` stay `null` until a native benchmark pack
  (Python/R) exists for the capability.

## Caveats

- Never mix `precision: high` and `precision: degraded` numbers in one
  comparison.
- One capability per `benchmark run`; use `scripts/run-benchmark-linux.sh`
  for the full server matrix.
- Backends without a registered benchmark pack are skipped with a warning,
  not fabricated.
