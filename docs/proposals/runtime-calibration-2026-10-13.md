# Automatic calibration: should the product measure its own backends?

2026-10-13. Status: design note. Question from the maintainer: 自动测速各个（后端）是否都要做上 —
should the toolkit benchmark its own backends on the user's machine and route accordingly?

## What already exists

- `runtime-preferences.json` maps a capability to a preferred backend, and `--backends auto` is
  resolved from it today (the benchmark harness explicitly refuses `auto`, precisely so a
  measurement can never be silently routed).
- `scripts/benchmark/analysis/update-runtime-preferences.py` writes those preferences **by hand
  from a measured window** — the write-back we already do after each benchmark round.
- The harness itself (`benchmark.run.v1`) already produces exactly the statistics a calibration
  needs: per-backend median wall time, IQR, CV, peak RSS, N ≥ 10, consistency verdict.

So the machinery is one user-facing command away from existing. The question is whether shipping
it is a good idea.

## Arguments for shipping a `runtime.calibrate.v1`

- The right backend genuinely depends on the machine: our own data shows Rust winning wall time
  almost everywhere, but a 200× gap on `expression.pca` came from *our* implementations, and on
  a machine with a fast BLAS the Python arm could win outright. Guessing is worse than measuring.
- It converts a marketing claim ("fast") into a local, checkable one: the user's own numbers on
  the user's own machine, with the protocol identical to the published one.
- It gives the project a second dataset: calibration results from other machines are exactly the
  hardware-diversity evidence our benchmark lacks.

## Arguments against (and how each is answered)

| Concern | Answer |
| --- | --- |
| Measurement noise becomes silent mis-routing | Reuse the harness's own decision rule: a backend is only preferred when the bootstrap CI excludes 1.0, and "material" needs a CI lower bound ≥ 1.10; otherwise keep the current preference |
| A user's machine is noisy (browser, thermal) | Record the environment block, warn when CV exceeds a threshold, and never overwrite existing preferences on an inconclusive run |
| It makes results depend on invisible state | The calibration writes a *file the user can read and delete*, records `sampled_at`, N, the environment, and the report path; `--backends auto` remains inspectable |
| It could be used to launder a claim | Calibration output is explicitly local-only: reports carry `purpose: calibration`, never enter `benchmark-results/`, and are never quoted in articles |
| Cost to the user | A calibration on a small fixture is seconds per capability; full-size inputs are opt-in (`--dataset full`) |

## Recommended shape

```
linxira-bio runtime calibrate [--capabilities ...] [--dataset small|full] [--repeat N]
                              [--accept-threshold 1.10] [--dry-run] [--json]
```

- Default: small fixtures, N=5 (cheaper than the published protocol, labelled as such in the
  output), dry-run on by default **off**? No — write-preferences-on by default, because that is
  the point, but print a diff of what changed and refuse to change anything when a run is
  inconclusive.
- Always writes: `runtime-preferences.local.json` (repo-local, gitignored) rather than editing the
  shipped `runtime-preferences.json`; `--save` promotes it, and that path goes through review like
  any other number.
- Never touches the network, never installs anything (that is the provisioning question, separate
  note), never runs the medical capabilities without an explicit capability list.

## Why "all backends, automatically, always" is the wrong default

Measuring every capability's every backend on first launch would (a) be slow, (b) run analysis
code on the user's machine uninvited, and (c) produce numbers with no protocol disclosure, which
is the failure mode this project exists to avoid. Opt-in, small-fixture, CI-gated, and local-scope
gives the benefit without pretending that a hidden benchmark is a measurement.

## What would falsify this design

If calibration on three different machines picks the same backend every time, the feature is
decoration and should be dropped in favour of a documented default. That experiment is cheap (a
fixture plus three machines) and should precede any product work.
