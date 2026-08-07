# RNA Secondary Structure Prediction

## Purpose

Predict the minimum free energy (MFE) secondary structure of RNA sequences using the ViennaRNA RNAfold tool.

## Inputs

A FASTA file containing one or more RNA sequences. Each sequence should use standard RNA nucleotide characters (A, U, G, C).

## Parameters

`--temp` sets the folding temperature in Celsius (0–100, default 37.0).

## Outputs

A text file with the predicted secondary structure in dot-bracket notation and the minimum free energy value. JSON result wraps the structure metadata.

## Examples

```bash
linxira-bio rna secondary-structure input.fa output.txt --json
linxira-bio rna secondary-structure input.fa output.txt --temp 25.0 --json
```

## Interpretation

The output uses dot-bracket notation: dots (`.`) represent unpaired bases, matching parentheses `()` represent paired bases. The minimum free energy (kcal/mol) indicates the thermodynamic stability of the predicted structure; more negative values indicate more stable structures.

## Caveats

Requires ViennaRNA RNAfold to be installed on the system. Predictions are based on thermodynamic models and may not represent the biologically active conformation. Only single-sequence folding is supported; no pseudoknots are predicted.

## Runtime Dependencies

Requires ViennaRNA RNAfold (version 2.x). Install via system package manager or from the ViennaRNA website.

## Citations

Lorenz, R., et al. (2011). ViennaRNA Package 2.0. Algorithms for Molecular Biology, 6:26.

## Troubleshooting

If RNAfold is not found, install ViennaRNA via your system package manager. Verify that the input FASTA file contains valid RNA sequences with only A, U, G, and C characters. Temperature values outside 0–100 °C will be rejected.