# mmCIF Structure Summary

## Purpose

Summarize local mmCIF atom-site coordinates deterministically without external tools.

## Inputs

One plain, gzip, or BGZF mmCIF file with a supported `_atom_site` loop.

## Parameters

Provide the input path. Add `--json` for the standard result envelope.

## Outputs

Returns model, chain, residue, atom, polymer-atom, and hetero-atom counts plus warnings.

## Examples

```bash
linxira-bio structure mmcif-summary structure.cif --json
```

## Interpretation

Counts cover every parsed model and retained alternate location in the file.

## Caveats

This capability does not expand biological assemblies, parse every mmCIF category, or infer chemistry.

## Runtime Dependencies

Local Rust only; no Python, R, Java, network, or external executable is required.

## Citations

Field interpretation follows the wwPDB PDBx/mmCIF atom-site data model.

## Troubleshooting

Confirm the file contains a tabular `_atom_site` loop and is below the decompressed input limit.
