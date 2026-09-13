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
  `rust` is the native engine. `python` and `r` route the request to the
  matching benchmark pack (`org.linxira.benchmark-python`,
  `org.linxira.benchmark-r`), which must carry an implementation of the
  capability; a capability with no implementation in the pack fails that
  backend with a disclosed error while the other backends continue.
  `--backends auto` is rejected: a benchmark must not consult
  `runtime-preferences.json`, because preferences are derived from
  benchmarks.
- `--repeat N`: timed repetitions after one untimed warmup run (default 3).
- `--output DIR`: directory for the report files (default `benchmark-results`).
- `--dataset-class CLASS`: data-class label such as `srr`, `annotation`,
  `matrix`, `structure`, `sequence`, `sample-table`, or `other`.
- `--parameters JSON`: capability parameters passed through to the worker.
  Python/R runs additionally write into a fresh temporary output directory
  that is removed after the run; the native engine needs none.
- `--json`: print the report inside the standard result envelope.

## Outputs

- `DIR/<capability>.benchmark.json`: the benchmark report (schema
  `benchmark-report.schema.json`) with per-backend runs, medians, min/max,
  IQR, peak RSS, consistency verdict, field-level findings, and the full
  environment disclosure (OS, kernel, distro, CPU, memory, engine/Python/R
  versions, container flag, page-cache and timing-precision labels). Inside
  WSL the disclosure additionally records the Windows host: `cmd.exe /c ver`
  output, machine model, CPU model, logical processors, and physical RAM,
  each captured over the interop bridge and omitted (not guessed) when
  interop is unavailable. Each run record
  carries optional `self_reported_wall_ms` and `self_reported_peak_rss_mb`
  from the pack's in-process instrumentation, so the interpreter start-up
  share of `wall_ms` is visible.
- `DIR/<capability>.benchmark.md`: the same report as a human-readable
  summary, including a self-reported timing table.
- `speedup` is `median_wall(first non-rust backend) / median_wall(rust)`;
  `memory_saving` is `1 - median_peak_rss(rust) / median_peak_rss(first
  non-rust backend)`; both stay `null` unless both sides produced results.
  The markdown summary names the compared backend explicitly.
- With `--json`, stdout carries exactly one result envelope.

## Examples

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=tests/fixtures/sequences/tiny.fa --backends rust --repeat 5 --output benchmark-results --dataset-class sequence --json
```

```bash
linxira-bio benchmark run sequence.stats.v1 fasta=input.fa --backends rust,python,r --repeat 5 --output benchmark-results --dataset-class sequence
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
- `consistency: consistent` requires every numeric `result` field to match
  within a relative tolerance of 1e-6 across backends and every string to be
  equal; `findings` lists the exact field paths that differ. Envelope
  provenance (timestamps, versions, per-run artifacts) is excluded from the
  comparison by design — only the analysis output is judged.
- `speedup` compares the native backend named in the summary against `rust`;
  with several native backends, read each backend's row for the full picture
  (runtime preferences store per-backend numbers).

## Caveats

- Benchmarks measure one capability on one input on one machine; they do not
  generalize across data classes.
- `benchmark.run.v1` executes through the worker subprocess and inherits its
  exit contract: failures are recorded per run instead of aborting the batch.
  A backend whose pack rejects the request (stale pack manifest hashes,
  missing capability, unreadable input) yields verdict `failed` with a
  finding naming the backend.
- For Python/R backends, `wall_ms` includes interpreter start-up and pack
  verification; the self-reported columns isolate the analysis time. The
  worker hashes each input before and after the pack runs, so very large
  inputs pay extra hashing outside the analysis; this is disclosed, not
  hidden, and identical across repeats.
- Do not compare reports sampled on different machines or tool versions;
  the environment disclosure exists to prevent that. Pack library versions
  that drift from the pack lock are flagged by a warning diagnostic and
  recorded in the envelope's provenance.

## Runtime Dependencies

- The `linxira-bio-worker` binary (found via `LINXIRA_BIO_WORKER`, next to the
  CLI executable, or on `PATH`).
- `/usr/bin/time` (GNU time) on Linux for high-precision metrics; optional.
- For `--backends python`: the `org.linxira.benchmark-python` pack (worker V2
  contract), a Python 3 interpreter, and the pack's locked dependencies
  (`biopython`) installed; the pack records the versions actually used.
- For `--backends r`: the `org.linxira.benchmark-r` pack, an R installation,
  and `jsonlite`, `digest`, plus the capability's R package (for
  `sequence.stats.v1`: `Biostrings`). `LINXIRA_BIO_WORKFLOW_R_LIBRARY` is
  honoured when set; the installed library versions are disclosed. A
  project library built for another platform (for example a Windows-built
  `.linxira-bio/ci/r` tree shared with WSL) must not be auto-discovered;
  move the workflow root (`LINXIRA_BIO_WORKFLOW_ROOT`) away from the shared
  checkout or unset the library.
- No network access.

## Citations

- Benchmark methodology, consistency tolerance, and disclosure requirements:
  repository `ROADMAP.md` §7 (M2).
- Report schema: `schemas/benchmark-report.schema.json`.
- Benchmark packs: `workflows/org.linxira.benchmark-python/README.md`,
  `workflows/org.linxira.benchmark-r/README.md`.

## Troubleshooting

- `the worker binary linxira-bio-worker was not found`: install the SDK or
  point `LINXIRA_BIO_WORKER` at the binary.
- `benchmark pack org.linxira.benchmark-python has no python implementation
  of <capability>`: the pack hosts only the capabilities listed in its
  README; the other backends still ran.
- `workflow file verification failed: <path>`: the pack's `manifest.json`
  hashes no longer match the files; run
  `python scripts/update-pack-manifest.py workflows/<pack-id>` and commit.
- `workflow output parent is not a directory` or `refusing to overwrite
  workflow output directory`: the temporary output directory collided or was
  left behind by an interrupted run; re-run.
- `consistency: inconsistent`: inspect `findings` for the differing field
  paths; verify both backends consumed the same absolute input paths.
- Failed runs with empty metrics: check the truncated stderr summary in the
  run record; common causes are missing input files, missing native tools
  (for example `Biostrings` not installed for the R pack), or a library
  built for a different platform.
