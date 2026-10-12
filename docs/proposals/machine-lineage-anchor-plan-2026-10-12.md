# Machine-lineage anchor plan

2026-10-12. Status: executable now for the first two workloads; the suite-based anchors
(lzbench, Squash, PTS) are listed with their blockers.

## What this is for

Every absolute number we publish is "on this machine": 2 cores / 4 threads, 15 GiB, CPU
governor `powersave`. A reader cannot tell whether that machine is a lemon, an ordinary
budget box, or unusually fast — which makes our absolute medians uninterpretable even though
the ratios inside them are fine. An anchor fixes that: run a *standard* workload whose
published throughput for comparable CPUs exists, publish our result with the protocol, and
show that this box lands where its hardware class should.

An anchor is not a benchmark of our project. It is a ruler check.

## Acceptance criteria

1. Each workload runs on the pinned host under the same disclosure discipline as the
   benchmark window: GNU `time -v` when present, N ≥ 3 with the spread reported, governor and
   machine block recorded, raw output archived.
2. Our figure is compared against at least one **published** reference for a comparable CPU
   class, cited with its source and measurement conditions; if the reference conditions differ
   (SIMD width, compiler, dataset), the difference is stated rather than smoothed.
3. A result outside the plausible range is published as a finding about the machine, not
   re-measured until it looks right.

## Workloads

| # | Workload | Command (pinned host) | What it anchors | Blocker |
| --- | --- | --- | --- | --- |
| 1 | `zstd` single-thread compression | `zstd -19 -k -f <100 MB input>` and `zstd -3` as a fast reference, through `/usr/bin/time -v` | memory bandwidth + integer throughput on a real 100 MB stream; zstd's own published throughput tables give the reference | none — `zstd` ships with the distro |
| 2 | `openssl speed` hash/cipher | `openssl speed -seconds 3 sha256` (and `-evp aes-128-gcm`) | per-cycle crypto throughput, the most widely published CPU micro-benchmark family | none — `openssl` ships with the distro |
| 3 | `lzbench` on the Silesia corpus | build `lzbench`, unpack Silesia, run its standard sweep | the compression lineage our M5 work touches (and the one our claims about I/O bound work sit next to) | fetching lzbench and Silesia from GitHub mirrors is unreliable from the measurement host (release downloads stall/reset); retry, mirror, or build from a checkout |
| 4 | Squash compression benchmark | its own harness over its dataset set | published cross-tool compression ratios/speeds | same network blocker; also a large dataset fetch |
| 5 | Phoronix Test Suite `pts/compression` + `pts/bioinformatics` | `phoronix-test-suite benchmark pts/compression pts/bioinformatics` | a public suite result we can upload to OpenBenchmarking, giving the machine a named, checkable lineage | PTS is packaged on Arch (AUR) but pulls its own subsystems; needs a window of its own, and the upload is a public action requiring the maintainer's approval |

Workloads 1 and 2 are the cheap half of the anchor and can run today; 3–5 are the
public-comparison half.

## How the results get published

`benchmark-results/<date>/machine-anchor/` with one record per workload: the raw tool output,
the parsed metrics, the machine block, and the cited published reference with its URL. The
article gains one paragraph stating that the machine's class was checked and pointing at the
record; no claim about our project's performance is derived from an anchor.

## Why not just rent a bigger machine

Because the anchor's purpose is to describe *this* machine. A rented machine answers a
different question ("what would better hardware give"), which is the M5 direction work, not
this.
