---
name: analyze-molecular-descriptors
description: Compute RDKit physicochemical descriptors (molecular weight, CLogP, TPSA, H-bond counts, rotatable bonds, rings, formal charge, formula) for SDF molecule records, or generate seeded ETKDGv3 3D conformers with MMFF94 energies for docking preparation.
---

# Analyze Molecular Descriptors

Inspect imported files before execution. Use the RDKit workflow packs; do not
reimplement descriptor computation or conformer embedding in Rust or plain
Python.

## Choose a capability

- Use `chemistry.descriptors.v1` with an SDF file to produce a TSV descriptor
  table and per-molecule JSON rows.
- Use `chemistry.conformers.v1` with an SDF file to generate 3D conformers
  (ETKDGv3 embedding, fixed seed, MMFF94 minimisation) written as a
  multi-record SDF with `mmff_energy_kcal_mol` properties; this is the local
  ligand-preparation step before `chemistry.dock.v1` (Meeko then converts the
  chosen conformer to PDBQT).

## Execute

```bash
linxira-bio chemistry descriptors MOLECULES.sdf DESCRIPTORS.tsv --json
linxira-bio chemistry conformers MOLECULES.sdf CONFORMERS.sdf --num-conformers 10 --seed 42 --json
```

Conformer parameters: `--num-conformers` (1-1000, default 10), `--seed`
(default 42; keep it fixed for reproducibility), `--rms-prune F` (Angstroms;
drops conformers within the threshold of an earlier one, so the returned
count can be lower than requested).

## Interpret

Report composition (molecular weight, formula), lipophilicity (CLogP),
polarity/permeability tendencies (TPSA, HBD/HBA), and flexibility/rigidity
(rotatable bonds, ring counts). Note the RDKit version dependency and that
values are standard RDKit defaults.

For conformers, report the per-molecule conformer count together with the
MMFF94 energy range in kcal/mol; the lowest-energy conformer is not
necessarily the bioactive pose — docking, not energetics, selects poses.

## Caveats

Requires the pinned RDKit Python 3.12 environment (`requirements.lock`).
Unparseable SDF records produce a structured error. Molecules whose atom
types lack MMFF94 parameters fail conformer generation with a structured
error rather than silently skipping minimisation.
