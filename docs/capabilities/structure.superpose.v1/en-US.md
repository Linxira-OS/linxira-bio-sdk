# Identity-Matched Structure Superposition

## Purpose

Rigidly superpose two local PDB/mmCIF structures using atoms with matching coordinate identities.

## Inputs

A reference structure and a mobile structure in PDB or mmCIF format.

## Parameters

Use `--atom` to choose the matched atom name; the default is CA.

## Outputs

Returns matched-atom count, RMSD before and after fitting, rotation matrix, translation, and warnings.

## Examples

```bash
linxira-bio structure superpose reference.pdb mobile.pdb --atom CA --json
```

## Interpretation

Atoms match by chain ID, residue ID, and atom name; lower post-fit RMSD means those matched coordinates fit more closely.

## Caveats

No sequence alignment, residue correspondence search, flexible fitting, symmetry search, or fold classification is performed.

## Runtime Dependencies

Local Rust only; no Python, R, Java, network, or external executable is required.

## Citations

Rigid fitting uses a least-squares rotation and translation over identity-matched coordinates.

## Troubleshooting

Ensure both files share at least three non-collinear matching atoms with the selected atom name.
