# Capability Roadmap

## Capability Classes

| Class | Initial capabilities | Default mode | Native direction |
| --- | --- | --- | --- |
| Sequence I/O and QC | FASTA statistics, FASTQ QC, filtering, conversion | Local CPU | Rust |
| Sequence algorithms | motif scan, ORF, codon use, k-mer and sketch | Local CPU | Rust, SIMD if measured |
| Genome intervals | BED/GFF/GTF parsing, intersect, merge, coverage | Local CPU | Rust or maintained backend |
| Alignment files | SAM/BAM/CRAM QC, pileup, coverage | Local CPU | Rust with htslib/noodles |
| Variants | VCF/BCF QC, filtering, normalization, summaries | Local CPU | Rust with htslib/noodles |
| Expression | count QC, sparse matrix primitives, bulk RNA-seq workflow | Local/HPC | Rust plus maintained R/native tools |
| Single-cell and spatial | object QC, sparse operations, aggregation | Local/HPC/cloud by size | Rust/C++ kernels after profiling |
| Metagenomics | classification workflow and abundance validation | Local/HPC by database size | Wrap maintained native tools |
| Comparative genomics | alignment statistics, synteny and orthology workflows | Local/HPC | Native tools plus Rust validation |
| Phylogenetics | distance, tree QC, bootstrap summaries | Local/HPC | Rust/C++ only where justified |
| Proteomics/metabolomics | table QC and workflow adapters | Local/HPC | Rust tables plus native tools |
| Structural biology | PDB/mmCIF, PAE, pLDDT, RMSD, contact analysis | Local CPU/GPU | Rust/C++ |
| Structure prediction | AlphaFold 2 compatible execution | Local GPU/HPC/cloud | External model backend |
| Browser services | AlphaFold Server and account-bound tools | Authenticated browser | Gated connector |
| Reporting | manifests, provenance, validated result packages | Local CPU | Rust |

## Releases

### 0.1 Local Core

- capability catalog and doctor command;
- environment audit and platform-specific installation planning;
- FASTA sequence statistics;
- FASTQ quality control;
- VCF descriptive statistics;
- content-aware previews for BED, GFF3, GTF, SAM, and tabular data;
- signature-only recognition for BAM and other planned binary formats;
- local execution and provenance contracts.

### 0.2 SDK And Agent Tools

- explicitly approved environment installation with checksums and rollback;
- Python bindings;
- MCP server generated from capability contracts;
- Codex and OpenCode installation smoke tests;
- stable structured result schema.

### 0.3 Workflows

- bulk RNA-seq, variant, annotation, and metagenomics profiles;
- resumable workflow manifests;
- container and scheduler adapters.

### 0.4 Accelerated And Remote Execution

- benchmark-selected C++ or GPU kernels;
- AlphaFold 2 local/HPC adapter;
- approved cloud execution connectors;
- explicitly gated authenticated browser connectors.
