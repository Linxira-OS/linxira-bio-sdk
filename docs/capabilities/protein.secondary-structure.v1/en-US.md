# Protein Secondary-Structure Annotation

## Purpose

Run DSSP on a local PDB or mmCIF coordinate file to produce residue-level
secondary-structure annotation.

## Inputs

Provide one local PDB or mmCIF coordinate file.

## Parameters

This version has no scientific tuning parameters.

## Outputs

The capability writes DSSP text and controlled execution metadata.

```bash
linxira-bio protein secondary-structure model.cif model.dssp --json
```

## Examples

The command above writes residue-level DSSP text beside the source structure.

## Interpretation

Treat DSSP assignments as coordinate-derived annotations. Missing residues,
alternate conformations, and incomplete coordinates affect the result.

## Caveats

The wrapper does not repair missing coordinates or select biological assemblies.

## Runtime Dependencies

Requires `mkdssp` on `PATH` or `LINXIRA_BIO_MKDSSP`. No structure is uploaded.

## Citations

Cite DSSP, its version, and the coordinate structure identifier and release.

## Troubleshooting

Audit `mkdssp`; configure `LINXIRA_BIO_MKDSSP` when it is outside `PATH`.
