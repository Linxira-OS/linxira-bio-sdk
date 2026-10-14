# Benchmark summary `bench-20261013-001` — 2026-10-13

> The locked-environment re-run of the 2026-10-11 window on the same host and inputs. The earlier numbers were measured while the interpreter environment did not match requirements.lock (biopython 1.88 / numpy 2.5.4 installed against 1.85 / 2.2.4 locked), a contradiction this repository now gates in CI; here the python arm runs inside a virtualenv installed from the lock. The archive carries code-label.txt: branch, full commit, `git describe --tags --always --dirty` (the short commit hash, because the project has published no release yet), the lock digest, the tree cleanliness and the interpreter versions actually used. Ecosystem-tool arms run in the same session and are archived under tool-arms/.

## Environment

- os: `linux`
- distro: `CachyOS`
- kernel: `7.2.9-1-cachyos`
- cpu_model: `Intel(R) Pentium(R) Gold G7400 (2 cores / 4 threads; cpufreq governor fixed at powersave for the whole window)`
- total_memory_mb: `15756`
- engine_version: `1.1.3`
- python_version: `Python 3.12.15`
- r_version: `R version 4.6.1 (2026-06-24) -- "Happy Hop"`
- precision: `high`
- code_revision: `f3b9f17a13ee79b5875f8ff1fc17b85050f2200b`

## Method

- backends_flag: --backends rust,python,r
- repeat_flag: --repeat 10
- warmup: one untimed warmup per backend before the timed repeats; the warmup is excluded from the report, which holds exactly 10 timed runs (run_index 1..10)
- page_cache: warm
- disclosure: Serial execution with no concurrent build, lint, archiving or unrelated job; the driver refuses to start when a CI job is pending or running, and, new in this window, when any unrelated process holds more than 5% CPU (the first attempt refused on a bulk downloader started outside the window), unless ALLOW_LOAD=1 records the contention. CPU governor fixed at powersave for the whole window; inputs staged on tmpfs so no disk latency enters the measurement; backend order fixed at rust->python->r, so cross-backend frequency/thermal drift is a known limitation. Ratios hold for this machine and these inputs only.
- code_label: branch main at the commit printed in code-label.txt; the binary embeds the same branch, commit and describe label in every report's environment block, so a report cannot be separated from the code that produced it.

## Results

| capability | dataset | class | rust | python | r | speedup | memory saving | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `sequence.stats.v1` | 20 x 5 Mb FASTA (~100 MB), engine-simulated | sequence | 810 ms | 1,845 ms | 1,810 ms | 2.28× | 90.8% | consistent |
| `expression.pca.v1` | 2000 x 40 count matrix (4 groups x 10 samples) | matrix | 20 ms | 3,250 ms | 2,745 ms | 162.50× | 82.0% | consistent |
| `set.venn.v1` | 20k identifiers x 4 sets (repeated occupancy masks) | other | 10 ms | 150 ms | 11,035 ms | 15.00× | 79.8% | consistent |
| `enrichment.overrepresentation.v1` | 5k genes / 45k gene-term associations across 300 terms | annotation | 30 ms | 200 ms | 4,425 ms | 6.67× | 72.9% | consistent |
| `structure.pdb.summary.v1` | 3000 residues / 24k atoms synthetic PDB with pLDDT column | structure | 30 ms | 700 ms | 20,555 ms | 23.33× | 55.2% | consistent |
| `variant.stats.v1` | 200k VCF records across 5 contigs (SNV/insertion/deletion mix) | other | 80 ms | 710 ms | 13,455 ms | 8.88× | 84.4% | consistent |
| `fastq.qc.v1` | 200 MB / 646k reads (150 bp, coverage 100) | other | 1,560 ms | 50,910 ms | 699,795 ms | 32.63× | 85.1% | consistent |
| `sequence.ssr.v1` | same ~100 MB FASTA as sequence.stats.v1 | sequence | 1,590 ms | 2,330 ms | 2,200,800 ms | 1.47× | 85.5% | consistent |

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
