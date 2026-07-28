---
name: transform-phylogenetic-trees
description: Infer local maximum-likelihood trees with IQ-TREE or normalize, relabel, summarize, and reroot existing Newick trees. Use for alignment-based phylogenetic inference, Newick validation, canonical serialization, label mapping, branch-length summaries, or outgroup rerooting.
---

# Transform Phylogenetic Trees

Infer a tree from an alignment with `phylogeny.iqtree.v1`:

```bash
linxira-bio phylogeny iqtree alignment.fa tree.nwk --model MFP --threads 4 --seed 1 --json
```

Worker v2 uses role `alignment`. Preserve the model and seed; this version does
not claim bootstrap analysis.

Inspect the imported tree before execution. Use `phylogeny.tree.transform.v1`; do not replace it with an ad hoc text substitution script.

## Execute

```bash
cargo run -p linxira-bio-cli -- phylogeny tree INPUT.nwk OUTPUT.nwk --reroot OUTGROUP --label-map labels.tsv --json
```

The CLI label map is a two-column tab-separated file containing old and new labels. For worker schema v2, use input role `tree`, required parameter `output`, optional string `reroot_label`, and optional string-to-string object `label_map`.

## Interpret and validate

- Preserve quoted labels, comments, topology, and finite non-negative branch lengths while producing normalized Newick.
- Require the reroot label to identify exactly one leaf. The reroot edge length is divided equally across the new root branches.
- Reject label maps that create duplicate leaf labels.
- Report leaf count, internal-node count, maximum depth, total branch length, reroot state, and relabeled count.
- Never overwrite an existing output path.

## Limits

Execution is local CPU and accepts at most 128 MiB of decompressed text and 1,000,000 nodes. It does not infer trees, align sequences, render a publication figure, or support multi-leaf clade rerooting.
