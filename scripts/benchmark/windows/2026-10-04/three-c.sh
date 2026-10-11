# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-04/three-c.sh
# Role: one stage of the three-backend N=3 NAS window (wrap-up)
# Windows: 2026-10-04 (first real execution of the R backend)
# Direction: exploratory measurement, superseded by the N=10 window
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
export LC_ALL=C
export PATH="$HOME/lx-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1

echo "=== sync ==="
git checkout -- runtime-preferences.json workflows/org.linxira.benchmark-r/manifest.json 2>/dev/null || true
for attempt in 1 2 3; do
  git fetch origin >/dev/null 2>&1 || true
  git pull --ff-only >/dev/null 2>&1
  head_now=$(git rev-parse --short HEAD); remote=$(git rev-parse --short origin/main 2>/dev/null || echo "?")
  [ "$head_now" = "$remote" ] && break
  sleep 5
done
echo "HEAD=$head_now origin/main=$remote"

echo "=== rebuild (dataset.rs changed) ==="
cargo build --release -p linxira-bio-cli -p linxira-bio-worker 2>&1 | tail -2

echo "=== SAM detection regression ==="
cargo test -p linxira-bio-core wide_numeric_matrix_is_not_mistaken_for_sam 2>&1 | tail -4
B=./target/release/linxira-bio
for f in /tmp/lxthree/m10.tsv /tmp/lxthree/m11.tsv /tmp/lxthree/m12.tsv tests/fixtures/data-inspection/alignments.sam; do
  printf "  %-46s -> " "$(basename $f)"
  $B dataset inspect "$f" --json 2>&1 | python3 -c "import json,sys;print(json.load(sys.stdin)['result'].get('format'))"
done

echo "=== regenerate R pack manifest ==="
python3 scripts/update-pack-manifest.py workflows/org.linxira.benchmark-r 2>&1 | tail -1

echo "=== R pack tests ==="
Rscript workflows/org.linxira.benchmark-r/tests/test_benchmark_harness.R 2>&1 | tail -4

echo "=== Rust ground truth for the repeated-mask venn input ==="
printf 'control\ttreated\ngeneA\tgeneA\ngeneB\tgeneB\ngeneC\t\ngeneD\t\n' > /tmp/lxthree/repeated.tsv
$B set venn /tmp/lxthree/repeated.tsv --include-items --json 2>&1 | head -c 600; echo

export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"
show() {
python3 -c "
import json,sys
d=json.load(sys.stdin)['result']
print('  verdict', d['consistency'], '| speedup', d.get('speedup'))
for b in d['backends']:
    print(f\"    {b['backend']:7} median {b['median_wall_ms']:11.3f} ms  rss {b.get('median_peak_rss_mb')} MB\")
if d.get('findings'): print('    findings:', json.dumps(d['findings'][:2], ensure_ascii=False)[:240])
"
}

echo "=== three-way: expression.pca.v1 (after the SAM detection fix) ==="
$B benchmark run expression.pca.v1 matrix=/tmp/lxthree/matrix.tsv --backends rust,python,r --repeat 3 \
  --output /tmp/lxthree/out-pca2 --dataset-class matrix --json 2>/dev/null | show

echo "=== three-way: set.venn.v1 (after the R grouping fix) ==="
rm -rf /tmp/lxthree/out-set.venn.v1
$B benchmark run set.venn.v1 table=/tmp/lxthree/sets.tsv --backends rust,python,r --repeat 3 \
  --output /tmp/lxthree/out-set.venn.v1 --dataset-class other --json 2>/dev/null | show

echo "=== write-back ==="
python3 scripts/benchmark/analysis/update-runtime-preferences.py /tmp/lxthree/out-pca2/*.benchmark.json /tmp/lxthree/out-set.venn.v1/*.benchmark.json
git --no-pager diff --stat runtime-preferences.json
echo THREE-C-DONE
