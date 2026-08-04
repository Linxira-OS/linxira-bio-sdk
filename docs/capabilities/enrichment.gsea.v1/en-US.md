# Preranked Gene Set Enrichment Analysis

## Purpose

Run deterministic local preranked gene set enrichment analysis from an ordered
gene statistic and an explicit gene-set membership table.

## Inputs

A headered CSV/TSV ranked table with gene identifier and finite numeric score
columns, plus a headered membership table with gene and term identifiers.
Optional term name and namespace columns are preserved.

## Parameters

Configure score exponent, minimum and maximum mapped set size, permutation
count, and random seed. Defaults are exponent 1, set sizes 15 through 500,
1,000 permutations, and seed 0.

## Outputs

JSON reports enrichment score, direction, peak rank, leading-edge genes,
fixed-seed nominal permutation p-value, Benjamini-Hochberg FDR, mapping counts,
skipped sets, and warnings.

## Examples

```bash
linxira-bio enrichment gsea ranks.tsv gene-sets.tsv \
  --min-set-size 15 --max-set-size 500 --permutations 1000 --seed 0 --json
```

## Interpretation

Scores are sorted descending; tied scores are ordered by gene identifier and
reported as a warning. Positive and negative results indicate enrichment near
opposite ends of this supplied ranking, not causal direction.

## Caveats

The nominal p-value uses deterministic gene-label permutations and add-one
correction. FDR is Benjamini-Hochberg over tested sets, not the classic pooled
normalized-enrichment-score procedure. Record the ranking method, universe,
gene-set release, parameters, seed, and permutation count.

## Runtime Dependencies

Local Rust only; no Python, R, Java, network service, or external executable is
required.

## Citations

Cite the original GSEA method, the gene-set database and release, and the
source of the ranking statistic. Also report this implementation's permutation
and multiple-testing methods.

## Troubleshooting

Duplicate ranked identifiers, non-finite scores, conflicting term metadata,
and sets equal to the complete ranked universe are rejected or skipped with
explicit diagnostics.
