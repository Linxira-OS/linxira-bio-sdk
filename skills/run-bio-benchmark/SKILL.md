---
name: run-bio-benchmark
description: Benchmark one Linxira Bio capability across execution backends with the local `benchmark.run.v1` capability. Use when comparing the Rust engine against the native Python (Biopython) or R (Biostrings) packs, measuring wall-time medians and peak RSS, reading self-reported in-process timing, or verifying cross-backend numerical consistency on the same input.
---

# Run Bio Benchmark

Measure speed and memory of one capability on one machine, and verify that
every backend returns numerically identical results.

## Run

1. Confirm the input exists and matches the capability's expected format with
   `inspect-bio-dataset`.
2. Run the installed CLI (warmup plus timed repeats are executed per backend):

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=INPUT.fasta --backends rust,python,r --repeat 3 --output benchmark-results --dataset-class sequence --json
```

When developing inside the source repository, run:

```bash
cargo run -p linxira-bio-cli -- benchmark run sequence.stats.v1 fasta=INPUT.fasta --backends rust,python,r --repeat 3 --output benchmark-results --dataset-class sequence --json
```

- `rust` is the native engine. `python` and `r` execute the benchmark packs
  (`workflows/org.linxira.benchmark-python`, `org.linxira.benchmark-r`), which
  host the independent Biopython/Biostrings implementations; a capability the
  pack does not implement fails that backend with a disclosed error while the
  others continue.
- `--backends auto` is rejected on purpose: benchmarks must not consult
  `runtime-preferences.json`, which is derived from them.

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
- For python/r runs, `wall_ms` includes interpreter start-up; the run's
  `self_reported_wall_ms` (and `self_reported_peak_rss_mb`) isolate the
  analysis itself, and the markdown summary shows the gap as start-up
  overhead.
- `consistency` compares the `result` objects across backends: numeric fields
  must match within a relative tolerance of 1e-6, strings must be equal.
  Provenance and per-run artifacts are excluded by design. `findings` lists
  every differing field path.
- `speedup` = first non-rust backend's median wall / rust's; `memory_saving`
  = 1 - rust peak RSS / native peak RSS. The summary names the compared
  backend.

## Caveats

- Never mix `precision: high` and `precision: degraded` numbers in one
  comparison.
- One capability per `benchmark run`; use `scripts/run-benchmark-linux.sh`
  for the full server matrix.
- A backend that never produces a result makes the whole report
  `consistency: failed` with a finding naming it — fix the pack or drop the
  backend; numbers are never fabricated.
- If a pack reports `workflow file verification failed`, its manifest hashes
  are stale: run `python scripts/update-pack-manifest.py workflows/<pack-id>`
  and commit.
- R backends resolve packages from the benchmark host's R library (a project
  library via `LINXIRA_BIO_WORKFLOW_R_LIBRARY` is honoured); versions that
  drift from the pack lock produce a warning diagnostic recorded in the
  envelope. A library built for another platform must not be auto-discovered
  — see the capability docs.
