# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-13/ssr.sh
# Role: the sequence.ssr.v1 stage of the locked-environment re-run (the ~8 h R leg)
# Windows: 2026-10-13
# Direction: the archived N=10 three-backend measurement
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
export LC_ALL=C
export PATH="$HOME/lx-locked-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1
BIN=./target/release/linxira-bio
export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"
W=/tmp/lxn13
mkdir -p "$W"

# Lock/environment gate (added 2026-10-12): the measurement is only reproducible if the
# interpreter environment is the one the pack lock describes. Numbers produced under a
# different environment must not be presented as reproducible.
python3 scripts/benchmark/analysis/verify-lock-env.py \
  --lock workflows/org.linxira.benchmark-python/requirements.lock || {
    echo "refusing to measure: interpreter environment differs from requirements.lock" >&2
    exit 1
  }

echo "=== sequence.ssr.v1 N=10 (r leg ~1633 s/run => ~5.0 h total) ==="
date -u +"start %H:%M:%SZ"
$BIN benchmark run sequence.ssr.v1 fasta=/tmp/lxn13/big.fa --backends rust,python,r --repeat 10 \
  --output "$W/out-sequence.ssr.v1" --dataset-class sequence --json 2>/dev/null | python3 -c "
import json,sys
d=json.load(sys.stdin)['result']
print('  verdict', d['consistency'], '| precision', d['environment'].get('precision'), '| speedup', d.get('speedup'), '| memory_saving', d.get('memory_saving'))
for b in d['backends']:
    runs=[r['wall_ms'] for r in b.get('runs',[]) if r.get('ok')]
    print(f\"    {b['backend']:7} median {b['median_wall_ms']:12.3f} ms  iqr {b['iqr_wall_ms']:10.3f}  n {len(runs)}  min {min(runs) if runs else 0:.0f}  max {max(runs) if runs else 0:.0f}  rss {b.get('median_peak_rss_mb')} MB\")
if d.get('findings'): print('    findings:', json.dumps(d['findings'][:2], ensure_ascii=False)[:200])
"
date -u +"end %H:%M:%SZ"
echo N10-SSR-DONE
