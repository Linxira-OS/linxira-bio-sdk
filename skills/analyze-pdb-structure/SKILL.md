---
name: analyze-pdb-structure
description: Parse and validate local PDB coordinates with the Rust-native `structure.pdb.summary.v1` capability. Use for model, chain, residue, atom, element, coordinate-bound, B-factor, and render-ready atom summaries, or when an AlphaFold-produced PDB should explicitly interpret B-factors as per-residue pLDDT.
---

# Analyze PDB Structure

Use the tested local parser instead of writing a one-off PDB reader.

## Run

1. Confirm with `inspect-bio-dataset` that the input is a local PDB file.
2. Run the installed CLI:

```bash
linxira-bio structure pdb INPUT.pdb --json
```

When developing inside the source repository, run:

```bash
cargo run -p linxira-bio-cli -- structure pdb INPUT.pdb --json
```

3. Add `--alphafold-plddt` only when provenance establishes that the producer
   stored AlphaFold pLDDT values in the PDB B-factor columns.
4. Preserve `structure.pdb.summary.v1`, the input hash, warnings, and JSON
   result. Reject an error result.

## Interpret

- Use `atoms[].position`, `element`, `model_id`, and `residue_index` as stable
  input for later local 3D rendering.
- Use `bounds.center` and `bounds.span` to frame a structure.
- Treat blank chain IDs as valid empty strings.
- Treat `alphafold_confidence` and `residues[].plddt` as unavailable unless the
  explicit AlphaFold option was used.
- Apply the reported pLDDT bands to model confidence, not experimental accuracy
  or biological function.

This capability does not parse mmCIF or PAE, infer chemical bonds, render 3D
images, compare structures, or predict structures. For local viewing only,
open PDB or mmCIF coordinates in the native GUI; it infers display bonds and
can export the current view as PNG. Route analytical gaps to an approved
external workflow or report the planned capability gap.
