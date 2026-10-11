# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-04/three-b.sh
# Role: one stage of the three-backend N=3 NAS window (SSR long pass)
# Windows: 2026-10-04 (first real execution of the R backend)
# Direction: exploratory measurement, superseded by the N=10 window
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
export LC_ALL=C
export PATH="$HOME/lx-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1
BIN=./target/release/linxira-bio
export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"
W=/tmp/lxthree
REF=/tmp/lxg/big.fa

echo "=== three-way sequence.ssr.v1 @ 97 MB (r leg is ~27 min per run; explicit user request) ==="
date -u +"start %H:%M:%SZ"
$BIN benchmark run sequence.ssr.v1 fasta="$REF" --backends rust,python,r --repeat 3 \
  --output "$W/out-sequence.ssr.v1" --dataset-class sequence --json 2>/dev/null | python3 -c "
import json,sys
d=json.load(sys.stdin)['result']
print('verdict', d['consistency'], '| precision', d['environment'].get('precision'), '| speedup', d.get('speedup'), '| memory_saving', d.get('memory_saving'))
for b in d['backends']:
    print(f\"  {b['backend']:7} median {b['median_wall_ms']:12.3f} ms  iqr {b['iqr_wall_ms']:10.3f}  rss {b.get('median_peak_rss_mb')} MB\")
if d.get('findings'): print('  findings:', json.dumps(d['findings'][:2], ensure_ascii=False)[:260])
"
date -u +"end %H:%M:%SZ"

echo "=== write-back ==="
python3 scripts/benchmark/analysis/update-runtime-preferences.py "$W/out-sequence.ssr.v1"/*.benchmark.json
git --no-pager diff --stat runtime-preferences.json
echo THREE-B-DONE
