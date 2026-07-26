# Residue Contact Map

## Purpose

Find representative-atom residue contacts within a distance cutoff in local PDB or mmCIF coordinates.

## Inputs

One plain, gzip, or BGZF PDB or mmCIF coordinate file.

## Parameters

Use `--cutoff` in angstroms, `--atom` for the representative atom, and `--intra-chain-only` to exclude inter-chain contacts.

## Outputs

Returns the first model, representative-residue count, contact count, identities, and distances.

## Examples

```bash
linxira-bio structure contact-map structure.cif --cutoff 8 --atom CA --json
```

## Interpretation

The default is CA within 8 angstroms including inter-chain contacts; P can be selected for nucleic acids.

## Caveats

This is a geometric contact definition, not evidence of a chemical bond or biological interaction.

## Runtime Dependencies

Local Rust only; no Python, R, Java, network, or external executable is required.

## Citations

Distance is Euclidean distance between selected representative atoms.

## Troubleshooting

Choose an atom name present in the structure; result size is capped at 1,000,000 contacts.
