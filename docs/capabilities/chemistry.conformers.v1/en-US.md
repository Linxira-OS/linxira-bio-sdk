# Conformer Generation

## Purpose

Generate 3D conformers for SDF molecules with RDKit ETKDGv3 embedding at a
fixed seed and minimise every conformer with MMFF94, producing the
structure ensemble that docking and shape comparison consume.

## Inputs

One SDF file (multi-record supported). Records may be 2D; hydrogens are
added before embedding, and the output carries explicit hydrogens as 3D
structures require.

## Parameters

`--num-conformers` (default 10, range 1-1000), `--seed` (default 42; the
fixed seed makes every run byte-identical), and optional `--rms-prune F`
(Angstroms; conformers within the threshold of an earlier one are dropped,
so the returned count can be lower than requested).

## Outputs

A multi-record SDF with one record per conformer carrying `_ConformerIndex`
and `mmff_energy_kcal_mol` properties. The JSON result envelope summarises
per-molecule conformer counts and MMFF94 energy ranges in kcal/mol, plus the
artifact path, size, and SHA-256.

## Examples

```bash
linxira-bio chemistry conformers ligands.sdf ligand-conformers.sdf --num-conformers 20 --seed 42 --json
```

## Interpretation

Energies are MMFF94 force-field values in kcal/mol, useful for ranking
conformers of one molecule — not for comparing different molecules. The
lowest-energy conformer is not necessarily the bioactive pose; use docking
to select among conformers. A wide energy spread with a prune threshold
indicates a flexible molecule whose ensemble matters.

## Caveats

Requires the pinned RDKit Python 3.12 environment selected by the worker
(`LINXIRA_BIO_WORKFLOW_PYTHON`); the pack never mutates a global
environment. Molecules whose atom types lack MMFF94 parameters fail with a
structured error instead of silently skipping minimisation. Reproducibility
depends on the seed: changing it changes the ensemble.

## Runtime Dependencies

The pinned Python 3.12 environment from the pack `requirements.lock`
(RDKit 2026.3.5, NumPy 2.5.2; installation gated by the environment
capability). No native tool is invoked.

## Citations

When reporting conformer ensembles, cite Ebejer et al. 2012 (J. Chem. Inf.
Model. 52:1146-1158) for ETKDG and Halgren 1996 (J. Comput. Chem.
17:490-519) for MMFF94, plus the RDKit citations in the descriptors
capability documentation.

## Troubleshooting

"MMFF94 has no parameters" means the molecule contains atom types the force
field does not cover (often metals); such inputs cannot be minimised. Zero
conformers despite a valid molecule usually means the embedding window was
too constrained — retry without `--rms-prune` or with a different seed.
