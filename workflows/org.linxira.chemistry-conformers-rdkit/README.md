# Chemistry Conformers (RDKit ETKDG)

Generates 3D conformers for SDF molecule records using RDKit ETKDGv3
embedding at a fixed random seed, then minimises every conformer with MMFF94.
Output is a multi-record SDF (one record per conformer, carrying
`_ConformerIndex` and `mmff_energy_kcal_mol` properties); the result envelope
summarises per-molecule conformer counts and MMFF energy ranges in kcal/mol.

Parameters: `num_conformers` (default 10, at most 1000), `seed` (default 42;
the fixed seed makes runs reproducible), and optional `rms_prune_threshold`
(Angstroms; drops conformers within the threshold of an earlier one).

Requires the pinned Python 3.12 environment from `requirements.lock`
(`pip install --require-hashes -r requirements.lock`). The pack is invoked
through the Linxira Bio worker; see
`docs/capabilities/chemistry.conformers.v1` for the capability documentation.
