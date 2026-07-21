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
2. Inspect formats from file contents or validated metadata; do not rely only on
   file extensions.
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

- FASTA, FASTQ, sequence manipulation, and assembly statistics: sequence and
  read-QC skills.
- BED, GFF, GTF, coverage, and coordinate joins: genome-interval skills.
- SAM, BAM, CRAM, pileup, and mapping QC: alignment-file skills.
- VCF, BCF, variants, genotypes, and normalization: variant skills.
- Counts, expression, sparse matrices, RNA-seq, and single-cell data:
  expression skills.
- PDB, mmCIF, PAE, pLDDT, structure comparison, and prediction:
  structural-biology skills.
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
