#!/bin/bash
# Round 4: real analysis chain on the rented CPUs (works on both machines).
# Simulated bulk-RNA expression matrix (2000 genes x 12 samples, 2 conditions,
# 150 DE genes) -> matrix-qc -> normalize -> pca -> cluster, plus GO ORA on
# the DE hit list against a structured association table.
exec > /root/rna-analysis.log 2>&1
set -x
cd /root/autodl-tmp || exit 8
BIN=/root/autodl-tmp/linxira-bio-sdk/target/release/linxira-bio

python3 - <<'PYEOF'
import numpy as np
rng = np.random.default_rng(4242)
n_genes, n_ctrl, n_trt, n_de = 2000, 6, 6, 150
base = rng.lognormal(mean=6.0, sigma=1.0, size=n_genes)
counts = np.empty((n_genes, n_ctrl + n_trt))
for s in range(n_ctrl + n_trt):
    lib = rng.uniform(0.85, 1.15)
    counts[:, s] = rng.poisson(base * lib).astype(float)
de_idx = rng.choice(n_genes, n_de, replace=False)
fc = np.exp(rng.uniform(1.0, 2.0, n_de)) * rng.choice([-1, 1], n_de)
counts[de_idx, n_ctrl:] *= np.exp(fc)[:, None]
genes = ["GENE%04d" % i for i in range(n_genes)]
samples = ["ctrl_%d" % i for i in range(n_ctrl)] + ["trt_%d" % i for i in range(n_trt)]
with open("expr.csv", "w") as fh:
    fh.write("gene," + ",".join(samples) + "\n")
    for g, row in zip(genes, counts):
        fh.write(g + "," + ",".join(str(int(v)) for v in row) + "\n")
de_sorted = de_idx[np.argsort(-np.abs(fc))]
with open("de_genes.txt", "w") as fh:
    for i in de_sorted[:80]:
        fh.write(genes[i] + "\n")
# structured GO-style associations: 30 terms, one enriched in the top DE slice
terms = ["GO:TERM%02d" % t for t in range(30)]
with open("assoc.csv", "w") as fh:
    fh.write("gene,term\n")
    for i in de_sorted[:40]:
        fh.write("%s,%s\n" % (genes[i], terms[0]))
    rng2 = np.random.default_rng(7)
    for g in genes:
        for t in rng2.choice(30, size=rng2.integers(1, 4), replace=False):
            fh.write("%s,%s\n" % (g, terms[t]))
print("inputs ready: expr.csv %dx%d, de_genes.txt 80, assoc.csv" % (n_genes, n_ctrl + n_trt))
PYEOF
echo "PY-EXIT=$?"

$BIN expression matrix-qc expr.csv --json > qc.json; echo "QC=$?"
$BIN expression normalize expr.csv norm.tsv --method median-ratio --json > norm.json; echo "NORM=$?"
$BIN expression pca expr.csv --components 4 --json > pca.json; echo "PCA=$?"
$BIN expression cluster expr.csv --json > cluster.json; echo "CLUSTER=$?"
$BIN enrichment go de_genes.txt assoc.csv --min-overlap 3 --json > enrich.json; echo "ENRICH=$?"

head -c 300 pca.json; echo
grep -o '"TERM00[^,]*' enrich.json | head -3
echo "RNA-ANALYSIS-DONE $(date +%H:%M:%S)"
