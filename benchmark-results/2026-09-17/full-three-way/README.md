# Full three-way benchmark sweep — all capabilities with multi-backend implementations

date: 2026-09-17; host: dedicated Linux benchmark machine (idle CPU, cores
8-15 pinned, nice 10); runner: linxira-bio benchmark run ... --backends
rust,python,r --repeat 3; inputs: committed test fixtures.

## Coverage

The python and r benchmark packs implement six capabilities; all six were
swept (3 backends x 3 repeats = 54 runs, 54 ok):

| capability | rust median | python median | r median |
|---|---|---|---|
| sequence.stats.v1 | 2 ms | 271 ms | 2,623 ms |
| fastq.qc.v1 | 2 ms | 283 ms | 572 ms |
| enrichment.overrepresentation.v1 | 2 ms | 275 ms | 512 ms |
| expression.pca.v1 | 2 ms | 284 ms | 484 ms |
| set.venn.v1 | 2 ms | 281 ms | 468 ms |
| structure.pdb.summary.v1 | 2 ms | 280 ms | 492 ms |

Every other capability in the catalog is rust-native only by design
(salmon orchestration, alignment, annotation, variant, structure-view,
plot rendering, ...); those are covered by the real-data benchmarks below.

Files: summary.csv (per-backend median of 3), runs_raw.csv (all 54 runs
with per-run wall_ms and errors).

## Environment notes (recorded honestly)

- The R backend initially failed 6/6 with "output identity does not match
  the request": root cause was missing R packages (jsonlite, digest) and
  Biostrings for sequence.stats on the fresh host, so the R harness fell
  into its error envelope. After installing jsonlite 2.0.0, digest 0.6.39
  and Biostrings 2.80.2 into the user library, all R runs pass. The worker
  resolves python3 from PATH; this sweep ran with a conda env python that
  carries Biopython (needed by the python sequence.stats pack).
- The deployment is part of the benchmark result: reproducing these numbers
  requires the pack dependency locks (dependencies.lock.json) to be
  satisfied.

## Real-data anchors (same host, larger inputs)

- fastq.qc.v1 on a 1.2 GB plain FASTQ (SRR28573921): rust 21.9 s vs python
  316.6 s (14.5x). The python/r packs reject gzip-compressed FASTQ while
  the rust engine decompresses transparently (dev-track item).
- expression.pca.v1 on 33,955 x 172 (log2 TPM): rust agrees with an
  independent numpy SVD to four significant figures (PC1 22.36 percent).
- Rhizosphere 32-sample PCA (33,955 x 32): PC1 40.1 percent, SDK and numpy
  agree.
