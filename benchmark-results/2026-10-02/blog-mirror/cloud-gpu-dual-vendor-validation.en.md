---
title: "Taking a Docking Benchmark with a Known Answer onto Two GPU Servers from Different Vendors"
date: 2026-10-02
tag: technical
lang: en
desc: "Pre-launch validation for GPU vendor support in the Linxira Bio SDK, run entirely on established community benchmarks (the official Vina tutorial system, the STREAM convention, pytrf as the reference implementation) on one NVIDIA and one Moore Threads server: correctness, byte-level reproduction, and speed. The docking search budget gets a clear enough line (e=8 loses the pose 1 in 3; e>=32 never did), five classes of outputs hash-identical across vendors, and the Rust implementation holds a steady 3.6-4.0x over the traditional Python tool."
---

# Taking a Docking Benchmark with a Known Answer onto Two GPU Servers from Different Vendors

> Linxira Bio SDK cloud validation report · 2026-10-02. Environment,
> machine performance, and commands fully disclosed; reproduction scripts
> ship with the repository.

## The short version

1. **Docking search budget has a clear "enough" line**: on the official
   Vina tutorial system (HIV-1 protease + indinavir), the default
   exhaustiveness of 8 lost the correct pose once in three seeds (off by
   12 Å); at 32 and above, all seeds landed within 0.07 Å of the official
   pose. Use 8 for fast screening, 32 for anything you report.
2. **The analysis chain passed a signal-injection test**: with 150
   differential genes and one deliberately enriched pathway planted in a
   simulated transcriptome, the five-step chain recovered the pathway at
   p = 6×10⁻³¹ with everything else at background level — no misses, no
   false alarms.
3. **The two servers produced byte-identical outputs**: docking, SSR
   mining, the expression chain, enrichment — five classes of output
   files have identical SHA256 hashes on both machines.
4. **The Rust implementation holds a steady 3.6–4.0× over the traditional
   Python tool** (1 M-base SSR mining, re-measured on three different
   CPUs), with byte-identical output to the reference implementation.
5. **The domestic card runs our stack**: the same kernel source compiled
   first-try under Moore Threads' mcc with correct results, and its
   memory bandwidth utilization is in the same band as the NVIDIA card.

## 1 Who ran this, and why

We did — the Linxira Bio SDK project. The SDK is about to gain GPU vendor
backends (including Moore Threads MUSA), and before shipping that we
needed one question answered: **on different vendors' hardware, is our
toolchain still correct, still fast, and still reproducible?** We rented
two cloud servers for a full validation round; this post is the record.

One principle up front: **every exam question is an existing community
benchmark, nothing home-made** —

- docking uses the official AutoDock Vina tutorial system (HIV-1 protease
  + indinavir, PDB entry 1iep [3]), whose answer key ships with the
  tutorial (best affinity -13.234 kcal/mol and the reference pose file);
- memory bandwidth uses the STREAM convention [5];
- SSR mining uses pytrf, the community reference implementation, as the
  correctness control;
- transcriptomics has no single answer key, so we used signal injection —
  plant known signals in simulated data and have the tool find them — the
  standard way to validate an analysis pipeline.

Established benchmarks mean the results are directly comparable with the
community, with no question of a test tailored to ourselves.

## 2 Environment and machine performance

Two cloud-rented servers, same-day window, same repository clone built
on-site (47.3 s for 59 crates):

| | Server A (NVIDIA) | Server B (Moore Threads) |
| --- | --- | --- |
| GPU | RTX 4080 SUPER, 32 GB, driver 595.71.05 (sm_89) | MTT S4000, 48 GB, MUSA driver 2.7.0 |
| GPU compiler | nvcc 11.8 | mcc (MUSA toolkit 3.1.0) |
| CPU | Xeon Platinum 8375C @2.9GHz, 12 vCPUs (62 GB RAM) | Xeon Gold 6430, 15 vCPUs (100 GB RAM) |
| OS | Ubuntu 22.04, kernel 5.15.0-25 | Ubuntu 22.04, kernel 5.15.0-105 |
| Memory bandwidth (measured, STREAM triad) | 14.9 GB/s | 13.5 GB/s |
| Data-disk seq. write (2 GiB) | 505 MB/s | 677 MB/s |
| SDK | linxira-bio 1.1.1 (Rust) | same |
| Docking tool | AutoDock Vina 1.2.5 official binary | same |

(Measured memory/disk figures reflect these two cloud instances under
host scheduling, not vendor specifications; see §8.)

## 3 What was proven

### 3.1 Docking: correct, and with a known budget line

The redocking check — take the experimentally solved complex apart and
let the program place the ligand back from a random start [2,4]. Twelve
(budget × seed) combinations on each machine; the table shows heavy-atom
deviation of the best pose from the official reference pose, in Å:

| Search budget | seed 42 | seed 43 | seed 44 |
| --- | --- | --- | --- |
| 8 (default) | 0.058 | 0.861 | **12.42 (-10.9, correct pose lost)** |
| 16 | 0.058 | 0.861 | 0.015 |
| 32 | 0.062 | 0.068 | 0.015 |
| 64 | 0.056 | 0.047 | 0.045 |

**Both machines produced this table value-for-value identical** (failed
seed included). Within 1–2 Å means "back in the same pocket"; 12 Å is the
classic Monte Carlo under-budget trap — no error message, just a quietly
suboptimal answer (-10.9 still looks "fine"). Threads stop helping past 8
(72 s → 13 s, then flat) because e=8 is exactly 8 independent chains.

### 3.2 The transcriptome chain passed signal injection

A simulated profile (2000 genes × 12 samples, Poisson counts) with 150
planted differential genes and one planted enriched pathway, run through
the five-step chain (QC → median-ratio normalization [6] → PCA →
clustering → GO hypergeometric enrichment [7]) entirely via our Rust CLI:

- the planted pathway came out **first at p = 6.3×10⁻³¹** (44 genes);
  all 25 others sat above p = 0.03 — nothing false surfaced;
- PCA and clustering separated the samples cleanly by condition.

### 3.3 Reproducibility: five output classes, byte-identical

Docking JSONs, SSR TSVs (1 M bases), same-seed docking reruns, expression
chain JSONs, enrichment JSONs — **SHA256-identical across the two
servers**. Different vendor, different CPU, different core count, same
bytes.

## 4 Our speed against the traditional tool

SSR (microsatellite) mining is a direct same-algorithm comparison: our
Rust implementation vs pytrf, the community's standard Python
implementation (which doubles as the correctness reference). 1 M-base
input, median of three runs on three different CPUs:

| Machine (CPU) | Rust | pytrf (traditional Python) | speedup |
| --- | --- | --- | --- |
| Round-1 cloud-rental instance (16-core Xeon) | 0.082 s | 0.312 s | 3.8× |
| 15-core Xeon Gold 6430 (server B) | 0.047 s | 0.169 s | 3.6× |
| 12-vCPU Xeon Platinum 8375C (server A) | 0.073 s | 0.288 s | 4.0× |

Output files are **byte-identical** to the reference implementation —
the speedup does not come at the cost of changed results. Worth stating
about docking: our role there is orchestration and reproduction (we call
the native Vina engine [2,4]); we claim no credit on the docking
algorithm itself — this round proves our orchestration adds no bias.

## 5 The two GPUs, side by side

One kernel source, each vendor's compiler, four kernels (copy, triad,
histogram, correlation), all through their correctness gates (bit-level
or < 10⁻⁴ vs an f64 reference):

| Kernel | RTX 4080 SUPER | MTT S4000 | Note |
| --- | --- | --- | --- |
| copy 1 GiB | 630 GB/s | 474 GB/s | one-directional; NVIDIA a third faster |
| triad (2r+1w, STREAM) | 666 GB/s | 711 GB/s | effectively a tie |
| histogram (atomics) | 26.6 GB/s | 6.5 GB/s | MUSA atomic throughput ~4× behind |
| correlation matrix | 282 GB/s | 49 GB/s | no coalescing on either side; not peak |

Conclusion: **same bandwidth league** (both ~90% of spec), equivalent for
data-movement-bound bioinformatics kernels; the atomics gap is worth
coding around on MUSA (bucketed reductions instead of atomic counters).

## 6 Two driver-layer facts found along the way

Neutral records, log evidence in the repository ledgers: NVIDIA's vGPU
cloud driver ships no Vulkan entry point (three independent instances,
same verbatim error); Moore Threads' Vulkan driver loads and opens the
device but fails during initialization (suspected userspace toolkit
3.1.0 vs host driver 2.7.0 pairing; evidence being packaged for vendor
feedback). Neither affects this post's results — the kernels went
through each vendor's native compiler.

## 7 How to reproduce

```bash
git clone https://github.com/Linxira-OS/linxira-bio-sdk.git
cd linxira-bio-sdk && cargo build --release -p linxira-bio-cli

bash scripts/rental/dock-study.sh                          # docking study + RMSD
mcc -O3 scripts/rental/bench_suite.mu -o bench_mu -lmusart # GPU kernels (MUSA)
cp scripts/rental/bench_suite.mu b.cu && nvcc -O3 -arch=sm_89 b.cu -o bench_cu
bash scripts/rental/rna_analysis.sh                        # signal-injection check
bash scripts/rental/system_bench.sh                        # storage/RAM bandwidth
```

Raw outputs (JSON/TSV/logs/hashes) live in the local archive
`release-artifacts-rental/` (not committed); per-round ledgers in
`docs/engine-evals/`.

## 8 Boundaries

- One rented instance per card, one window; cloud vCPUs, memory
  bandwidth, and disks are subject to host scheduling — **measured
  latencies describe these two instances, not vendor specifications**.
  CPU/memory in the environment table follow the rental platform's
  console specification (in-container nproc reports host CPUs, not the
  instance quota); software versions are on-machine readings.
- Disk reads are page-cache figures (containers may not drop caches);
  the docking conclusions hold for the official 1iep system — validate
  others independently.
- The e=8 failure mode is an inherent property of Monte Carlo search
  budget, not a defect; its value is reminding you to set your own
  budget line.

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
- **Self-assessment**: an author-reported validation, not third-party
  verified; one instance per card, one window; boundaries in §8.
  Third-party replication is planned follow-up.
- **License**: code AGPL-3.0-or-later; this article CC-BY-4.0.
- **Author & provenance**: Linxira-OS project maintainer · Repository:
  <https://github.com/Linxira-OS/linxira-bio-sdk>

---

*Linxira-OS · AGPL-3.0-or-later (code) · this article CC-BY-4.0 ·
<https://github.com/Linxira-OS/linxira-bio-sdk>*
