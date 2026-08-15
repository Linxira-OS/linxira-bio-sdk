# Molecular Descriptors

## Purpose

Compute RDKit physicochemical descriptors for SDF molecule records: molecular weight, CLogP, TPSA, H-bond donors/acceptors, rotatable bonds, ring and aromatic-ring counts, formal charge, and molecular formula.

## Inputs

An SDF file with one or more molecule records (`$$$$` separated).

## Parameters

No parameters are required beyond the input and output paths.

## Outputs

A TSV descriptor table with one row per molecule (`molecule_index` plus the descriptor columns). JSON output reports `molecule_count`, `descriptor_names`, and the per-molecule rows.

## Examples

```bash
linxira-bio chemistry descriptors molecules.sdf descriptors.tsv --json
```

## Interpretation

Molecular weight and formula describe composition; CLogP estimates lipophilicity; TPSA and H-bond counts describe polarity and permeability tendencies; rotatable bonds and ring counts describe flexibility and rigidity. Values are RDKit defaults (no explicit hydrogen normalization beyond RDKit's standard parser).

## Caveats

Requires the pinned RDKit Python environment (`requirements.lock`, Python 3.12). Results depend on the RDKit version. Molecules that RDKit cannot parse produce a structured error.

## Runtime Dependencies

RDKit 2026.3.5 and NumPy 2.5.2 (Python 3.12) installed via the hashed `requirements.lock`.

## Citations

RDKit: Open-Source Cheminformatics Software (https://www.rdkit.org).

## Troubleshooting

If a molecule fails to parse, check the SDF record validity (atom and bond blocks). Confirm the pack environment is installed: `pip install --require-hashes -r workflows/org.linxira.chemistry-descriptors-rdkit/requirements.lock`.
