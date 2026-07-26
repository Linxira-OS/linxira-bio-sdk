# Coordinate-Derived Polymer Sequence

## Purpose

Extract polymer sequences from residue identities present in local PDB or mmCIF coordinates.

## Inputs

One plain, gzip, or BGZF PDB or mmCIF coordinate file.

## Parameters

Provide the input path. Add `--json` for the standard result envelope.

## Outputs

Returns the selected model, chain polymer type, residue counts, sequences, and warnings.

## Examples

```bash
linxira-bio structure sequence structure.pdb --json
```

## Interpretation

Sequences represent residues with retained coordinate records in the first model.

## Caveats

Missing or unresolved residues can be absent; this is not automatically the complete reference sequence.

## Runtime Dependencies

Local Rust only; no Python, R, Java, network, or external executable is required.

## Citations

Residue-name mapping follows standard protein and nucleic-acid coordinate conventions.

## Troubleshooting

Inspect warnings for unknown residues and confirm the input contains polymer atom records.
