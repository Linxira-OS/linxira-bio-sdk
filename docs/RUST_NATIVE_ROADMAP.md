# Rust-native roadmap: what stays external, what moves in-process

> Purpose: a reviewed matrix of the SDK's current execution model — which
> analysis paths are already pure Rust, which delegate to native tools, and
> which of those delegations have a credible Rust-ecosystem replacement.
> Each candidate carries expected gain, migration cost, and risk so work can
> be scheduled by value instead of enthusiasm.
>
> 本文档回答三个问题：哪些管线已是纯 Rust、哪些外调原生工具、哪些外调有成熟
> Rust 生态可直接替换（含收益/成本/风险定级）。

## 1. Current execution model (verified against `native_tools.rs`)

- **Pure Rust in-engine** (no external process): all sequence.* capabilities,
  fastq.* , variant.* (VCF parse/filter/normalize/compare), annotation.*
  (GFF3/GTF parse/stats/extract), expression statistics (PCA/cluster/
  normalize/matrix-QC), set.venn/upset, enrichment ORA + preranked GSEA,
  structure.pdb/mmcif/contact-map/geometry/superpose, protein.properties,
  functional (GO/eggNOG normalization), benchmark + environment, and the
  PlotSpec SVG renderers.
- **Delegated to native tools** (22 programs, controlled shell-free
  invocation): makeblastdb/blast, diamond, hmmer (hmmsearch/hmmscan/mast),
  muscle, trimal, iqtree2, MCScanX, KaKs_Calculator, meme, mkdssp, samtools,
  bamCoverage (deepTools), minimap2, snpEff, RNAfold, kraken2, salmon,
  Rscript (WGCNA), plus fasterq-dump in the benchmark harness.

## 2. Replacement matrix

### Tier 1 — credible pure-Rust replacements (do first)

| current external use | Rust replacement | expected gain | cost | risk |
|---|---|---|---|---|
| hand-rolled FASTA/FASTQ line readers | `needletail` (SIMD-fast parser, production-grade) | 2–5× parse throughput on large FASTQ; less of our code | low (drop-in reader swap + tests) | low |
| `flate2` default backend (miniz_oxide) | same crate, **zlib-ng** feature | ~2–3× gzip decompress; affects every `.gz` input | trivial (feature flag) | low |
| single-threaded gzip output | `gzp` (parallel gzip over flate2) | ~core-count× on `.gz` writing (optional output format) | low | low |
| samtools (BAM/CRAM decode for `alignment.qc`, `coverage`, `bam-cram-qc`) | `noodles` (pure-Rust BAM/CRAM/SAM/BGZF/VCF/GTF…) | drops a native dependency for read paths; in-process streaming; equal decode speed | medium (API surface, index handling) | medium |
| bamCoverage / deepTools (`alignment.bam-to-bigwig`) | `bigtools` (pure-Rust bigWig/bigBed with zoom levels) | drops Python/deepTools from the path | medium | medium |
| serial batch execution in bench harnesses | `rayon` (batch-level parallelism, disk-watermark aware) | near-core-count× on multi-sample batches once I/O allows | low | low |

### Tier 2 — FFI bindings (same decode speed, better plumbing)

| current external use | Rust binding | gain | cost | risk |
|---|---|---|---|---|
| minimap2 (long-read alignment) | `minimap2-sys`/`minimap2` crate | no process spawn; streaming API; same aligner core | medium | medium |
| SRA archives (fasterq-dump) | **`ncbi-vdb-sys` (ArcInstitute) — Rust FFI to ncbi-vdb, ships a fastq-dump-style stdout example; FDA CFSAN also maintains Rust SRA tooling** | decode stays C-speed, but in-process record streaming enables **zero-intermediate SRA→salmon** (no 18 GB temp) plus overlapped decode/compress/quant stages: measured unpack 790 s + quant 669 s serial vs projected max(decode, quant) on HDD | high (bindings are young: 6 stars, self-declared not feature-complete; we must add edge-case tests) | medium |

Note on SRA: upstream salmon consumes FASTQ streams, so the Tier-2 SRA
binding pays off only together with a piped pipeline (SRA → decode →
salmon, no temp files). On HDD-bound hosts that removes ~2×18 GB of
write+read per deep sample (our measured unpack segment: 250–790 s, I/O
dominated).

### Tier 3 — keep external (the tool IS the science)

blast/makeblastdb, diamond, hmmer, muscle, trimal, iqtree2, MCScanX,
KaKs_Calculator, meme/mast, mkdssp, snpEff, RNAfold, kraken2, salmon,
Rscript/WGCNA — decades of domain science, active upstream development, no
Rust equivalent at competitive correctness. The SDK's value here is
controlled invocation, structured results, and three-way verification, not
re-implementation.

**Mid-term candidate**: a pure-Rust WGCNA port (correlation → TOM →
clustering → module-trait) would remove the R runtime from
`expression.wgcna.v1`; scheduled only after the deterministic-engine work
lands, since numerical parity with R's WGCNA is the acceptance bar.

## 3. Scheduling proposal

1. zlib-ng feature + needletail reader swap (hours, immediate `.gz` win).
2. noodles for BAM/CRAM read paths (days; unblocks in-process streaming).
3. bigtools for bam-to-bigwig (days; drops deepTools).
4. gzp optional `.gz` outputs (small).
5. rayon batch parallelism in bench tools (small; disk-watermark aware).
6. minimap2 FFI + SRA ncbi-vdb FFI + piped SRA→salmon (the measured
   I/O-dominated unpack segment justifies this only after the SSD/HDD A/B
   quantifies the storage share).

## 中文摘要

现有 22 个外调工具分三档：**纯 Rust 可直接替换**（needletail 读解析、
zlib-ng 后端、gzp 并行压缩、noodles 替 samtools 读路径、bigtools 替
deepTools、rayon 批并行）；**FFI 绑定同速但省进程/可流式**（minimap2、
ncbi-vdb/SRA——后者价值在"免 18GB 中间落盘"的管道化）；**保持外调**
（blast/diamond/hmmer/muscle/trimal/iqtree/kraken2/salmon/snpEff 等，工具
即科学本身）。WGCNA 的纯 Rust 化列为中期候选，验收线是与 R 版数值一致。
落地顺序按收益/成本比：zlib-ng+needletail → noodles → bigtools → gzp →
rayon → FFI/管道化。
