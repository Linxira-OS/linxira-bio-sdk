---
name: analyze-functional-enrichment
description: Normalize local GO and eggNOG-mapper annotation tables and run deterministic over-representation or preranked gene-set enrichment analysis. Use for gene-to-term association construction, eggNOG cleanup, custom/GO/KEGG hypergeometric enrichment, weighted GSEA, deterministic label permutations, Benjamini-Hochberg correction, mapped/unmapped accounting, and bounded enrichment results.
---

# Analyze Functional Enrichment

Use the tested Rust capabilities without uploading identifiers or writing a new
statistics script.

## Prepare Annotations

Normalize GO columns containing comma, semicolon, or pipe-separated identifiers:

```bash
linxira-bio annotation go INPUT.tsv OUTPUT.tsv --json
```

Use `--gene-column` or `--go-column` when the headers are not one of the
documented aliases. Normalize a standard eggNOG-mapper annotations table with:

```bash
linxira-bio annotation eggnog INPUT.tsv OUTPUT.tsv --json
```

Both commands refuse to overwrite an existing output. Preserve the normalized
TSV, input hash, capability version, warnings, and JSON result.

## Run Enrichment

Provide a query identifier file and an association table with `gene_id` and
`term_id`; `term_name` and `namespace` are optional:

```bash
linxira-bio enrichment custom GENES.txt ASSOCIATIONS.tsv --json
linxira-bio enrichment go GENES.txt ASSOCIATIONS.tsv --json
linxira-bio enrichment kegg GENES.txt ASSOCIATIONS.tsv --json
```

The association universe is the background. Use `--min-overlap` to exclude
terms with insufficient query overlap, `--max-terms` to bound the report, and
`--include-genes` only when the member identifiers are required.

When developing in the source repository, prefix commands with
`cargo run -p linxira-bio-cli --`.

## Interpret

- `p_value` is the one-sided hypergeometric upper-tail probability.
- `adjusted_p_value` is Benjamini-Hochberg corrected across tested terms.
- `fold_enrichment` compares the query term fraction with its background
  fraction.
- `query_unmapped_count` must be reviewed before interpreting the result.
- GO mode accepts valid `GO:` identifiers. KEGG mode selects KEGG namespaces
  and conventional pathway or ortholog identifiers. Custom mode keeps every term.

Do not treat enrichment as causal evidence or a clinical conclusion. Report
the identifier system, association source and version, universe definition,
filtering rules, multiple-testing method, and omitted terms.

## Run Preranked GSEA

Provide a headered ranked table with gene identifier and numeric score columns,
plus a headered gene-set membership table with gene and term identifiers:

```bash
linxira-bio enrichment gsea RANKED.tsv GENE_SETS.tsv \
  --min-set-size 15 --max-set-size 500 --permutations 1000 --seed 0 --json
```

- Preserve score direction and document how the ranking statistic was derived.
- Review skipped sets and unmapped memberships before interpreting results.
- Treat `nominal_p_value` as the fixed-seed gene-label permutation estimate;
  `fdr_bh` is Benjamini-Hochberg across all tested sets.
- Use more permutations when tail precision matters and record the seed.
