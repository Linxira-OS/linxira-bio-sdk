# Ka/Ks Calculation

## Purpose

Estimate nonsynonymous and synonymous substitution rates for local codon-aligned sequence pairs.

## Inputs

Provide a valid AXT codon alignment. Each pair requires a name line followed by two aligned coding sequences whose lengths are compatible with complete codons.

## Parameters

Choose one supported method: `NG`, `LWL`, `LPB`, or `YN`. The default is `NG`.

## Outputs

A tab-separated result produced by the configured native KaKs Calculator executable.

## Examples

```text
linxira-bio comparative kaks codon-pairs.axt rates.tsv NG --json
```

## Interpretation

Interpret Ka, Ks, and their ratio in the context of alignment quality, divergence, gene history, and the selected estimator. A Ka/Ks ratio alone does not prove positive selection.

## Caveats

Input must be a biologically valid codon alignment rather than an arbitrary nucleotide alignment. The worker invokes the executable directly with an argument vector and does not execute a shell.

## Runtime Dependencies

Requires `KaKs_Calculator` on `PATH`, or an executable selected with `LINXIRA_BIO_KAKS_CALCULATOR`. Run the environment audit when the executable is missing.

## Citations

Cite KaKs Calculator, its version, the selected method, and the codon-alignment procedure.

## Troubleshooting

Check reading frame, sequence-pair structure, stop codons, and alignment length before retrying. Confirm executable availability with the environment audit.
