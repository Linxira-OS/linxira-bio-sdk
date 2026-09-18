---
title: "When Rust Meets Transcript Quantification: A Reproducibility Benchmark of a Local-First Bioinformatics SDK"
date: 2026-09-14
tag: technical
lang: en
desc: "Linxira Bio SDK benchmark: Rust/Python/R three-backend consistency, bit-level reproduction of an existing salmon pipeline on 10 samples (TPM r = 1.000000, NumReads diff 0), and a 26-sample backfill batch that auto-resumed 20 seconds after a real scheduled reboot. All data sources and environments disclosed."
---

# When Rust Meets Transcript Quantification: A Reproducibility Benchmark of a Local-First Bioinformatics SDK

> Linxira Bio SDK benchmark report · 2026-09-14. A local-first bioinformatics toolkit measured on real data for scientific workstations: environment, data, and reproduction commands fully disclosed. Version history and declarations at the end ("Version & Declarations").

## TL;DR

- **Three backends agree**: independent Rust / Python / R implementations of the same algorithm agree field by field within a 1e-6 tolerance (bit-identical on several items); fixture-level median latency rust 2 ms, python 271–284 ms, r 468–2,623 ms (§5.1, §6.3).
- **Bit-level reproduction of the existing pipeline**: on 10 normally covered samples, TPM Pearson r = 1.000000 (σ = 0), NumReads relative difference 0, zero 3σ outliers (§5.2); byte-level SHA256 identity evidence in §6.2.
- **r = 1 is earned**: the control group without `--seqBias --gcBias` measured r = 0.980896 — it holds only under strict parameter alignment; derivation and control in §6.1, determinism boundary in §5.5.
- **A 26-sample backfill batch** (~520M reads) crossed a real scheduled reboot, auto-resumed 20 seconds after boot with zero manual intervention, and delivered the complete 33,955-transcript ID set 26/26 (§5.4).

## Changelog

- **2026-09-14** Initial release.
- **2026-09-15** Added the deep-library backfill batch results (§5.4).
- **2026-09-18** Added the r=1 derivation and control experiment, byte-level evidence, determinism boundary, follow-up validation, and engineering records; same day the article was restructured: results consolidated into §5.1–5.5, derivation & independent verification split out as §6, all commands consolidated into §7, engineering/ops records moved to appendix §8.
- **2026-09-18 (addendum)** Added §7.4: archived verification data and process files (`benchmark-results/`).
- **Maintenance rule**: new content always goes to its logical chapter plus a dated line in this changelog; no more whole-section in-place appends.

## 1. Background

The core design of Linxira Bio SDK is "a deterministic Rust engine + controlled invocation of native tools + three-way cross-validation". For statistical analyses the engine is implemented in Rust, and the benchmark pack ships independent Python and R implementations of the same algorithm. For ecosystem-level tools (salmon, fasterq-dump, kraken2, and so on), the engine performs controlled, shell-free orchestration and reduces their output to verifiable structured results.

This round answers two questions:

1. Can the three implementations corroborate each other? (trustworthiness)
2. On real data, with exactly the same parameters as an existing pipeline, can the SDK reproduce its output? (correctness)

## 2. Data Sources (all public)

| Data | Accessions | Source |
|---|---|---|
| Buckwheat (Fagopyrum) transcriptome samples ×10 (batch validation) | SRR15243898, SRR1552100, SRR1552203, SRR1552215, SRR1552217, SRR1552218, SRR17715775, SRR17715776, SRR17715777, SRR17715778 | NCBI SRA (PRJNA253089, PRJNA749630, etc.; the full 172-run list is in the project census) |
| Deep-library paired sample (deployment validation) | SRR26171873 | NCBI SRA |
| Deep/regular backfill samples ×26 (the uncomputed subset of the same 172-run study; full accessions in §5.4) | SRR19049440/41/43-45, SRR22699505–516, SRR24322339/40/42-45/47/52/53 | NCBI SRA (pulled via the NAS authoritative copy) |
| Edge-case sample (truncated FASTQ guard) | SRR22699515 | NCBI SRA (truncated copy; the authoritative original was verified separately) |
| Zero-mapping small-RNA samples ×14 (edge cases) | SRR28573920-25, SRR8205656-63 | NCBI SRA |
| Reference index | — | Pinku1 CDS salmon index (33,955 transcripts) |
| Reference quantification | same 10+14 runs | quant.sf output of the existing pipeline (read-only control) |

Raw reads and the reference index stay on the lab workstation and are not committed; this report and the repository carry only metrics, methodology, and provenance.

## 3. Environment Disclosure (complete)

### 3.1 Batch-validation workstation (Linux)

| Item | Value |
|---|---|
| OS | CachyOS (Arch-based), kernel 7.2.3-1-cachyos |
| CPU | Intel Xeon E5-2676 v3 @ 2.40GHz (24 threads; runs pinned to cores 8–15, nice 10) |
| Memory | 32 GiB DDR3-1600 MT/s (2×16 GiB DIMM) |
| GPU | AMD Radeon RX 580 2048SP (Polaris 20, amdgpu driver) — **unused**; all workloads are CPU-only |
| Storage | System/logs: Kingston SA400S37 120GB **SSD**; benchmark I/O volume: WD WD5000AZLX 500GB 7200rpm **HDD**; reference volume: Seagate ST3000DM001 3TB **HDD** |
| SDK | linxira-bio v1.0.3 (release build, Rust) |
| salmon | 2.7.0 (official bioconda ELF binary, micromamba user prefix) |
| fasterq-dump | 3.4.1 |
| Python / R | 3.14 / 4.6.1 (verification helpers only) |
| Containers | none |

### 3.2 Three-backend baseline environment (Windows + WSL2)

| Item | Value |
|---|---|
| Host | Windows 11 build 26200.9168; 14-inch laptop (Intel Core Ultra 5 225H, 14 logical cores; 32 GiB LPDDR5X-8533) |
| GPU | Intel Arc 130T integrated graphics (driver 32.0.101.8991) — unused |
| Storage | NVMe SSD: Samsung PM991a 512GB (system) + YMTC PC411 1TB (WSL2 filesystem lives on NVMe) |
| WSL2 | Arch Linux guest, kernel 6.18.33.2-microsoft-standard-WSL2, 7940 MB visible to the guest |
| Software stack | Rust engine 1.0.1; Python 3.14.6 (Biopython 1.88); R 4.6.1 (Biostrings 2.80.2, jsonlite, digest) |

### 3.3 Software references

- Patro R, Duggal G, Love MI, Irizarry RA, Kingsford C. **Salmon provides fast and bias-aware quantification of transcript expression.** Nature Methods 14, 417–419 (2017).
- Leinonen R, Sugawara H, Shumway M; INSDC. **The Sequence Read Archive.** Nucleic Acids Research 39, D19–D21 (2011). (SRA Toolkit / fasterq-dump)
- Cock PJA et al. **Biopython.** Bioinformatics 25, 1422–1423 (2009).
- Page AJ et al. **Biostrings.** Bioconductor (used by the R reference implementation).
- Key Rust crates: flate2 (zlib-ng backend planned), csv, serde, rayon (roadmap in the repository's `docs/RUST_NATIVE_ROADMAP.md`).

## 4. Methods

### 4.1 Three-backend consistency (bench-20260913-001)

Independent implementations of the same algorithm in three languages (Rust engine / Python pack / R pack), identical input, one warm-up per backend, then 5 timed runs: median wall time and peak RSS (`/usr/bin/time -v`). Consistency criterion: structured field-level diff with a numeric relative tolerance of 1e-6.

### 4.2 Exact reproduction of the reference pipeline (bench-20260914 series)

The complete flow for each sample (entirely through the public CLI, no internal shortcuts): SRA → FASTQ decompression → SDK-orchestrated salmon quantification, with the decompression and quantification segments timed separately and parameters identical to the reference pipeline. **All commands are consolidated in §7.2** and not repeated here.

**3σ acceptance rule**: for each passing batch, compute mean/σ/3σ interval and outlier count per metric; a batch PASSES only with zero 3σ outliers and a worst-case TPM r ≥ 0.995.

## 5. Results

### 5.1 Three-backend consistency (repository fixtures, bench-20260913-001)

| Capability | rust | python | r | Speed-up (vs python) | Memory saved |
|---|---|---|---|---|---|
| sequence.stats.v1 | 40 ms | 330 ms | 2110 ms | 8.25× | 86.5% |
| expression.pca.v1 | 40 ms | 300 ms | 460 ms | 7.50× | 85.1% |
| set.venn.v1 | 40 ms | 310 ms | 400 ms | 7.75× | 85.7% |
| structure.pdb.summary.v1 | 40 ms | 320 ms | 410 ms | 8.00× | 86.3% |

All four capabilities are Consistent across the three backends (against the Rust golden: PDB bit-identical at 0.0; PCA/Venn maximum relative error 2.26e-11). On small inputs the wall clock is dominated by interpreter start-up — which is exactly what this baseline characterizes; real-data throughput is in §6.3.

### 5.2 Bit-level reproduction of the reference pipeline (10-sample batch, bench-20260914-003/004)

Transcript-by-transcript comparison against the existing pipeline's quant.sf (parameters fully aligned: `-l A -p 8 --validateMappings --seqBias --gcBias`, same salmon 2.7.0, same index):

| Metric | n | mean | σ | 3σ interval | 3σ outliers |
|---|---|---|---|---|---|
| TPM Pearson r | 10 | 1.000000 | 0.000000 | [1.0, 1.0] | 0 |
| NumReads relative difference | 10 | 0.000e+00 | 0.000e+00 | [0, 0] | 0 |
| Quantification wall time (s) | 10 | 602.9 | 94.4 | [319.8, 886.0] | 0 |
| Decompression wall time (s) | 10 | 250.8 | 27.4 | [168.7, 332.9] | 0 |

**Batch-level 3σ acceptance: PASS** (10/10 passed; zero outliers; worst r = 1.000000).

Per-run detail (decompression/quantification seconds; CPU utilization = (user+sys)/wall; the first run SRR15243898 has no CPU accounting, but its 250 s / 677 s wall times are included in the statistics above):

| run | decompress s | quantify s | CPU utilization |
|---|---|---|---|
| SRR1552100 | 302 | 684 | 79.4% |
| SRR1552203 | 255 | 645 | 81.9% |
| SRR1552215 | 267 | 614 | 89.2% |
| SRR1552217 | 228 | 534 | 102.6% |
| SRR1552218 | 234 | 661 | 82.0% |
| SRR17715775 | 204 | 613 | 96.1% |
| SRR17715776 | 275 | 526 | 115.8% |
| SRR17715777 | 236 | 685 | 88.4% |
| SRR17715778 | 257 | 390 | 155.9% |

*The first same-parameter control (SRR1460477, 16.7M mapped reads) likewise gave r = 1.000000 with identical NumReads per transcript; that run is also the worked example of §6's derivation and byte-level evidence.*

### 5.3 Deployment validation and data guards

- A deep-library paired-end sample (SRA 3.68 GB → 2×9.07 GB FASTQ) ran end to end; the output quant.sf matches the reference format item by item (row count / IDs / header).
- **Truncation guard**: a truncated FASTQ whose line count is not a multiple of 4 is rejected with a structured error (zero silent output). This is not a theoretical concern — an interrupted transfer produced exactly such a file, and it had caused the existing pipeline to fail repeatedly on the same run.
- **Automatic exposure of non-mRNA libraries**: two independent implementations measured 0–0.0019% mapping rates on 14 samples, confirming small-RNA libraries (which belong on a miRNA track); they are annotated `excluded: non-mRNA library`. Two independent implementations producing the same "near-zero mapping" signal on the same data is a direct demonstration of the value of cross-validation.

### 5.4 Deep-library backfill batch (26 samples, ~520M reads, one real reboot)

From 2026-09-14 to 09-15, using exactly the same pipeline and parameters as §5.2 (same salmon 2.7.0, same index, `-l A -p 8 --validateMappings --seqBias --gcBias`, 8 threads pinned to cores 8–15), we backfilled **26 paired-end samples** from the same 172-run study that the existing pipeline **had not yet computed**. These samples have no existing output to compare against — they are precisely the part waiting to be computed — so the acceptance criteria were threefold: ① complete output row count and transcript ID set (33,955/33,955); ② the pipeline itself had already achieved bit-level reproduction with r = 1.000000 on comparable samples in §5.2; ③ structured logging throughout (per-run quantify.json / logs / CPU accounting).

**Batch results**: 26/26 passed, every quant.sf exactly 33,955 rows; total decompression 8,891 s and quantification 6,551 s (≈4.3 hours of pure compute, excluding NAS pulls); median quantification CPU utilization 681% (8 threads nearly saturated). Per-run detail (reads = NumReads total, util = (user+sys)/wall; the edge-case sample SRR24322347 is moved to the anomaly detail below and excluded from the homogeneous rows):

| run | reads (M) | expressed transcripts | decompress s | quantify s | util |
|---|---|---|---|---|---|
| SRR19049440 | 18.11 | 23,779 | 1,023 | 154 | 432.5% |
| SRR19049441 | 16.87 | 23,722 | 1,111 | 806 | 95.8% |
| SRR19049443 | 17.60 | 23,713 | 1,062 | 1,063 | 69.4% |
| SRR19049444 | 18.49 | 22,954 | 1,026 | 935 | 68.3% |
| SRR19049445 | 17.77 | 23,461 | 133 | 72 | 659.2% |
| SRR22699505 | 22.23 | 24,820 | 193 | 83 | 681.3% |
| SRR22699506 | 24.31 | 24,715 | 219 | 94 | 679.7% |
| SRR22699507 | 21.78 | 24,563 | 188 | 84 | 684.8% |
| SRR22699508 | 26.53 | 24,535 | 226 | 98 | 677.8% |
| SRR22699509 | 22.46 | 26,730 | 198 | 97 | 694.3% |
| SRR22699510 | 20.50 | 26,558 | 192 | 88 | 690.5% |
| SRR22699511 | 23.30 | 26,338 | 219 | 97 | 691.2% |
| SRR22699512 | 21.03 | 26,380 | 197 | 90 | 693.2% |
| SRR22699513 | 21.53 | 23,946 | 185 | 80 | 687.4% |
| SRR22699514 | 21.90 | 24,385 | 199 | 81 | 685.8% |
| SRR22699515 | 20.49 | 26,385 | 185 | 82 | 691.0% |
| SRR22699516 | 17.97 | 25,887 | 164 | 71 | 690.6% |
| SRR24322339 | 12.22 | 20,693 | 299 | 435 | 213.1% |
| SRR24322340 | 22.04 | 22,907 | 265 | 345 | 240.6% |
| SRR24322342 | 10.83 | 19,834 | 285 | 373 | 232.3% |
| SRR24322343 | 29.45 | 24,392 | 280 | 376 | 223.7% |
| SRR24322344 | 28.70 | 23,877 | 277 | 378 | 224.4% |
| SRR24322345 | 27.80 | 24,160 | 269 | 354 | 226.9% |
| SRR24322352 | 17.80 | 24,891 | 165 | 76 | 693.8% |
| SRR24322353 | 17.81 | 24,314 | 169 | 75 | 684.9% |

**Anomaly detail**:

| run | reads (M) | expressed transcripts | decompress s | quantify s | util | note |
|---|---|---|---|---|---|---|
| SRR24322347 | 0.02 | 4,257 | 162 | 64 | 690.9% | near-empty library edge case (12.5% of the index), see observation 2 |

**Observation 1: CPU-seconds are far more stable than wall time — which is exactly why CPU accounting is recorded per run.** The same group of deep-library samples (SRR1904944x, 16.9–18.5 M reads) completed under three load conditions: concurrent with the main analysis workload (util 68–96%), quantification wall time was 806–1,063 s; after the reboot, with near-exclusive use of the pinned cores (util 659–694%), comparable samples took only 72–154 s; an intermediate state (SRR243 group, util ~213–241%) fell in between. But converted to CPU time (user+sys), samples of this size converge to roughly 475–772 CPU·s, while wall time spreads over 15× (72–1,063 s). The conclusion matches the honest boundaries in §5.5: **in this mechanical-disk + shared-workstation deployment environment, wall time describes I/O and concurrency, while CPU-seconds describe the computation itself**. Any wall-time comparison across load states should start from the util column.

**Observation 2: the empty-library edge case was exposed automatically again.** SRR24322347 has only 16,927 reads and 4,257 expressed transcripts (12.5% of the index) — a nearly idle library. Like the non-mRNA case in §5.3, it was not silently folded into any mean; it remains a separate line in the anomaly detail for downstream researchers to judge, and its 64 s quantification time is proportional to its read count — the processing itself was normal.

**Automatic resumption across a real reboot.** The batch deliberately crossed the host's daily 06:50 scheduled reboot: the machine rebooted at 06:50:30, and at 06:50:50 (20 seconds after boot) a user-level systemd oneshot unit automatically triggered the resume — completed samples were skipped by output existence (4), the remaining 22 continued in order, and the batch finished at 09:24:18 with zero manual intervention. Before starting, the resume entry cleans two kinds of interruption debris: half-decompressed FASTQ files in tmp, and partial writes where "quant.sf exists but the CSV has no final row" (preventing incomplete artifacts from being mistaken for completed ones). Each sample directory includes a README (new_computation annotation: source accession, quantifier and parameters, verification records); the batch ledger (per-run CPU model / pinned cores / threads / utilization) is in bench/tier26-benchmark.csv.

### 5.5 Honest boundaries (the single authoritative list — what we do not claim)

1. **We do not claim to "beat salmon/fasterq-dump themselves"** — the SDK orchestrates the very same engines; under identical parameters we aim for bit-level reproduction (achieved), not overtaking.
2. The wall-clock times in this batch are **bounded by mechanical-disk throughput** (the benchmark I/O volume is a 500GB 7200rpm HDD); they characterize the deployment environment and are not an engine speed claim.
3. Zero-mapping samples are retained as edge cases; TPM comparisons on them are noise against noise, any implementation would "fail", and they are not used for scoring.
4. Numbers across machines, cache states (warm/cold), or code versions (each report embeds its git sha) are never mixed.
5. The backfilled samples in §5.4 have no existing output to compare against (which is why they needed backfilling); their correctness rests on the pipeline's bit-level reproduction on comparable samples and on complete ID-set verification, not per-run correlation coefficients. The batch's wall times are likewise affected by concurrent load and mechanical-disk I/O and are for deployment planning only.
6. **Determinism boundary**: r = 1.000000 holds only for "the same deterministic tool + same parameters + same input, re-run"; in that setting r < 1 would itself be an anomaly signal. Any comparison across implementations, parameters, or version factors necessarily gives r < 1 (measured control in §6.1). This report's r = 1 therefore asserts **orchestration-layer equivalence** only — not an accuracy claim, and not a new biological result.

## 6. Derivation and Independent Verification

### 6.1 Derivation of r = 1.000000 and the control experiment

**Derivation.** For a given run (e.g. SRR1460477, 16.7M mapped reads), the SDK-orchestrated salmon 2.7.0 (official bioconda build) and the reference pipeline each produce a quant.sf: same binary version, same Pinku1 index, same parameters (`-l A -p 8 --validateMappings --seqBias --gcBias`), same input FASTQ. Under exactly these conditions salmon is deterministic: per-transcript TPM and NumReads match line by line, and a Pearson correlation over all 33,955 transcript pairs gives r = 1.000000 (σ=0).

**Control experiment (recomputed on site, 2026-09-18).** The same run with the two parameters `--seqBias --gcBias` removed:

- TPM Pearson r = **0.980896** (not 1)
- max |ΔTPM| = 1.27e+04
- NumReads identical per transcript for only 25,422 / 33,955 (75%)

One wrong parameter and r instantly drops from 1.000000 to 0.981. Re-running with the full parameter set returns exactly to r = 1.000000000, max |ΔTPM| = 0, NumReads identical 33,955/33,955 (measured the same day). This shows the reported 1.000000 is **earned** through strict parameter alignment, and the control also demonstrates the sensitivity of this comparison method. The boundary interpretation lives in §5.5 item 6; reproduction commands in §7.2.

### 6.2 Byte-level evidence (SHA256 identity)

On-site re-verification of SRR1460477 on 2026-09-18: the SDK-orchestrated output and the reference pipeline's retained quant.sf are SHA256-identical (both `9b6a4cca0461e8d4117c88dc996f23f83a1730e6352936f82714377e691536af`, 1,503,599 bytes, bit for bit). r = 1.000000 is no statistical coincidence — the two files are the same string of bytes, produced independently by different orchestration paths on different dates.

### 6.3 Follow-up independent validation (2026-09-17/18, same idle host)

1. **Full three-backend sweep**: all 6 capabilities covered by the python/r benchmark packs (sequence.stats / fastq.qc / enrichment.OR / expression.pca / set.venn / structure.pdb.summary) × rust,python,r × 3 repeats = **54/54 passed**; fixture-level median latency rust 2 ms / python 271–284 ms / r 468–2,623 ms. The remaining capabilities are rust-native by design.
2. **Real-data anchors**: fastq.qc on a 1.2GB FASTQ — rust 21.9 s vs python 316.6 s (**14.5×**); PCA of the 33,955×172 expression matrix — rust and an independent numpy SVD agree on the top five principal-component ratios to four decimal places (PC1 22.36%).

## 7. Reproduction

The SDK and all benchmark tooling are open source; all commands are consolidated here.

### 7.1 Three-backend baseline (any supported platform)

```bash
linxira-bio benchmark run sequence.stats.v1 \
  fasta=tests/fixtures/sequences/tiny.fa \
  --backends rust,python,r --repeat 5 --dataset-class sequence
```

### 7.2 Real-sample comparison (SRA → quant.sf)

Generic flow (public CLI; decompression and quantification timings in the `--json` envelope):

```bash
export LINXIRA_BIO_SALMON=/path/to/salmon
fasterq-dump -e 8 --force -O tmp/run-fq SRR1460477.sra
linxira-bio expression quantify tmp/run-fq/SRR1460477_1.fastq \
  tmp/run-fq/SRR1460477_2.fastq --index <salmon_idx> --threads 8 \
  --seq-bias --gc-bias --output results/SRR1460477/quant.sf --json
```

Concrete example starting from the NAS authoritative copy (the same input as §6.1's control experiment):

```bash
scp -r <NAS>:/mnt/disk1/tier23/SRR1460477 . && fasterq-dump --split-files SRR1460477
gzip SRR1460477_1.fastq SRR1460477_2.fastq
linxira-bio expression quantify SRR1460477_1.fastq.gz SRR1460477_2.fastq.gz \
  --index <pinku1_cds_idx> --threads 8 --seq-bias --gc-bias --output quant.sf
```

(The comparison target is the reference pipeline's quant.sf for the same run; salmon must be ≥ 2.7.0 — older versions reject v2 indexes.)

### 7.3 Batch comparison and backfill

```bash
# Batch comparison and 3σ summary; batch backfill supports checkpoint resume
# (completed samples auto-skipped by artifacts), per-run CPU accounting into a CSV ledger
scripts/tier23-bench.sh --cli <linxira-bio> --index <salmon_idx> \
  --sra-dir <inbox> --reference-dir <ref_quant> --output-dir <results> \
  --tmp-dir <tmp> --csv <ledger.csv> --pin "taskset -c 8-15 nice -n 10" \
  --cores 8 --threads 8 --pull-source <nas>:<tier23> --scp-identity <key> \
  --fasterq <fasterq-dump> --runs <RUN...>
```

The raw reports (per-run JSON envelopes, per-line CPU accounting, 3σ statistics objects) are archived as well — see §7.4 below.

### 7.4 Archived verification data and process files (this repository)

All verification data, raw reports, and hash manifests behind this article are archived in the SDK repository under `benchmark-results/`, published together with the online article:

1. **On-site r = 1.000000 re-verification (2026-09-18, SRR1460477)**
   - Description and result table: `benchmark-results/2026-09-18/r-verification/README.md`
   - Hash manifest (SDK output and the reference pipeline's quant.sf are byte-level identical): `benchmark-results/2026-09-18/r-verification/SHA256SUMS.txt`
   - SDK output quant.sf / control-group (no bias parameters) quant.sf / reproduction script / raw verification output: the four files in the same directory
2. **Full record of the first 10-sample bit-level reproduction and the 0.980896 parameter-mismatch control (2026-09-14, including per-run JSON envelopes, per-line CPU accounting, and 3σ statistics objects)**: `benchmark-results/2026-09-14/summary.md`
3. **Full three-backend sweep (6 capabilities × rust/python/r × 3 repeats, 54/54 passed)**: `benchmark-results/2026-09-17/full-three-way/summary.csv`, `benchmark-results/2026-09-17/full-three-way/runs_raw.csv`
4. **Index of all benchmark records**: `benchmark-results/RECORDS.md`

Repository path (branch `dev/more-tools-and-cloud`; the path is unchanged after merging to main): <https://github.com/Linxira-OS/linxira-bio-sdk/tree/dev/more-tools-and-cloud/benchmark-results>

## 8. Appendix: Engineering Records (2026-09-17/18)

Deployment and ops records; not part of the results chapter.

### 8.1 A content-integrity lesson: byte-size acceptance is not enough

Download acceptance based on byte size alone lets "size-correct, content-corrupt" files through (12 of the 18 files in this batch were gzip-corrupted, caused by interrupted splicing from multiple concurrent downloader instances). A post-download `gzip -t` content-acceptance gate has been added; the 12 affected files were deleted and re-downloaded. The lesson that multi-instance download concurrency and content acceptance do not mix has been recorded in the ops log.

### 8.2 A known, ticketed interface defect: calling WGCNA directly via the CLI

When calling WGCNA directly via the CLI, the rust glue layer pre-creates the output directory, which is mutually exclusive with the R driver's "output directory must not exist" check, so this path always fails (CI has no R, hence it went unexposed); the current workaround is to call the R driver directly, with the fix scheduled on the development track.

## 9. Closing

The first question a "local-first" bioinformatics toolkit must answer is not "how fast" but "**is the result correct, and can others trust it**". This round answers: three languages corroborate the same algorithm; the existing pipeline is reproduced bit for bit under identical parameters; data anomalies are rejected loudly instead of quietly producing numbers; and a 4.3-hour backfill batch auto-resumed 20 seconds after a scheduled reboot and ran to completion with zero manual intervention. The speed figures (7.5–8.25× versus interpreters) came along for free — verifiability is the product.

## Appendix: the developer's note (2026-09-18)

To be honest: this batch of numbers reads somewhat "abstract". r = 1.000000, SHA256-identical files — they do not say "our tool is more accurate" or "faster". They state something plainer: the same deterministic ruler, given aligned parameters and identical input, must yield the same string of bytes on two independent runs — we achieved that, and we can see it immediately whenever the ruler is handled wrong (control group r = 0.981). The factual data is what it is, and we publish it exactly as measured. If readers take one thing away from this report, we would like it to be: **the orchestration layer can be strictly verified, anomalies are rejected loudly, and every boundary is written down** — not any single pretty number among them.

## Version & Declarations

- **Version**: initial release 2026-09-14; deep-library backfill batch added 2026-09-15 (§5.4); 2026-09-18 added the derivation and control experiment, byte-level evidence, determinism boundary, follow-up independent validation, and engineering records, and completed the full restructuring (§6 as its own chapter, commands consolidated in §7, engineering records moved to §8). See the changelog at the top for itemized changes.
- **Self-assessment**: this is an author-reported reproduction report, not independently verified by a third party. Self-assessed against the REFORMS checklist (a 32-item reporting standard for ML-based science, Science Advances 2024): **23 items met, 9 not applicable by study design (no ML modeling task here, so items on model selection, loss functions, data leakage, and statistical tests are naturally waived), 0 unmet**. Third-party replication is planned as follow-up work.
- **License**: code AGPL-3.0-or-later; this article CC-BY-4.0.
- **Author & provenance**: Linxira-OS project maintainer · Repository: <https://github.com/Linxira-OS/linxira-bio-sdk> · Product page: [Linxira Bio SDK](/bio-sdk/)

---

*Linxira-OS · AGPL-3.0-or-later (code) · this article CC-BY-4.0 ·
<https://github.com/Linxira-OS/linxira-bio-sdk>*
