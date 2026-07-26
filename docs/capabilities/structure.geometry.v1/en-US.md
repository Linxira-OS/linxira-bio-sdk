# Structure Geometry

## Purpose

Measure one atom distance, angle, or dihedral from local PDB or mmCIF coordinates.

## Inputs

One coordinate file and exactly two, three, or four atom selectors.

## Parameters

Use repeated `--atom CHAIN/RESIDUE/ATOM` or `--atom MODEL/CHAIN/RESIDUE/ATOM` selectors.

## Outputs

Returns the measurement type, selected atom identities, numeric value, and units.

## Examples

```bash
linxira-bio structure geometry structure.pdb --atom A/1/N --atom A/1/CA --atom A/1/C --json
```

## Interpretation

Two selectors produce angstrom distance; three produce degrees angle; four produce degrees dihedral.

## Caveats

The capability measures supplied coordinates and does not judge stereochemistry or bond validity.

## Runtime Dependencies

Local Rust only; no Python, R, Java, network, or external executable is required.

## Citations

Measurements use standard Euclidean vector angle and signed torsion formulas.

## Troubleshooting

Use explicit model selectors for multi-model files and verify every selector uniquely identifies an atom.
