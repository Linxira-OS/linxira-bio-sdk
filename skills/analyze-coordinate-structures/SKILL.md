---
name: analyze-coordinate-structures
description: Analyze local PDB or mmCIF coordinate files with native Linxira Bio capabilities for summaries, polymer sequence extraction, residue contacts, atom geometry, rigid superposition, and DSSP secondary-structure annotation. Use when an agent must inspect or compare coordinate structures without uploading data or writing replacement analysis code.
---

# Analyze Coordinate Structures

Inspect the dataset first, then select the narrowest available coordinate
capability. Execute locally; these operations require neither Python nor an
external molecular viewer.

## Select A Capability

- Use `protein.secondary-structure.v1` to run local `mkdssp` and write a DSSP
  residue-annotation artifact from PDB or mmCIF coordinates.

- Use `structure.mmcif.summary.v1` for model, chain, residue, and atom counts
  from mmCIF.
- Use `structure.sequence.extract.v1` for polymer sequences derived from PDB or
  mmCIF residue coordinates.
- Use `structure.contact-map.v1` for cutoff-based residue contacts.
- Use `structure.geometry.v1` for one distance, angle, or dihedral measurement.
- Use `structure.superpose.v1` for rigid superposition using atoms with matching
  chain IDs, residue IDs, and atom names.

## Run Locally

```bash
linxira-bio structure mmcif-summary structure.cif --json
linxira-bio structure sequence structure.pdb --json
linxira-bio structure contact-map structure.cif --cutoff 8 --atom CA --json
linxira-bio structure geometry structure.pdb \
  --atom A/1/N --atom A/1/CA --atom A/1/C --json
linxira-bio structure superpose reference.pdb mobile.pdb --atom CA --json
```

Use `MODEL/CHAIN/RESIDUE/ATOM` selectors when a file contains multiple models.
Two, three, or four geometry selectors request distance, angle, or dihedral
respectively. For nucleic-acid contact maps, request `--atom P` when phosphate
contacts are scientifically appropriate.

## Interpret Results

- Summary reports all parsed models; sequence, contact, geometry, and
  superposition use the first model unless selectors name a model.
- Alternate locations retain blank, `A`, or `1`; other conformers are excluded
  deterministically.
- Contact-map distances are in angstroms. The default representative atom is
  `CA`, the default cutoff is 8 angstroms, and inter-chain contacts are included.
- Superposition reports RMSD before and after rigid fitting. It proves only the
  fit of identity-matched atoms, not sequence or fold equivalence.
- Coordinate-derived sequences can omit residues absent from the coordinate
  records and must not be treated as a complete reference sequence automatically.

## Boundaries

- Accept at most 128 MiB of decompressed coordinate text and 100,000 atoms.
- Return at most 1,000,000 contacts.
- Do not claim biological-assembly expansion, sequence alignment, flexible
  fitting, ligand chemistry, bond-order inference, PAE interpretation, or
  structure prediction.
- Use `analyze-pdb-structure` for explicit AlphaFold PDB pLDDT interpretation.
- Use `select-bio-execution` before any remote, GPU, cloud, or authenticated
  browser workflow.

Preserve the capability ID, parameters, input hashes, warnings, and full result
envelope in the analysis record.
