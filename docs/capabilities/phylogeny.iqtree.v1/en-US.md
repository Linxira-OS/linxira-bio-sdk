# IQ-TREE Phylogenetic Inference

## Purpose

Infer a local maximum-likelihood phylogeny from a multiple-sequence alignment.

## Inputs

Provide a local multiple-sequence alignment.

## Parameters

Set thread count, a model expression such as `MFP`, and a deterministic seed.

## Outputs

The capability isolates IQ-TREE working files and copies the resulting
`.treefile` to the requested Newick output.

```bash
linxira-bio phylogeny iqtree alignment.fa tree.nwk --model MFP --threads 4 --seed 1 --json
```

## Examples

The command above performs model finding and maximum-likelihood inference.

## Interpretation

Interpret topology and branch lengths with the selected model and sampling design.

## Caveats

Model selection and bootstrap support are distinct scientific decisions. This
version returns the inferred tree and does not claim bootstrap analysis.

## Runtime Dependencies

Requires IQ-TREE as `iqtree2`, or `LINXIRA_BIO_IQTREE`.

## Citations

Cite IQ-TREE, its version, the selected model, seed, and input alignment method.

## Troubleshooting

Audit `iqtree`; configure `LINXIRA_BIO_IQTREE` when the executable is outside `PATH`.
