---
name: analyze-comparative-genomics
description: Run and validate implemented local comparative-genomics capabilities for genome collinearity inference, codon-alignment Ka/Ks estimation, and dual, multiple, micro, or circular synteny visualization. Use for ordered gene-position plus similarity-hit tables, AXT codon alignments, or synteny anchor TSV files.
---

# Analyze Comparative Genomics

Use the versioned capabilities instead of generating replacement analysis code.

## Run

1. Inspect every input with `linxira-bio dataset inspect <path> --json`.
2. Select one operation:
   - infer collinear blocks: `linxira-bio comparative mcscanx <genes.tsv> <hits.blast> <output.collinearity> --json`;
   - estimate Ka/Ks: `linxira-bio comparative kaks <pairs.axt> <output.tsv> <NG|LWL|LPB|YN> --json`;
   - render supplied anchors: `linxira-bio comparative synteny-plot <anchors.tsv> <output.svg> --style dual|multiple|micro|circular --json`.
3. Run `linxira-bio environment audit --json` before native execution. MCScanX and KaKs Calculator must be available on `PATH` or through their documented executable environment variables.
4. Preserve input hashes, native-tool versions, parameters, diagnostics, and output paths.

For artifact-aware jobs, use `comparative.mcscanx.v1`, `comparative.kaks.v1`, or `comparative.synteny.visualize.v1`. Do not treat visualization as collinearity inference.

## Validate

- Require exact identifier agreement between gene positions and similarity hits.
- Require biologically valid codon-aligned AXT pairs before Ka/Ks estimation.
- Inspect representative blocks and alignments before evolutionary interpretation.
- Treat Ka/Ks as method- and alignment-dependent evidence; a ratio alone does not prove positive selection.
- Cite the native algorithm, version, similarity or alignment method, and estimator.
