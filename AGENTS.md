# Linxira Bio SDK

This repository builds executable, local-first bioinformatics capabilities and
the agent skills that select and validate them.

## Skill Routing

- Start biological analysis requests with `skills/route-bio-analysis/SKILL.md`.
- Read `skills/select-bio-execution/SKILL.md` when local, GPU, HPC, cloud, or
  browser execution must be selected.
- Read `skills/analyze-sequence-statistics/SKILL.md` for the implemented
  `sequence.stats.v1` capability.
- Read `skills/find-sequence-repeats/SKILL.md` for the implemented
  SSR/microsatellite mining capability (`sequence.ssr.v1`, rust/pytrf/
  Biostrings backends).
- Read `skills/manipulate-biological-sequences/SKILL.md` for the implemented
  `sequence.extract.v1`, `sequence.filter.v1`,
  `sequence.reverse-complement.v1`, `sequence.translate.v1`, and
  `sequence.orf.v1`, `sequence.id.normalize.v1`, `sequence.merge.v1`,
  `sequence.split.v1`, `sequence.to-table.v1`, and
  `sequence.from-table.v1`, `sequence.kmer.count.v1`, `sequence.consensus.v1`,
  `sequence.shuffle.v1`, `sequence.convert.biopython.v1`, and
  `primer.epcr.v1` capabilities.
- Read `skills/analyze-fastq-quality/SKILL.md` for the implemented
  `fastq.qc.v1` capability.
- Read `skills/process-fastq-reads/SKILL.md` for the implemented
  `fastq.trim.v1`, `fastq.adapter.v1`, and `fastq.deduplicate.v1`
  capabilities.
- Read `skills/analyze-sam-quality/SKILL.md` for the implemented SAM-text
  `alignment.qc.v1`, native BAM/CRAM quality, coverage, fixed-window depth
  (`alignment.window-depth.v1`), duplicate marking (`alignment.markdup.v1`),
  and short-read alignment capabilities.
- Read `skills/analyze-genome-annotations/SKILL.md` for implemented GFF3/GTF
  statistics, normalization, position-table, gene-density, and
  reference-guided extraction.
- Read `skills/analyze-sequence-similarity/SKILL.md` for implemented local
  BLAST+, DIAMOND, HMMER, BLAST result parsing, and reciprocal best-hit
  analysis.
- Read `skills/analyze-metagenomics/SKILL.md` for the implemented local
  Kraken2 taxonomic classification and abundance-table capability
  (`metagenomics.classify.v1`).
- Read `skills/analyze-pharmacogenomics/SKILL.md` for the implemented local
  PGx star-allele interpretation capability (`medical.pharmacogenomics.v1`).
- Read `skills/analyze-spatial-transcriptomics/SKILL.md` for the implemented
  local 10x count-matrix summary capability
  (`medical.spatial-transcriptomics.v1`).
- Read `skills/analyze-microbiome-diversity/SKILL.md` for the implemented
  local Kraken2 microbiome alpha-diversity capability
  (`medical.microbiome.v1`).
- Read `skills/analyze-survival-data/SKILL.md` for the implemented
  research-use-only Cox survival-analysis workflow (`medical.survival.v1`).
- Read `skills/analyze-molecular-descriptors/SKILL.md` for the implemented
  RDKit molecular-descriptor workflow (`chemistry.descriptors.v1`) and the
  seeded ETKDGv3 conformer workflow (`chemistry.conformers.v1`).
- Read `skills/dock-molecules/SKILL.md` for the implemented local AutoDock
  Vina ligand-receptor docking capability (`chemistry.dock.v1`).
- Read `skills/analyze-metabolomics-peaks/SKILL.md` for the implemented
  local mzML parsing and centroid peak detection
  (`medical.metabolomics.v1`).
- Read `skills/align-biological-sequences/SKILL.md` for implemented MUSCLE 5
  multiple-sequence alignment and trimAl trimming.
- Read `skills/analyze-sequence-motifs/SKILL.md` for implemented local MEME
  de novo motif discovery.
- Read `skills/analyze-functional-enrichment/SKILL.md` for implemented GO and
  eggNOG annotation normalization plus custom, GO, and KEGG
  over-representation analysis and preranked GSEA.
- Read `skills/intersect-genomic-intervals/SKILL.md` for the implemented BED
  `interval.intersect.v1`, `interval.merge.v1`, `interval.subtract.v1`, and
  `interval.closest.v1` capabilities.
- Read `skills/annotate-peaks/SKILL.md` for the implemented
  `peak.annotate.v1` capability (nearest-feature peak annotation).
- Read `skills/simulate-bio-data/SKILL.md` for the implemented
  `simulate.sequence.v1` and `simulate.reads.v1` seeded data-synthesis
  capabilities (benchmark and GPU-consistency fuel).
- Read `skills/analyze-expression-matrix/SKILL.md` for implemented CSV/TSV
  matrix QC, normalization, PCA, sample/feature clustering, and native
  clustered-heatmap preparation.
- Read `skills/quantify-expression/SKILL.md` for the implemented
  `expression.quantify.v1` capability (native salmon quant orchestration
  with quant.sf cross-check summaries).
- Read `skills/import-npz-matrix/SKILL.md` for the implemented
  `matrix.from-npz.v1` npz-to-CSV/TSV matrix import capability.
- Read `skills/fit-dose-response-curves/SKILL.md` for the implemented
  `curve.fit.v1` capability (IC50/ELISA 4PL, Michaelis-Menten,
  Lineweaver-Burk fitting).
- Read `skills/analyze-differential-expression/SKILL.md` for implemented local
  bulk RNA-seq differential expression and its research-use-only medical
  entrypoint.
- Read `skills/analyze-comparative-genomics/SKILL.md` for implemented local
  collinearity inference, Ka/Ks estimation, and synteny visualization.
- Read `skills/analyze-set-overlaps/SKILL.md` for implemented exact Venn and
  UpSet analysis of biological identifier-set tables.
- Read `skills/analyze-protein-properties/SKILL.md` for implemented local
  protein FASTA physicochemical properties.
- Read `skills/analyze-protein-domains/SKILL.md` for implemented InterProScan
  TSV and HMMER domtblout parsing.
- Read `skills/transform-phylogenetic-trees/SKILL.md` for implemented Newick
  normalization, label mapping, and single-leaf rerooting.
- Read `skills/manipulate-bio-tables/SKILL.md` for the implemented CSV/TSV
  `table.manipulate.v1` capability.
- Read `skills/call-variants/SKILL.md` for the implemented
  `variant.call.v1` capability (native bcftools calling).
- Read `skills/analyze-variant-statistics/SKILL.md` for the implemented
  `variant.stats.v1`, `variant.filter.v1`, `variant.normalize.v1`, and
  `variant.compare.v1` capabilities.
- Read `skills/analyze-pdb-structure/SKILL.md` for the implemented
  `structure.pdb.summary.v1` capability and explicit AlphaFold pLDDT handling.
- Read `skills/analyze-coordinate-structures/SKILL.md` for implemented local
  mmCIF summaries, coordinate-derived sequences, residue contacts, geometry,
  and identity-matched structure superposition.
- Read `skills/visualize-bio-results/SKILL.md` for implemented annotation,
  enrichment, and protein-domain SVG plots plus the native interactive
  PDB/mmCIF structure viewer.
- Read `skills/render-bio-plots/SKILL.md` for the implemented
  `plot.render.v1` capability (deterministic matplotlib/ggplot2 figure
  rendering from a PlotSpec).
- Read `skills/inspect-bio-dataset/SKILL.md` before analyzing imported data.
- Read `skills/export-bio-table/SKILL.md` to export supported result tables.
- Read `skills/configure-bio-environment/SKILL.md` to audit managed Python, R,
  Java, Conda/Bioconda, BLAST, DIAMOND, native command-line tools, WSL Debian,
  WSL Arch, Docker, Podman, or GPU prerequisites.
- Read `skills/run-bio-benchmark/SKILL.md` for the implemented
  `benchmark.run.v1` capability.
- Do not use a capability marked `planned` as though it were available.

## Documentation Map

Paths and purposes, for agents working in this repository (not end-user docs):

- `README.md` — product entry point (English-first, Chinese mirror below).
- `AGENTS.md` — this file: agent routing, repository rules, validation gates.
- `ROADMAP.md` — milestones and the closed capability-gap list (§11: register
  a capability there before implementing it).
- `docs/capabilities/<id>/{en-US,zh-CN}.md` — canonical bilingual capability
  pages (10 fixed sections each; embedded in the GUI at build time).
- `docs/engine-evals/` — engineering experiment ledgers (GPU kernels, engine
  evaluations, vendor trials); committed together with the code that produced
  them. Measurement: median of >=3 runs, GPU phases split.
- `docs/*.md` (AI_AND_SDK, EXECUTION_POLICY, DATA_FORMATS, …) — engineering
  policies and reference notes.
- `benchmark-results/` — benchmark archives and the approved blog mirror;
  public articles are written only after a quantified test the user approved.
- `skills/<name>/SKILL.md` + `agents/openai.yaml` — agent skill body and its
  registration-required interface file.
- `skill-pack.json` — the import boundary for agent runtimes; every skill
  folder must appear here with its capability ids, otherwise it ships in the
  bundle but stays invisible to agents.
- `gpu-lab/` — standalone (workspace-excluded) Rust GPU experiment carrier
  (probe / bench / gpu-bench); not part of the release workspace.

## Repository Rules

- Treat `skills/` as concise agent-facing procedures, not a place for shared
  implementation code.
- Put deterministic shared computation in `engine/` and expose it through a
  versioned capability, CLI command, result schema, and test fixture.
- Keep `.research/` source clones untracked and do not edit upstream bodies.
- Record provenance before adapting an upstream method or example.
- Prefer maintained native tools over reimplementing mature algorithms.
- Add C++ only after a benchmark identifies a kernel that Rust or an existing
  native dependency does not handle adequately.

## Execution Safety

- Execute locally by default.
- Require explicit approval before provisioning cloud resources, incurring
  cost, uploading data, or opening authenticated browser services.
- Never automate password entry, MFA, CAPTCHA, or acceptance of service terms.
- Keep protected, clinical, and controlled-access data out of public services
  unless the user supplies an approved data-governance path.

## Validation

Run these checks before reporting a capability complete:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p linxira-bio-cli -- sequence stats tests/fixtures/sequences/tiny.fa --json
```

Use the skill creator validator for every changed folder under `skills/`.
