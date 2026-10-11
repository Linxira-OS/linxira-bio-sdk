#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-11/archive.sh
# Role: assemble the 2026-10-11 archive from the stage reports (staging, input digests, summary, tables, figures, figure check)
# Windows: 2026-10-11
# Direction: last step of the window; runs on the measuring host, produces benchmark-results/2026-10-11/
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
# Run from a checkout on the measuring host:
#     cat scripts/benchmark/windows/2026-10-11/archive.sh | ssh <host> bash -s
#
# Stages the per-capability reports under unique names (the granularity shapes
# share the capability sequence.stats.v1), records input digests, then builds
# the archive with the committed tools and re-checks every figure number
# against the tables. Idempotent: it rewrites the archive directory.
set -eu
export LC_ALL=C
export PATH="$HOME/lx-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1

W=/tmp/lxn10
S=/tmp/lxn10-archive
CAPS="sequence.stats.v1 expression.pca.v1 set.venn.v1 enrichment.overrepresentation.v1 structure.pdb.summary.v1 variant.stats.v1 fastq.qc.v1 sequence.ssr.v1"

rm -rf "$S"
mkdir -p "$S"
for cap in $CAPS; do
  src="$W/out-$cap/$cap.benchmark.json"
  if [ ! -f "$src" ]; then
    echo "skipping $cap (no report at $src)"
    continue
  fi
  cp "$src" "$W/out-$cap/$cap.benchmark.md" "$S/"
done
for shape in one twenty many; do
  cp "$W/out-gran-$shape/sequence.stats.v1.benchmark.json" "$S/sequence.stats.v1.granularity-$shape.benchmark.json"
  cp "$W/out-gran-$shape/sequence.stats.v1.benchmark.md" "$S/sequence.stats.v1.granularity-$shape.benchmark.md"
done

echo "=== input digests (paths as staged on this host; verify with sha256sum -c) ==="
sha256sum /tmp/lxg/big.fa \
  "$W/matrix.tsv" "$W/sets.tsv" "$W/structure.pdb" "$W/variants.vcf" "$W/genes.txt" "$W/associations.tsv" \
  "$W/g_one.fa" "$W/g_twenty.fa" "$W/g_many.fa" > "$S/inputs.sha256"
if [ -f "$W/reads200.fq" ]; then
  sha256sum "$W/reads200.fq" >> "$S/inputs.sha256"
elif [ -f /tmp/lxg8/reads200.fq ]; then
  sha256sum /tmp/lxg8/reads200.fq >> "$S/inputs.sha256"
fi
cat "$S/inputs.sha256"

echo "=== archive ==="
python3 scripts/benchmark/analysis/summarize-reports.py \
  --descriptor scripts/benchmark/windows/2026-10-11/descriptor.json \
  --reports "$S" --date 2026-10-11 --output-dir benchmark-results/2026-10-11
cp "$S/inputs.sha256" benchmark-results/2026-10-11/
# The granularity experiment is archived beside the summary, not inside its
# entries: three sequence.stats.v1 runs in one summary would collide on the
# capability key. The figures read them through --granularity.
cp "$S"/sequence.stats.v1.granularity-*.benchmark.json "$S"/sequence.stats.v1.granularity-*.benchmark.md \
  benchmark-results/2026-10-11/ 2>/dev/null || true

echo "=== statistics tables ==="
python3 scripts/benchmark/analysis/statistics-tables.py \
  --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 \
  --output-dir benchmark-results/2026-10-11 \
  --comparison benchmark-results/2026-09-13/summary.json

echo "=== figures ==="
mkdir -p benchmark-results/2026-10-11/blog-mirror/figures
python3 scripts/benchmark/figures/render-svgs.py \
  --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 \
  --comparison benchmark-results/2026-09-13/summary.json \
  --granularity benchmark-results/2026-10-11 \
  --output benchmark-results/2026-10-11/blog-mirror/figures

echo "=== figure/table consistency ==="
python3 scripts/benchmark/figures/verify-figures.py \
  --figures benchmark-results/2026-10-11/blog-mirror/figures \
  --summary benchmark-results/2026-10-11/summary.json \
  --reports benchmark-results/2026-10-11 \
  --comparison benchmark-results/2026-09-13/summary.json \
  --granularity benchmark-results/2026-10-11

echo "=== archive contents ==="
ls -la benchmark-results/2026-10-11
echo ARCHIVE-DONE
