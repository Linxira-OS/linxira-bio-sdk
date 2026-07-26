---
name: analyze-protein-properties
description: Calculate deterministic local physicochemical properties from protein FASTA files. Use for sequence length, amino-acid composition, molecular weight, theoretical pI, charge at pH 7, aromaticity, GRAVY, and sequence extinction coefficients without external databases.
---

# Analyze Protein Properties

Use the tested Rust capability for sequence-derived protein characterization.

## Run

1. Confirm the input is a local protein FASTA rather than nucleotide FASTA or
   an alignment containing gaps.
2. Run:

```bash
linxira-bio protein properties INPUT.faa --json
```

When developing in the source repository, run:

```bash
cargo run -p linxira-bio-cli -- protein properties INPUT.faa --json
```

3. Preserve `protein.properties.v1`, CLI version, input hash, warnings, and the
   complete JSON result.
4. Stop on gap, stop, digit, or other unsupported symbols. Do not silently
   remove them.

## Interpret

- Molecular weight is in daltons and assumes an unmodified linear sequence.
- Theoretical pI and pH 7 charge use sequence ionizable groups; buffer,
  modification, and structural effects are not modeled.
- GRAVY is the mean Kyte-Doolittle hydropathy value.
- Reduced and oxidized extinction coefficients use tryptophan, tyrosine, and
  possible cystine contributions from sequence counts.
- Records containing `B`, `J`, `O`, `U`, `X`, or `Z` retain composition and
  length, but derived physicochemical values are `null` instead of guessed.

Treat the results as sequence characterization, not experimental measurements.
Do not infer folding, localization, toxicity, function, or clinical relevance
from these values alone.
