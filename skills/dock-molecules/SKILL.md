---
name: dock-molecules
description: Dock a prepared small-molecule ligand into a prepared receptor with local AutoDock Vina. Use for binding-affinity scoring, pose prediction, search-box specification, seeded reproducible docking runs, and interpreting Vina kcal/mol pose tables. Requires PDBQT inputs; ligand/receptor preparation is a documented prerequisite, not part of the capability.
---

# Dock Molecules

Run one controlled native AutoDock Vina docking job locally.

## Run
1. Inspect inputs with `linxira-bio dataset inspect <receptor.pdbqt> --json` and
   the ligand likewise; require detected format `pdbqt`.
2. Run `linxira-bio chemistry dock <receptor.pdbqt> <ligand.pdbqt>
   <output.pdbqt> --center-x F --center-y F --center-z F --size-x F --size-y F
   --size-z F --seed N --exhaustiveness 8 --num-modes 9 --cpu N --json`.
3. Preserve the capability version, input hash, command, warnings, and result.

The search box (center plus edge lengths, in Angstroms) is required and must
come from biological context: a co-crystallized ligand, a known catalytic
residue, or an alphafold pocket prediction. Passing `--seed` makes the run
reproducible; omit it only when surveying seed sensitivity.

Input preparation happens before this capability. Prepare the ligand from SDF
or SMILES with Meeko (`mk_prepare_ligand.py -i ligand.sdf -o ligand.pdbqt`)
and the receptor from a cleaned, protonated PDB with the AutoDockTools
receptor workflow or `prepare_receptor`. Both `vina` and `mk_prepare_ligand.py`
are probed by the environment audit.

## Validate And Interpret

- Report `best_affinity_kcal_per_mol` (rank 1) with the full pose table; do
  not cherry-pick a lower-ranked pose.
- Vina scores are relative, not absolute: compare poses of the same ligand in
  the same box, or ligands docked under identical settings. A 1.4-fold change
  in Ki per kcal/mol is the thermodynamic conversion, but the scoring function
  is empirical — report kcal/mol, never a measured binding constant.
- The docked output PDBQT contains all reported poses in rank order; the JSON
  `docking.modes` array mirrors it.
- Treat a missing pose table (warning `no pose table was parsed`) as a failed
  run even if the exit status was success.

Stop on non-finite box coordinates, non-positive box edges, or inputs that are
not PDBQT. Do not use docking scores for clinical or toxicity decisions.

## Notes

- Vina never writes intermediate files outside the declared output path; a
  partially written output is removed on failure.
- Research use: docking predicts poses and relative scores only; experimental
  validation is always required before any downstream claim.
