# Protein Physicochemical Properties

## Purpose

Calculate deterministic sequence-derived properties for local protein FASTA records.

## Inputs

A local plain or gzip-compressed protein FASTA. Standard amino acids are calculated directly; `B`, `J`, `O`, `U`, `X`, and `Z` are retained as ambiguous or non-standard residues.

## Parameters

This version has no analysis parameters.

## Outputs

JSON reports length, residue composition, molecular weight, theoretical isoelectric point, charge at pH 7, aromaticity, GRAVY, and reduced/oxidized extinction coefficients per record.

## Examples

```bash
linxira-bio protein properties proteins.faa --json
```

## Interpretation

Use these values for sequence characterization and experimental planning. Molecular weight is reported in daltons; extinction coefficients use `M^-1 cm^-1`.

## Caveats

Derived physicochemical values are `null` when a sequence contains ambiguous or non-standard residues, rather than silently inventing residue properties. Post-translational modifications, disulfide topology, buffers, and structural state are not inferred.

## Runtime Dependencies

FASTA parsing and calculations run in local Rust with no external database.

## Citations

Cite the residue-mass, Henderson–Hasselbalch charge, Kyte–Doolittle hydropathy, and sequence extinction-coefficient methods appropriate to the reported metric.

## Troubleshooting

Remove alignment gap and stop symbols before analysis. Inspect warnings for records containing ambiguous residues.
