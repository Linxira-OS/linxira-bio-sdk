---
title: "Bandwidth Twins: A Fair Cross-Vendor Comparison of the RTX 4080 SUPER and Moore Threads MTT S4000 on Rented Hardware"
date: 2026-10-02
tag: technical
lang: en
desc: "Linxira Bio SDK cloud-rental validation: a single-source dual-compiler kernel suite confirms the bandwidth-twin pairing (STREAM-triad 666 vs 711 GB/s, 90%/93% of spec); a 36-point docking sweep reproduces value-for-value across vendors with exhaustiveness>=32 converging within 0.07 A of the official pose; five classes of analysis outputs hash-identical across vendor CPUs; storage, RAM, and toolchain ecosystem dimensions fully disclosed."
---

# Bandwidth Twins: A Fair Cross-Vendor Comparison of the RTX 4080 SUPER and Moore Threads MTT S4000 on Rented Hardware

> Linxira Bio SDK cloud-rental validation report · 2026-10-02. One NVIDIA vGPU
> (RTX 4080 SUPER, modified, 32 GB) and one Moore Threads MTT S4000 (48 GB),
> rented in the same window, built from the same repository clone on-site, fed
> the same data and seeds. Environment, commands, and boundaries fully
> disclosed; reproduction scripts ship with the repository.

## TL;DR

- **The bandwidth-twin pairing holds**: in the single-source dual-compiler
  kernel suite, STREAM-triad measures 665.7 vs 711.2 GB/s (spec 736/768 GB/s;
  90.2%/92.8% efficiency) — for bandwidth-bound kernels these two cards are a
  fair controlled pair (§3.1).
- **36 docking runs reproduce value-for-value across vendors**: the
  RMSD-to-reference table for 12 (exhaustiveness, seed) points is identical on
  both vendors' CPUs; at exhaustiveness ≥ 32 every seed converges within
  0.07 Å of the official pose, while e = 8 loses the global minimum 1-in-3
  seeds (RMSD 12.4 Å) (§3.2).
- **Five classes of analysis outputs are SHA256-identical across vendors**:
  docking JSONs, SSR TSVs, same-seed docking reruns, the five-step expression
  chain (QC/normalize/PCA/clustering), and GO-enrichment JSONs — the
  determinism design holds across the stack, not just one analysis type
  (§3.4).
- **Ecosystem observations (neutral, evidence attached)**: Moore Threads'
  vendor compiler mcc fired on the first try; the NVIDIA community frontier
  Rust stack (cutile 0.0.0-alpha) traces to NVIDIA's experimental TILE
  compiler lineage (LLVM 21 + MLIR required) and was unusable this round.
  Vulkan is blocked on both vendors at different layers, each with verbatim
  evidence (§3.5).

## Changelog

- **2026-10-02** Initial release.

## 1 Background and question

Comparing GPUs from two vendors first demands an answer to **which pairing
axis is fair**. Pairing by compute (TFLOPS) conflates architecture
differences; pairing by price chases market noise. Our GPU workloads are
dominated by low-arithmetic-intensity kernels (a few FMAs per element, large
streaming traffic), which run under the memory-bandwidth roof of the roofline
model [1]. This round therefore uses **bandwidth-twin pairing**: the RTX 4080
SUPER (736 GB/s spec) against the MTT S4000 (768 GB/s spec, 4.2% apart) — a
controlled single-variable comparison for bandwidth-bound kernels. The cards
differ ~2× in peak compute; under the bandwidth roof that gap must not turn
into a throughput gap. That is the hypothesis under test.

The second question is **reproducibility**. A local-first SDK that claims
determinism should not carry evidence from a single vendor's machines only.
This round takes five classes of SDK outputs (including orchestrated native
tool output) to both vendors' CPUs/GPUs for bit-level comparison.

## 2 Environment and methods

### 2.1 Machines (cloud-rental container instances, same-day window)

| | Instance A (NVIDIA) | Instance B (Moore Threads) |
| --- | --- | --- |
| GPU | RTX 4080 SUPER, 32 GB (modified vGPU card), driver 595.71.05, sm_89 | MTT S4000, 48 GB, MUSA driver 2.7.0 |
| GPU compiler | nvcc 11.8 (V11.8.89) | mcc (clang-14 derivative, MUSA toolkit 3.1.0) |
| CPU | Xeon Platinum 8375C @2.9GHz, 128 vCPUs | Xeon Gold 6430, 15 cores |
| Kernel | 5.15.0-25-generic | 5.15.0-105-generic |
| SDK | linxira-bio 1.1.1, on-site `cargo build --release` after clone (rsproxy mirror) | same (build 47.3 s, 59 crates) |
| Docking tool | AutoDock Vina 1.2.5 (official release binary, probed by name, never bundled) | same |

### 2.2 Method contract

- **Single source, dual compile**: the GPU kernel suite is one C source file
  (`scripts/rental/bench_suite.mu`) compiled by `mcc -O3` (MUSA) and
  `nvcc -O3 -arch=sm_89` (CUDA); only the file extension differs.
- **Timing**: device events, median of 3; every kernel carries a correctness
  gate (copy/triad bit-level, histogram exact 256-bin equality, Pearson
  relative error < 2e-4 against an f64 reference).
- **Docking**: the official basic_docking example (HIV-1 protease / indinavir,
  PDB 1iep [3]); sweep of exhaustiveness {8,16,32,64} × seeds {42,43,44} plus
  a thread sweep; metrics are binding affinity, heavy-atom in-place RMSD to
  the official reference pose, and wall clock. Vina methodology per [2,4].
- **Analysis chain**: a simulated expression profile (2000 genes × 12 samples,
  150 planted DE genes and one planted enriched term, seed 4242) flows through
  `expression matrix-qc → normalize (median-ratio) [6] → pca → cluster →
  enrichment go` (hypergeometric test [7]).
- Cross-checked datasets and tools are identical by construction (the
  repository rental bundle, SHA256-verified).

## 3 Results

### 3.1 GPU kernel suite: the bandwidth-twin test

Same source, both vendor compilers, all correctness gates passed:

| Kernel | 4080 SUPER (nvcc) | MTT S4000 (mcc) | ratio |
| --- | --- | --- | --- |
| copy 1 GiB (read+write) | **629.8 GB/s** | 473.8 GB/s | 1.33× |
| STREAM-triad (2r+1w) | 665.7 GB/s | **711.2 GB/s** | 0.94× |
| 256-bin histogram (shared+atomic) | 26.6 GB/s | 6.5 GB/s | 4.1× |
| Pearson v2 4000×1000 (499,500 pairs) | 281.9 GB/s | 49.0 GB/s | 5.75× |

**Verdict**: on triad — the closest to the STREAM convention [5] — both cards
sit in the same efficiency band (90.2% vs 92.8%), confirming the twin
hypothesis. The texture is interesting: unidirectional copy favors NVIDIA by
33% (write path / L2 policy) while triad tips to Moore Threads. Atomic
throughput and compiler maturity gaps (4–5.8×) are current MUSA weaknesses,
consistent with our per-card strategy rule — no pre-declared winners. The
Pearson kernel is a first non-coalesced version (equally penalized on both
cards); the ratio is fair, the absolute numbers are not peak bandwidth.

### 3.2 Molecular docking: convergence + cross-vendor determinism

**Convergence** (identical on both machines; RMSD to the official reference
pose, Å):

| exhaustiveness | seed 42 | seed 43 | seed 44 |
| --- | --- | --- | --- |
| 8 | 0.058 | 0.861 | **12.42 (-10.9, global minimum lost)** |
| 16 | 0.058 | 0.861 | 0.015 |
| 32 | 0.062 | 0.068 | 0.015 |
| 64 | 0.056 | 0.047 | 0.045 |

Practical reading: default e = 8 is fine for fast screening, but
**pose-defining conclusions need e ≥ 32**. Thread scaling saturates at 8
threads (72 s → 12.7 s, then flat) — Vina's e = 8 is exactly 8 independent
MC chains, so 8 is the physical limit [2].

**Cross-vendor determinism**: the RMSD and affinity tables for all 12
(e, seed) points are **value-for-value identical** on the two vendors' CPUs
(failed seed included); same-seed reruns produce byte-identical output files.
Given seed and exhaustiveness, Vina's result is independent of CPU vendor and
core count.

### 3.3 Storage and RAM dimensions

| Dimension | Instance A | Instance B |
| --- | --- | --- |
| Data-disk sequential write (dd conv=fdatasync, 2 GiB) | 505 MB/s | **677 MB/s** |
| Data-disk read (page-cache figure†) | 5.4 GB/s | 7.4 GB/s |
| RAM STREAM triad (single thread) | **14.9 GB/s** | 13.5 GB/s |
| RAM STREAM triad (all cores, OpenMP) | 14.6 GB/s | 13.3 GB/s |

† The container may not drop caches, so reads reflect the cache; the flat
OpenMP scaling (128 threads ≈ single thread) is a cloud per-instance memory
bandwidth cap — an **instance property, not a vendor CPU comparison**,
corroborated by the docking wall clocks (instance B's 15 physical cores beat
instance A's 128 vCPUs at e = 64; suspected oversubscription).

### 3.4 Analysis chain and five classes of bit-level reproduction

The simulated expression profile ran end-to-end on both machines with
**SHA256-identical result JSONs for all five steps**: QC (2000×12) →
median-ratio normalization → PCA (4 components) → two-way clustering → GO
hypergeometric enrichment. The planted term is recovered at the top with
**p = 6.3e-31 (44/80 genes)** while background terms sit at p > 0.03 — a
closed signal-injection/recovery validation.

Together with the earlier rounds this makes **five classes of cross-vendor
bit-level reproduction**: docking result JSONs, SSR three-backend TSVs
(1 M bases), same-seed docking reruns, the five-step expression chain, and
enrichment JSONs. The SSR rust backend holds 3.6–4.0× over pytrf across three
CPUs (15/16/128 cores) with byte-identical output.

### 3.5 Ecosystem observations (neutral, evidence attached)

- **Moore Threads' mcc works out of the box**: CUDA-style source compiled by
  `mcc x.mu -lmusart` fired on the first serious attempt (three micro
  iterations: no `--musarchs` flag — nvcc-style defaults; explicit
  `-lmusart` needed). The `musa_runtime` API is CUDA-shaped, and the
  prerequisite for a Rust-side FFI wrapper was validated on the spot.
- **The NVIDIA community frontier Rust stack was unusable this round**: the
  crates.io `cutile 0.0.0-alpha` tree contains a yanked `cuda-bindings` (no
  repository field); after assembling a CUDA 13 pip-wheel toolchain and
  headers, the final blocker was `mlir-sys`/`tblgen` requiring LLVM 21 +
  MLIR (absent on Ubuntu 22.04; fetching the 144 MB toolchain over the
  rented network did not complete inside the window). Tracing the tree shows
  `cuda-bindings`' repository field pointing at nvidia/tile-rust with
  `cuda-tile-rs`/`cutile-compiler` in the graph — **it is a community
  snapshot of NVIDIA's experimental TILE compiler stack, not a thin CUDA
  FFI**. Conclusion: thin-FFI duty goes to a mature route; the TILE stack
  waits for its hardware generation.
- **Vulkan (the wgpu route) is blocked on both vendors, at different layers,
  with three layers of evidence archived**: the NVIDIA vGPU guest driver
  595.71.05 exposes the negotiate symbol but no `vkCreateInstance` entry
  point (three independent instances reproduce the same loader error
  verbatim); Moore Threads' `libVK_MT.so` loads, negotiates, opens the device
  node, and completes its ioctls, then fails `vkCreateInstance` inside the
  driver (loader/strace evidence archived; suspected userspace toolkit 3.1.0
  vs host kernel driver 2.7.0 mismatch). Both are **driver-stack facts**, not
  SDK defects; neither extrapolates to retail cards.

## 4 Honest boundaries

1. **Instance properties ≠ vendor specifications**: rented vCPUs, memory
   bandwidth, and storage are subject to host load and oversubscription;
   wall clocks are recorded as instance observations only. Cross-vendor
   conclusions rest solely on same-source kernel ratios and bit-level
   identity.
2. **Read figures are page-cache numbers**; dd is a sequential large-block
   figure, not fio 4k-random.
3. The Pearson kernel is a non-coalesced first version; absolute numbers are
   not peak bandwidth.
4. One instance per card, one rental window; **no cross-architecture
   extrapolation** (to other GPU models or retail drivers).
5. The e = 8 failure mode (§3.2) is an inherent property of the Monte Carlo
   search budget, not a defect; the numbers hold for the official 1iep
   example.

## 5 Reproduction

Reproduction scripts and kernel sources ship with the repository
(`scripts/rental/`; visible in-repo, never bundled into any release):

```bash
git clone https://github.com/Linxira-OS/linxira-bio-sdk.git
cd linxira-bio-sdk && cargo build --release -p linxira-bio-cli

# GPU kernel suite (single source, dual compile)
mcc -O3 scripts/rental/bench_suite.mu -o bench_mu -lmusart          # MUSA
cp scripts/rental/bench_suite.mu b.cu && nvcc -O3 -arch=sm_89 b.cu -o bench_cu  # CUDA
./bench_mu   # median-of-3 + correctness gate per kernel; exit 0 iff all pass

# Docking study (run once per machine, then compare rmsd_vs_official.tsv)
bash scripts/rental/dock-study.sh         # 128-core parameters
bash scripts/rental/dock-study-s4000.sh   # 15-core parameters
python3 scripts/rental/analyze_dock.py <bundle>/docking <study_dir>

# Storage and RAM
bash scripts/rental/system_bench.sh       # dd + four-kernel STREAM

# Expression chain + GO enrichment (simulated data, seed 4242)
bash scripts/rental/rna_analysis.sh
```

Raw artifacts (JSON/TSV/logs/hashes) live in the local archive
`release-artifacts-rental/round{1..4}-*` (git-ignored, not committed).

## 6 Engineering records (operational lessons, all fixed in the scripts)

1. Long remote jobs must be detached with `setsid`: cancelling the parent SSH
   session kills the whole process group — round 1's LLVM install died
   exactly this way.
2. Rental containers generally lack `/usr/bin/time`; use `date +%s.%N`
   deltas instead.
3. Two PDBQT parsing traps: `ENDROOT` matches a naive prefix test of
   `startswith("END")` (the ROOT block holds exactly 4 atoms, producing a
   "4-atom ligand" illusion); polar hydrogens (HD) must be filtered by
   element for heavy-atom RMSD to mean anything.
4. The GO term validator accepts only `GO:` plus seven digits — a synthetic
   association table using `GO:TERM00` was correctly rejected; input error,
   not over-validation.
5. On mainland-China cloud networks, cargo via the rsproxy mirror and GitHub
   via the academic accelerator are mandatory; the accelerator is conversely
   slower for apt.llvm.org (~7 KB/s measured).

## 7 Conclusion

On the bandwidth-bound axis the RTX 4080 SUPER and the MTT S4000 are a
legitimate pair: same STREAM efficiency band, triad slightly to Moore
Threads, unidirectional copy and atomics to NVIDIA — the "never cheaper"
intuition holds at the ecosystem layer (toolchains, driver stacks) but not
at the silicon layer. The more important result for the SDK: **five classes
of analysis outputs are bit-identical across the two vendors' machines**;
the determinism claim survived a cross-vendor test.

## References

[1] Williams, S., Waterman, A., Patterson, D. Roofline: an insightful
visual performance model for multicore architectures. *Commun. ACM*
52(4), 65–76 (2009). doi:10.1145/1498685.1498715

[2] Trott, O., Olson, A. J. AutoDock Vina: improving the speed and
accuracy of docking with a new scoring function, efficient optimization,
and multithreading. *J. Comput. Chem.* 31(2), 455–461 (2010).
doi:10.1002/jcc.21334

[3] Berman, H. M. et al. The Protein Data Bank. *Nucleic Acids Res.*
28(1), 235–242 (2000). doi:10.1093/nar/28.1.235

[4] Eberhardt, J., Santos-Martins, D., Tillack, A. F., Forli, S.
AutoDock Vina 1.2.0: New docking methods, expanded force field, and
Python bindings. *Nucleic Acids Res.* 49(W1), W3355–W3362 (2021).
doi:10.1093/nar/gkab663

[5] McCalpin, J. D. STREAM: Sustainable Memory Bandwidth in High
Performance Computers. Technical Report, University of Virginia
(1991–2007). https://www.cs.virginia.edu/stream/

[6] Anders, S., Huber, W. Differential expression analysis for sequence
count data. *Genome Biol.* 11, R106 (2010). doi:10.1186/gb-2010-11-10-r106

[7] Ashburner, M. et al. Gene Ontology: tool for the unification of
biology. *Nat. Genet.* 25(1), 25–29 (2000). doi:10.1038/75556

## Version & Declarations

- **Version**: initial release 2026-10-02.
- **Self-assessment**: an author-reported comparison, not third-party
  verified; single instance per card, single window; boundaries in §4.
  Third-party replication (via `scripts/rental/` and the commands above) is
  planned follow-up work.
- **License**: code AGPL-3.0-or-later; this article CC-BY-4.0.
- **Author & provenance**: Linxira-OS project maintainer · Repository:
  <https://github.com/Linxira-OS/linxira-bio-sdk>

---

*Linxira-OS · AGPL-3.0-or-later (code) · this article CC-BY-4.0 ·
<https://github.com/Linxira-OS/linxira-bio-sdk>*
