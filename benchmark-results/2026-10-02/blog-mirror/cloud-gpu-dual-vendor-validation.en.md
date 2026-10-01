---
title: "Taking a Docking Benchmark with a Known Answer onto Two GPU Servers from Different Vendors"
date: 2026-10-02
tag: technical
lang: en
desc: "Linxira Bio SDK cloud validation: on the official HIV-1 protease / indinavir redocking benchmark, how much search budget is enough (e=8 loses the correct pose 1 in 3 times; e>=32 never did); whether our analysis chain recovers a signal deliberately planted in a simulated transcriptome; and whether switching to a completely different vendor's server changes the output by a single byte."
---

# Taking a Docking Benchmark with a Known Answer onto Two GPU Servers from Different Vendors

> Linxira Bio SDK cloud validation report · 2026-10-02. We rented two
> servers — one with an NVIDIA RTX 4080 SUPER, one with a Moore Threads
> MTT S4000 — and ran several of our analysis chains on both. This post
> records the answers to three questions. Environment and commands are
> fully disclosed; reproduction scripts ship with the repository.

## The short version

1. **Molecular docking has a clear "enough" line for search budget**: on
   the official tutorial system (HIV-1 protease + indinavir), the default
   exhaustiveness of 8 lost the correct pose once in three seeds (off by
   12 Å); at 32 and above, all seeds landed within 0.07 Å of the official
   pose. Use 8 for fast screening, 32 for anything you will report —
   that is our operational recommendation.
2. **The analysis chain recovered everything we planted**: in a simulated
   transcriptome with 150 differential genes and one deliberately
   enriched pathway, the five-step chain (QC → normalization → PCA →
   clustering → enrichment) pulled the planted pathway out at
   p = 6×10⁻³¹ while every other pathway sat at background level — no
   misses, no false alarms.
3. **Different vendor, different CPU, different core count — not a
   single byte of difference**: docking outputs, SSR mining TSVs, the
   five-step expression chain, enrichment results — five classes of
   output files have identical SHA256 hashes on both machines. For a
   tool that claims local-first reproducibility, this is the hardest
   evidence there is.
4. **The domestic card runs our stuff**: the same kernel source compiled
   first-try under Moore Threads' mcc and produced correct results, and
   its memory bandwidth utilization is in the same band as the NVIDIA
   card (both have weak spots — see section 4).

## 1 The questions we were answering

This was not a benchmarking exercise for its own sake. We build
bioinformatics tooling; what we care about are three things every user
runs into:

**First, is the tool correct?** Docking is the ideal exam question
because it has an answer key: the AutoDock Vina tutorial uses HIV-1
protease with indinavir (PDB entry 1iep [3]) and publishes its best pose
at -13.234 kcal/mol. Starting from the same receptor and ligand, does
our toolchain put the molecule back where the crystal structure says it
belongs?

**Second, is the analysis chain sensitive and specific?** Transcriptome
analysis has no single answer key, so we used signal injection: plant
known differential genes and an enriched pathway in simulated data, then
check whether the tool finds exactly those. A miss is a false negative;
anything "discovered" beyond the plants is a false positive.

**Third, do results reproduce?** If the same input produces different
outputs on two vendors' machines, every downstream statistic inherits
that problem. We wanted the strictest version of the answer: file-level,
byte-identical.

## 2 The docking experiment: how much budget is enough

**What redocking is**: take an experimentally solved complex apart, keep
only the structures of the protein and the ligand, and let the docking
program place the ligand back from a random start. If it returns to the
crystal-bound position, the scoring function and search are trustworthy
on that system — the routine sanity check before virtual screening
[2,4].

We swept exhaustiveness (the search budget — roughly the number of
independent searches Vina runs in parallel) × random seed, 12
combinations, on both machines. The table shows the heavy-atom deviation
of the best pose from the official reference pose, in angstroms:

| Search budget | seed 42 | seed 43 | seed 44 |
| --- | --- | --- | --- |
| 8 (default) | 0.058 | 0.861 | **12.42 (affinity -10.9, correct pose lost)** |
| 16 | 0.058 | 0.861 | 0.015 |
| 32 | 0.062 | 0.068 | 0.015 |
| 64 | 0.056 | 0.047 | 0.045 |

How to read it: within 1–2 Å means "back in the same pocket"; 12 Å means
the molecule ended up somewhere else entirely — and the program does not
know it, because the affinity of -10.9 still looks "fine". That is the
Monte Carlo trap: not an error message, but a quietly suboptimal answer.

**The recommendation**: keep the default 8 when screening large compound
libraries (the misses are random and ranking mostly survives); the moment
a docking result is going into a conclusion, budget 32 or more.

One counterintuitive observation along the way: threads stop helping
beyond 8 (72 s → 13 s, then flat). The reason is plain — e=8 is 8
independent search chains, so 8 threads map one-to-one and there is
nothing left to parallelize. More threads do not help; only more budget
does.

## 3 The transcriptome chain: plant a signal, find the signal

We built a simulated expression profile: 2000 genes, 12 samples (6
control, 6 treated), Poisson counts with library-size variation. 150
genes were given a planted up- or down-regulation (1–2×), and one
pathway was made specifically enriched among them.

The five-step chain ran entirely through our Rust CLI (QC → median-ratio
normalization [6] → PCA → two-way clustering → GO hypergeometric
enrichment [7]):

- the planted pathway came out **first at p = 6.3×10⁻³¹** (44 genes hit);
- all 25 other pathways sat above p = 0.03 — nothing false surfaced;
- PCA and clustering separated the 12 samples cleanly by condition
  (simulated data should separate; failing to separate would mean a
  broken pipeline).

This plant-and-recover scheme is entering our routine validation flow:
it is far stricter than "ran without errors" and needs no real clinical
data.

## 4 The two GPUs, side by side

Docking and transcriptome work are CPU-bound. On the GPUs we did a
different thing: **compile the same kernel source under both vendors'
compilers and check correctness and speed**. Four kernels (copy, triad,
histogram, correlation) all passed their correctness gates (bit-level or
< 10⁻⁴ against an f64 reference):

| Kernel | RTX 4080 SUPER | MTT S4000 | Note |
| --- | --- | --- | --- |
| copy 1 GiB | 630 GB/s | 474 GB/s | one-directional; NVIDIA a third faster |
| triad (2r+1w) | 666 GB/s | 711 GB/s | the standard memory-bandwidth convention [5]; effectively a tie |
| histogram (atomics) | 26.6 GB/s | 6.5 GB/s | MUSA atomic throughput clearly behind |
| correlation matrix | 282 GB/s | 49 GB/s | see below |

In plain words: **these two cards deliver memory bandwidth in the same
league** (both around 90% of spec), so bioinformatics kernels that
mostly move data treat them as equivalent; the gap is in fine-grained
synchronization — Moore Threads is about 4× behind on atomics, which
means MUSA kernels should prefer bucketed reductions over atomic
counters. The correlation kernel is slow on both because this version
does no coalesced access (equally penalized, so the ratio means
something and the absolute numbers do not).

**Two ecosystem facts** (neutral, evidence archived in the repository
ledgers):

- Moore Threads' mcc compiled CUDA-style source directly; three micro
  fixes (a link flag and friends) later the kernels ran right the first
  time. For us this means the MUSA backend has no toolchain obstacle.
- Both vendors' Vulkan stacks have a broken layer on these cloud
  instances (NVIDIA's vGPU driver ships no Vulkan entry point; Moore
  Threads' driver loads but fails during initialization), so the
  cross-platform Vulkan GPU route is currently blocked on both — logs
  archived, feedback to the vendors planned. This does not affect the
  results above: the kernels went through each vendor's native compiler.

## 5 How to reproduce

The scripts ship in the repository (`scripts/rental/`); any Linux box
with the corresponding GPU works:

```bash
git clone https://github.com/Linxira-OS/linxira-bio-sdk.git
cd linxira-bio-sdk && cargo build --release -p linxira-bio-cli

# docking study (12-point sweep + RMSD analysis)
bash scripts/rental/dock-study.sh
python3 scripts/rental/analyze_dock.py <bundle>/docking <study_dir>

# GPU kernel comparison (one source, two vendor compilers)
mcc -O3 scripts/rental/bench_suite.mu -o bench_mu -lmusart
cp scripts/rental/bench_suite.mu b.cu && nvcc -O3 -arch=sm_89 b.cu -o bench_cu

# transcriptome signal-injection validation + storage/RAM bandwidth
bash scripts/rental/rna_analysis.sh
bash scripts/rental/system_bench.sh
```

Raw output files (JSON/TSV/logs/hashes) live in the local archive
`release-artifacts-rental/` (not committed); per-round experiment
ledgers are in `docs/engine-evals/`.

## 6 Boundaries (as always)

- One rented cloud instance per card, one time window; cloud vCPUs and
  memory bandwidth are subject to host scheduling, so **wall clocks
  describe that instance, not the vendor's hardware spec**.
- Disk read figures are page-cache numbers (the container may not drop
  caches); the docking conclusions hold for the official 1iep system —
  validate others independently.
- The e=8 failure mode is an inherent property of Monte Carlo search
  budget, not a defect in Vina or in us; its value is reminding you to
  set your own budget line.

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

- **Version**: initial release 2026-10-02 (this is a rewrite: the first
  draft was restructured the same day after reader feedback — organized
  around the scientific questions and written for the user).
- **Self-assessment**: an author-reported validation, not third-party
  verified; one instance per card, one window; boundaries in §6.
  Third-party replication is planned follow-up.
- **License**: code AGPL-3.0-or-later; this article CC-BY-4.0.
- **Author & provenance**: Linxira-OS project maintainer · Repository:
  <https://github.com/Linxira-OS/linxira-bio-sdk>

---

*Linxira-OS · AGPL-3.0-or-later (code) · this article CC-BY-4.0 ·
<https://github.com/Linxira-OS/linxira-bio-sdk>*
