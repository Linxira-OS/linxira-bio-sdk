---
name: analyze-molecular-descriptors
description: Compute RDKit physicochemical descriptors (molecular weight, CLogP, TPSA, H-bond counts, rotatable bonds, rings, formal charge, formula) for SDF molecule records.
---

# Analyze Molecular Descriptors

Inspect imported files before execution. Use the RDKit workflow pack; do not
reimplement descriptor computation in Rust or plain Python.

## Choose a capability

- Use `chemistry.descriptors.v1` with an SDF file to produce a TSV descriptor
  table and per-molecule JSON rows.

## Execute

```bash
linxira-bio chemistry descriptors MOLECULES.sdf DESCRIPTORS.tsv --json
```

## Interpret

Report composition (molecular weight, formula), lipophilicity (CLogP),
polarity/permeability tendencies (TPSA, HBD/HBA), and flexibility/rigidity
(rotatable bonds, ring counts). Note the RDKit version dependency and that
values are standard RDKit defaults.

## Caveats

Requires the pinned RDKit Python 3.12 environment (`requirements.lock`).
Unparseable SDF records produce a structured error.
