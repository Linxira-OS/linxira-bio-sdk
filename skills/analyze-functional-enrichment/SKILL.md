---
name: analyze-functional-enrichment
description: Normalize local GO and eggNOG-mapper annotation tables and run deterministic custom, Gene Ontology, or KEGG over-representation analysis. Use for gene-to-term association construction, eggNOG result cleanup, hypergeometric enrichment, Benjamini-Hochberg correction, mapped/unmapped query accounting, and bounded enrichment result tables.
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
