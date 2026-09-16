# Annotation enrichment deliverables (GO / KEGG / KO over-representation)

> **LOCKED 2026-09-16**: deliverables frozen after study-side acceptance
> (R0 fix verified: query_mapped=1663 for module_sweet_1). 48 CSV + 4
> cross-species files; any change after this line invalidates the lock.
> Gene sets: three differential-expression contrasts of a Tartary buckwheat
> study (blue-light vs dark; young fruit vs leaf; mature fruit vs leaf) and
> 13 WGCNA modules.

- tool: linxira-bio enrichment custom (hypergeometric upper tail +
  Benjamini-Hochberg, SDK engine), one run per gene set
- bridge: M0 — GWHTBJBL<6-digit tail> <-> GWHPBJBL<6-digit tail>,
  1:1 unique tails; 27908/33955 mapped
- universes (mapped and annotated): GO 14944, KO 13912,
  KEGG pathway 8929
- significance: padj < 0.05 and |lfc| > 1 (mainline DEG sets, direction
  split by lfc sign); reported terms filtered at padj < 0.05
- CSV columns: term, description, cnt_in_set (query genes in term),
  set_size (mapped query size), bg_cnt (universe genes in term),
  universe (background gene count), p, padj
- term descriptions: GO from go-basic.obo; KO/pathway from rest.kegg.jp
- ledger: bench/enrich-benchmark.csv
- duplicate 6-digit tails in the annotation: 0
- batch date: 2026-09-15 21:45:05

## R0 fix (2026-09-15 evening)

Module enrichment originally used only the BRGA/wall-annotated subset as
the query (query_mapped=9 for module_sweet_1) — rejected and recomputed.
Every module's query is now its FULL mapped membership: module_sweet_1 =
2,639 mapped genes (GO query_mapped 1,663), module_tartarv2_1 = 3,508
mapped (GO query_mapped 1,289). BRGA/wall gene tables were correct and are
unchanged.

## R1/R2 additions

- cross_species_mod1_top20.csv: top20 GO comparison for the
  module_sweet_1 x module_tartarv2_1 pair
- cross_species_convergence.txt: term-ID level top20 overlap = 0
  (Fisher p = 1.0)
- cross_species_convergence_full.txt: full significant GO overlap = 14 of
  308/229, Fisher p = 3.3e-3 — a coherent cell-wall / membrane /
  transport signature; hormone-term-level identity is NOT present
  (brassinosteroid terms significant in 0 of 2 modules)
- cross_species_BRGA_wall_counts.csv: per-module BR/GA and cell-wall gene
  hit counts
- blue_DOWN_BRGA_terms.csv: brassinosteroid-related terms and
  DWF4/BZR1/DWF1-bearing terms in the blue-light DOWN direction
  (padj 0.78-0.86 — not significant)
