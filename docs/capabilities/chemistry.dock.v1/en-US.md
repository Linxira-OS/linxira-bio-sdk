# Molecular Docking

## Purpose

Dock a prepared small-molecule ligand into a prepared receptor with native
AutoDock Vina, producing posed ligand conformations and a ranked
binding-affinity table for local structure-based analysis.

## Inputs

Two PDBQT files: a rigid receptor and a flexible-or-rigid ligand. Preparation
is a prerequisite, not part of this capability — prepare ligands with Meeko
(`mk_prepare_ligand.py`) and receptors with the AutoDockTools workflow before
docking.

## Parameters

The search box is required: `--center-x/--center-y/--center-z` and
`--size-x/--size-y/--size-z` (Angstroms; edges must be positive). Optional:
`--seed N` for reproducible runs, `--exhaustiveness` (default 8, higher is
slower and more thorough), `--num-modes` (default 9), `--cpu N` (default 1).

## Outputs

Writes the docked ligand PDBQT containing all reported poses in rank order.
JSON reports the native-tool provenance plus the parsed pose table: rank,
affinity in kcal/mol, and RMSD lower/upper bounds per mode, with the rank-1
affinity surfaced as `best_affinity_kcal_per_mol`.

## Examples

```bash
linxira-bio chemistry dock receptor.pdbqt ligand.pdbqt docked.pdbqt \
  --center-x 11.7 --center-y -4.5 --center-z 0.25 \
  --size-x 20 --size-y 20 --size-z 20 --seed 42 --json
```

## Interpretation

Vina scores are empirical and relative: valid comparisons are poses of one
ligand in one box, or ligands docked under identical settings. Report the
rank-1 affinity with the full table; lower-ranked poses describe the energy
landscape, not alternatives of equal standing. RMSD bounds measure distance
from the best mode, not experimental truth.

## Caveats

Scores are not measured binding constants and must not be read as Ki or Kd.
Water molecules, protonation states, and receptor flexibility are outside
this fixed-receptor workflow. A run whose stdout contains no pose table
yields a warning and an empty `docking.modes` — treat it as a failure to
interpret, not a zero-affinity result.

## Runtime Dependencies

The native `vina` executable (environment audit category molecular-docking;
bioconda `autodock-vina`; overridable via `LINXIRA_BIO_VINA`). Meeko
(bioconda `meeko`) is probed for the preparation prerequisite. No Python, R,
or Java runtime is used by the docking step itself.

## Citations

When reporting docking results, cite Trott & Olson 2010 (J. Comput. Chem.
31:455-461) and Eberhardt et al. 2021 (J. Chem. Inf. Model. 61:3891-3898)
for AutoDock Vina 1.2.x.

## Troubleshooting

Run the environment audit first to confirm `vina` is installed. "File cannot
be opened" errors almost always mean the input is not PDBQT. If Vina exits
successfully but no pose table appears, rerun with a smaller box or higher
exhaustiveness and inspect the raw log the warning points to.
