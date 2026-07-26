---
name: route-bio-analysis
description: Route biological data-analysis requests to verified Linxira Bio capabilities and exact execution skills. Use when an agent must analyze biological files or plan a bioinformatics workflow, especially when the input format, scientific category, available implementation, or local-versus-remote execution path is not yet clear.
---

# Route Bio Analysis

Route the request to an available executable capability before writing analysis
code. Do not treat an indexed or planned capability as implemented.

## Route The Request

1. Identify the biological question, input files, expected output, and required
   scientific QC.
2. Run `inspect-bio-dataset` for local files so format detection uses content,
   compression signatures, and validated metadata rather than only extensions.
3. Run `linxira-bio capabilities --json` and select an `available` capability.
4. Read the exact execution skill for that capability.
5. Invoke `select-bio-execution` when resource, GPU, remote-data, cloud, or
   browser decisions are involved.
6. Preserve the command, capability version, input hashes, warnings, and output
   paths in the analysis record.
7. Validate the result scientifically before interpreting it.

If the required capability is only `planned`, use a maintained external tool
through an approved workflow or report the missing capability. Do not generate
a replacement script silently.

## Category Routing

- FASTA assembly statistics and FASTQ read QC/processing: use
  `analyze-sequence-statistics` for supported FASTA and
  `analyze-fastq-quality` for supported FASTQ QC. Use
  `process-fastq-reads` for FASTQ quality trimming and adapter removal. Use
  `manipulate-biological-sequences` for supported FASTA record extraction,
  coordinate extraction, filtering, reverse-complement generation,
  translation, ORF finding, ID normalization, merge/split, and FASTA-table
  conversion.
  Use the same sequence skill for exact k-mer counting and exact-match ePCR.
- BED interval operations: use `intersect-genomic-intervals` for overlap
  summaries, BED3 merge, and BED3 subtraction.
- GFF3/GTF annotation statistics, GTF-to-GFF3 normalization, feature position
  tables, and reference-guided gene/transcript/CDS/exon/UTR/promoter extraction:
  use `analyze-genome-annotations`. Annotation joins, closest-feature lookup,
  ontology validation, and automatic relationship repair remain planned.
- SAM text mapping QC: use `analyze-sam-quality`. BAM, CRAM, pileup, coverage,
  and advanced mapping reports still require a maintained native workflow.
- VCF, BCF, variants, genotypes, and normalization: use
  `analyze-variant-statistics` for supported VCF summaries, basic filters, and
  reference-guided biallelic small-variant normalization. BCF remains
  recognized but unsupported.
- Rectangular CSV/TSV expression matrix QC, CPM/log2-CPM/median-ratio
  normalization, PCA, deterministic sample/feature clustering, and native
  clustered heatmaps: use `analyze-expression-matrix`. Use
  `manipulate-bio-tables` for local CSV/TSV row filtering, column selection,
  column dropping, row skipping, row limiting, and delimiter conversion.
  Differential expression, sparse containers, complete RNA-seq workflows, and
  single-cell analysis remain external or planned.
- Biological identifier-set CSV/TSV tables: use `analyze-set-overlaps` for
  exact 2–6-set Venn regions or ranked 2–64-set UpSet intersections. Do not
  infer enrichment or significance from overlap counts alone.
- Protein FASTA physicochemical characterization: use
  `analyze-protein-properties` for composition, molecular weight, theoretical
  pI, charge, aromaticity, GRAVY, and extinction coefficients. Ambiguous
  sequences deliberately return null derived values.
- PDB coordinate summaries and explicitly requested AlphaFold pLDDT from PDB
  B-factors: use `analyze-pdb-structure`.
- PDB/mmCIF coordinate-derived sequence, residue contacts, atom geometry, and
  identity-matched superposition, plus mmCIF summaries: use
  `analyze-coordinate-structures`. The native GUI can preview coordinates in
  three representation modes and export the current view as PNG. PAE,
  sequence-aligned structural comparison, assembly expansion, and structure
  prediction remain planned.
- Missing Python, R, BLAST, DIAMOND, command-line tools, WSL, or GPU runtime:
  `configure-bio-environment` before the domain capability runs.
- Scheduler, cloud, GPU, or authenticated web service: execution selection
  before the domain skill runs.

## Boundaries

- Keep local execution as the default.
- Never turn a prediction or statistical association into a biological or
  clinical conclusion without the corresponding evidence and uncertainty.
- Require explicit approval before uploading data, provisioning resources,
  spending money, or using an authenticated browser service.
- Do not expose credentials, controlled data, or protected health information.
