# Benchmark summary `bench-20261011-001` — 2026-10-11

> Archived N=10 three-backend window on a low-end x86 host: eight capabilities measured with the Rust engine against Biopython- and Biostrings-based arms, plus a record-granularity experiment testing whether language-boundary crossings explain the sequence.stats parity. Also the first window in which the R backend executed as a real third arm.

## Environment

- os: `linux`
- distro: `CachyOS`
- kernel: `7.2.9-1-cachyos`
- cpu_model: `Intel(R) Pentium(R) Gold G7400 (2 cores / 4 threads; cpufreq governor fixed at powersave for the whole window)`
- total_memory_mb: `15756`
- engine_version: `1.1.3`
- python_version: `Python 3.14.7`
- r_version: `R version 4.6.1 (2026-06-24) -- "Happy Hop"`
- precision: `high`
- code_revision: `89882a5a31714e12e0dee4333d20f2b50b0c99c5`

## Method

- backends_flag: --backends rust,python,r
- repeat_flag: --repeat 10
- warmup: one untimed warmup per backend before the timed repeats; the warmup is excluded from the report, which holds exactly 10 timed runs (run_index 1..10)
- page_cache: warm
- disclosure: Serial execution with no concurrent build, lint or archiving; CPU governor fixed at powersave for the whole window; inputs are staged on tmpfs so no disk latency enters the measurement; backend order fixed at rust->python->r, so cross-backend frequency/thermal drift is a known limitation. Ratios hold for this machine and these inputs only; per-run wall times are archived so they can be re-checked.

## Results

| capability | dataset | class | rust | python | r | speedup | memory saving | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 20 x 5 Mb FASTA (~100 MB), engine-simulated | sequence | 810 ms | 825 ms | 1,815 ms | 1.02× | 91.5% | consistent |
| `expression.pca.v1` | 2000 x 40 count matrix (4 groups x 10 samples) | matrix | 20 ms | 4,010 ms | 2,770 ms | 200.50× | 84.0% | consistent |
| `set.venn.v1` | 20k identifiers x 4 sets (repeated occupancy masks) | other | 10 ms | 180 ms | 11,075 ms | 18.00× | 82.5% | consistent |
| `enrichment.overrepresentation.v1` | 5k genes / 45k gene-term associations across 300 terms | annotation | 30 ms | 230 ms | 4,480 ms | 7.67× | 75.4% | consistent |
| `structure.pdb.summary.v1` | 3000 residues / 24k atoms synthetic PDB with pLDDT column | structure | 30 ms | 780 ms | 20,680 ms | 26.00× | 57.1% | consistent |
| `variant.stats.v1` | 200k VCF records across 5 contigs (SNV/insertion/deletion mix) | other | 80 ms | 760 ms | 13,500 ms | 9.50× | 86.5% | consistent |
| `fastq.qc.v1` | 200 MB / 646k reads (150 bp, coverage 100) | other | 1,570 ms | 49,345 ms | 702,190 ms | 31.43× | 86.6% | consistent |
| `sequence.ssr.v1` | same ~100 MB FASTA as sequence.stats.v1 | sequence | 1,600 ms | 2,280 ms | 2,211,405 ms | 1.43× | 86.1% | consistent |

## Data sources

- `generated-synthetic` (generated): scripts/benchmark/inputs/generate-inputs.py --set synthetic — random.seed(42); matrix.tsv (2000x40), sets.tsv (20k x 4 sets with repeated occupancy masks), structure.pdb (3000 residues / 24k atoms), variants.vcf (200k records), genes.txt + associations.tsv (5k genes / 45k annotations). Digests in inputs.sha256; byte-identical on Windows/Python 3.13 and Linux/Python 3.14.
- `generated-engine` (generated): scripts/benchmark/inputs/generate-inputs.py --set sequence|reads — big.fa via the engine simulator (--count 20 --length 5000000 --seed 42); reads200.fq via simulate reads on big.fa (--read-length 150 --coverage 100 --seed 42). Digests in inputs.sha256.
- `generated-granularity` (generated): scripts/benchmark/inputs/generate-inputs.py --set granularity — numpy default_rng(42); the same ~100 Mb of FASTA as 1x100 Mb, 20x5 Mb and 100k x 1 kb. Digests in inputs.sha256.

## Reports

- `sequence.stats.v1.benchmark.json` (sequence.stats.v1)
- `expression.pca.v1.benchmark.json` (expression.pca.v1)
- `set.venn.v1.benchmark.json` (set.venn.v1)
- `enrichment.overrepresentation.v1.benchmark.json` (enrichment.overrepresentation.v1)
- `structure.pdb.summary.v1.benchmark.json` (structure.pdb.summary.v1)
- `variant.stats.v1.benchmark.json` (variant.stats.v1)
- `fastq.qc.v1.benchmark.json` (fastq.qc.v1)
- `sequence.ssr.v1.benchmark.json` (sequence.ssr.v1)
