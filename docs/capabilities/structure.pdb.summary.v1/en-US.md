# PDB Structure Summary

## Purpose

Parse local PDB fixed-column coordinate records into a deterministic summary
and render-ready atom dataset without Python, Java, or an external executable.

## Inputs

One readable PDB file containing at least one `ATOM` or `HETATM` record. Plain
text, gzip, and BGZF are detected by content. mmCIF is not accepted.

## Parameters

The input path is required. `--alphafold-plddt` explicitly interprets polymer
atom B-factor values as AlphaFold pLDDT. `--json` returns the standard analysis
result envelope.

## Outputs

Returns model, chain, residue, polymer-atom, hetero-atom, and element counts;
coordinate bounds in angstroms; B-factor statistics; model and chain summaries;
indexed residues; and atom records with coordinates, occupancy, B-factor,
element, alternate location, and residue identity. Explicit AlphaFold mode also
returns residue pLDDT values and four confidence-band counts.

## Examples

```bash
linxira-bio structure pdb tests/fixtures/structure-pdb-summary/alphafold-style.pdb --alphafold-plddt --json
```

## Interpretation

`atoms[].residue_index` joins atoms to `residues[]`. `bounds.center` and
`bounds.span` support camera framing. AlphaFold confidence bands are very high
at 90 or above, confident at 70 to below 90, low at 50 to below 70, and very
low below 50. They describe model confidence, not experimental validation.

## Caveats

PDB content alone cannot prove that B-factor columns contain pLDDT, so the
capability never enables that interpretation automatically. It does not parse
mmCIF or PAE, infer bonds, expand biological assemblies, resolve alternate
locations, compare structures, render images, or predict structures.

The native GUI can separately load PDB/mmCIF coordinates, infer display-only
bonds, and export the current view as PNG. Those display behaviors are not
part of this analysis capability's result contract.

## Runtime Dependencies

This is a local Rust capability. It has no Python, R, Java, molecular viewer,
or external command-line dependency and adds no third-party package beyond the
already registered serialization and gzip dependencies.

## Citations

PDB column semantics follow the wwPDB legacy PDB format specification.
AlphaFold pLDDT interpretation follows the confidence convention documented
for AlphaFold outputs and is applied only when the caller confirms provenance.

## Troubleshooting

Use the reported line for malformed fixed-width records. Convert mmCIF with a
maintained structure tool before this capability, or retain mmCIF for a future
native parser. Do not use `--alphafold-plddt` for crystallographic B-factors.
