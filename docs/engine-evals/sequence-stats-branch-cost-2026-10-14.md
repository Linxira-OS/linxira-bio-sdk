# `sequence.stats` rust leg: where 810 ms went

<!-- 发布前检查 / pre-publication check: not applicable — repository-internal engine ledger (docs/engine-evals/ is not staged into the release bundle). -->

Measurement date: 2026-10-14. Machine: Intel Pentium Gold G7400, 2 cores / 4 threads,
governor `powersave`, input on tmpfs, page cache warm. Baseline: the rust leg of
`sequence.stats.v1` in archive `bench-20261013-001` — **810 ms** (N=10, `median_cpu_ms`
810, peak RSS 5.9 MB) on a 101,667,131-byte FASTA holding 20 records × 5,000,000 bases.

The question was whether 810 ms for a scan that `pyfastx stat` does in 10 ms and
`seqkit stats` in 30 ms is our algorithm, the toolchain, or a step in the pipeline.
It was none of the first two, and there was not one step but three.

## 1. The file is 60-column wrapped, which pointed the wrong way first

`scripts/benchmark/inputs/generate-inputs.py` writes FASTA at 60 bases per line, so the
input has **1,666,700 lines**. `fasta_stats` read it with `reader.lines()`, which
allocates a `String` per line, so ~1.67 M allocations looked like the obvious cost.
Measuring instead of assuming cut that term down: in one session, unwrapping the same
bases to 21 lines moved the in-process CLI path from 0.66 s to 0.59 s, i.e. tens of
milliseconds, while §3 puts the classification term at ~640 ms. The allocations are
real but small, and they are still there after the fix (the CLI path reads 0.14 s with
allocating `lines()`, matching the allocation-free arms in §3). Note that readings from
different sessions on this host carry a spread of roughly 10% under the `powersave`
governor, which is why §3's table comes from one session with three runs per arm.

## 2. What the code actually does per base

the core crate's `crates/linxira-bio-core/src/sequence.rs` scanned with:

```rust
for base in trimmed.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
    match base.to_ascii_uppercase() {
        b'G' | b'C' => { gc_bases += 1; canonical_bases += 1; }
        b'A' | b'T' | b'U' => canonical_bases += 1,
        b'N' => n_count += 1,
        _ => {}
    }
}
```

On random sequence that branch structure is unpredictable at every base.

## 3. The discriminating experiment

`scripts/benchmark/analysis/profile-sequence-scan.rs` (std-only; run it with
`rustc -C opt-level=3 profile-sequence-scan.rs -o scan && ./scan <fasta>`) reads the same
file four ways. Three files with the same 60-column line structure and the same size to
within 441 bytes differ only in which bases they contain: random ACGT (the real input),
all `A`, and a cyclic `ACGT` repeat.

| Variant | random ACGT | all A | ACGT cycle |
| --- | ---: | ---: | ---: |
| read-only, 1 MiB chunks | 9.6 ms | 10.3 ms | 8.7 ms |
| `lines()` + branchy `match` (the code above) | **642.3 ms** | 168.2 ms | 184.3 ms |
| `lines()` + 256-entry class table | **124.4 ms** | 124.7 ms | 122.3 ms |
| 1 MiB chunks + class table | 123.3 ms | 123.9 ms | 122.8 ms |

(Numbers from the committed probe compiled exactly as section 7 documents, `rustc -C opt-level=3` and three runs per arm. An earlier draft of this table came from a
cargo `release` build of the same probe; its `lines()`+table arm read 139.4 ms against
124.4 ms here, while its chunked arm moved by under 1 ms. That build differed in build
system as well as flags, so the difference is not attributed to any single one - the
recipe now names its compiler settings, which is what matters for re-running it. Between
sessions under the `powersave` governor the fixed arms move by up to ~10%; the branchy
arm's gap over them does not.)

Two things follow. I/O is ~10 ms, i.e. never the bottleneck. And on the same random file
the branchy loop costs **~518 ms more than the branchless scan** (642.3 vs 124.4 ms),
while on the ACGT-cycle file - which takes the same G/C branch half the time - it costs
**~62 ms more than that file's branchless scan** (184.3 vs 122.3 ms) yet stays 3.5x above
it. Same arithmetic, different predictability: over 100 M bases, ~518 ms is ~5.2 ns
(about 15 cycles at 3 GHz) of misprediction per base, while the pre-fix pipeline as a
whole ran at 8.1 ns per base.

## 4. Where the rest of the 810 ms sat: the benchmark pipeline reads the input three times

The archived number does not come from the CLI subcommand. `benchmark run` writes a
**schema-v2 job** and spawns the worker under `/usr/bin/time -v` (which is why every
reading in the archive lands on a 10 ms boundary). The v2 worker path then touches the
input three times per timed repetition:

1. `validate_v2_inputs` — full-file SHA-256 (the worker crate's `lib.rs:2983`),
2. the one-pass scan (the worker's `lib.rs:1542` → `fasta_stats_path`),
3. `finalize_v2_input_hashes` — full-file SHA-256 again (the worker's `lib.rs:3160`), recorded as
   provenance.

Measured `sha2` cost per pass: 66 ms; `sha256sum` on the same file: 60 ms.

Two terms are direct measurements: the in-process CLI path is **0.66 s** (§5, which
already contains the ~10 ms read and the JSON) and the two SHA-256 passes are **~130 ms**.
The third - spawning the worker and writing its request - is below this timer's
resolution: the same pipeline on a 20-base input reports **0.0 ms** median over 10 runs,
i.e. under the 10 ms step of GNU `time -v`. The terms therefore account for ~790 ms of the
810 ms observed, the remainder sitting inside the ~10% session spread noted in section 1.
No term here is a computed residual.

## 5. The change, and what it measured

`fasta_stats` now classifies with a 256-byte table built at compile time (bit 0
canonical, bit 1 G/C, bit 2 N, bit 3 counted-as-sequence-byte) and accumulates
branchlessly. The table is built with `to_ascii_uppercase`, so case equivalence is
structural rather than a second hand-written mapping.

| Measurement (same machine, same input, N=10) | before | after |
| --- | ---: | ---: |
| in-process CLI path (`linxira-bio sequence stats big.fa --json`) | 0.66 s | **0.14 s** |
| same loop on an all-`A` file built to the same size (441 bytes apart) | 0.19 s | 0.14 s |
| **benchmark pipeline** (`benchmark run … --backends rust`) | 810 ms | **280 ms** |
| rust-vs-rust run spread in the pipeline | 810-820 ms | 280 ms flat |

Equivalence evidence: the stats JSON for the real 101.7 MB input is **byte-identical**
before and after; `cargo test --workspace` is green; three added tests pin the
case-folding, the interior-whitespace rule and the gap-character rule.

The same three terms account for the post-fix 280 ms: the CLI path is **0.14 s** (the
table loop at ~124 ms whether it reads lines or 1 MiB chunks, so the reader is worth
nothing measurable and not worth a rewrite), the two SHA-256 passes are still ~130 ms,
and the fixed term stays below the timer's resolution (0.0 ms on the 20-base input) - so
the three terms sit inside the 280 ms measured with min = max across 10 runs. The misprediction term is the
one that vanished; the hashing term is now the largest single remaining cost.

## 6. What is left, and it is a decision rather than a bug fix

- **The harness hashes the input twice per repetition.** That is 130 ms of every
  measured run, spent on provenance rather than on the capability. One pass is
  defensible; the second is not obviously so. Changing it changes what a report's
  timed window covers, so it needs a decision and a new window, not a quiet edit.
- **A ~100 ms scalar pass is not the floor.** `seqkit stats` reads the same file in
  30 ms, which is word-at-a-time or vectorized counting (its `bytes.Count`-class
  helpers), not a per-byte scalar loop. Closing that gap means a SIMD or multi-pass
  counting path — a separate design decision, and per the repository's capability
  ownership rule it is also legitimate to conclude that this job should call the tool.
- **The same branchy classifier exists elsewhere.** `fastq.rs::add_base` (called per
  base, with `matches!` plus checked adds) is the same shape, and `fastq.qc`'s rust leg
  sits at the same throughput class (217 MB in 1560 ms). Registered as follow-up work.

## 7. Reproducing this record

```bash
# the scan probe: same machine, any FASTA; std-only, no repository build needed
rustc -C opt-level=3 scripts/benchmark/analysis/profile-sequence-scan.rs -o /tmp/scan && /tmp/scan <fasta>

# the same-size controls that separate predictability from work
awk 'BEGIN{print ">a"; for(i=0;i<1666667;i++) print "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}' > /tmp/all-A.fa
awk 'BEGIN{print ">p"; row="ACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGTACGT"; for(i=0;i<1666667;i++) print row}' > /tmp/cycle.fa
sha256sum /tmp/probe/big.fa   # one hash pass, for comparison with the sha2 row above

# regenerate a benchmark-identical input (60-column, seeded)
python3 scripts/benchmark/inputs/generate-inputs.py --set sequence --out /tmp/probe --binary target/release/linxira-bio

# the two pipeline readings
target/release/linxira-bio benchmark run sequence.stats.v1 "fasta=/tmp/probe/big.fa" \
  --backends rust --repeat 10 --output /tmp/probe/bench --dataset-class sequence --json
```

Numbers in this ledger are single-window readings on one host, not a published window.
The archive `bench-20261013-001` keeps the pre-fix numbers as they were measured; new
numbers require a new window.
