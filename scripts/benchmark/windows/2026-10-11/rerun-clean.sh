#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-11/rerun-clean.sh
# Role: CI-free re-run of the stages a same-host CI job contaminated (fastq.qc R leg) plus the deferred sequence.ssr stage, with a preflight gate and per-stage provenance logging
# Windows: 2026-10-11 (re-run)
# Direction: run on the measuring host after its CI runner is stopped; ends by calling archive.sh
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
# Why this script exists: the local Gitea runner shares this host with the
# benchmark, and two CI jobs landed inside fastq.qc's R-leg window (see the
# contamination audit in docs/engine-evals/local-nas-g7400-window-2026-10-04.md
# §6.2.5). The fast batch and the granularity experiment were verified clean
# against the CI job windows, so they are not repeated here.
#
# Run from a checkout on the measuring host:
#     cat scripts/benchmark/windows/2026-10-11/rerun-clean.sh | ssh <host> bash -s
#
# ALLOW_CI=1      proceed even if the runner is active (records the fact)
# SSR_REPEATS=N   timed repeats for sequence.ssr (default 10)
set -u
export LC_ALL=C
export PATH="$HOME/.cargo/bin:$HOME/lx-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1

W=/tmp/lxn10
LOG="$W/rerun-window-log.txt"
SSR_REPEATS="${SSR_REPEATS:-10}"
mkdir -p "$W"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" | tee -a "$LOG"; }

echo "=== preflight: exclusive host ==="
runner_state="$(systemctl --user is-active gitea-runner 2>/dev/null || true)"
desktop_state="$(systemctl --user is-active gitea 2>/dev/null || true)"
log "preflight: gitea-runner=${runner_state:-unknown} gitea(user)=${desktop_state:-unknown} loadavg=$(cut -d' ' -f1-3 /proc/loadavg)"
if [ "${runner_state:-unknown}" = "active" ] && [ "${ALLOW_CI:-0}" != "1" ]; then
  echo "refusing to start: the CI runner is active on this host." >&2
  echo "stop it first (systemctl --user stop gitea-runner) or pass ALLOW_CI=1 to record the contamination." >&2
  exit 2
fi
for attempt in $(seq 1 24); do
  load="$(cut -d' ' -f1 /proc/loadavg)"
  case "$load" in 0.*) log "load settled at $load"; break;; esac
  log "waiting for load to settle (now $load)"; sleep 5
done

echo "=== build (the binary records its own revision in every report) ==="
cargo build --release -p linxira-bio-cli -p linxira-bio-worker 2>&1 | tail -2
log "built at revision $(git rev-parse --short HEAD)"

BIN=./target/release/linxira-bio
export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"

show() {
python3 -c "
import json, sys
d = json.load(sys.stdin)['result']
print('  verdict', d['consistency'], '| precision', d['environment'].get('precision'), '| speedup', d.get('speedup'))
for b in d['backends']:
    runs = [r for r in b.get('runs', []) if r.get('ok')]
    print(f\"    {b['backend']:7} median {b['median_wall_ms']:11.3f} ms  n {len(runs)}  rss {b.get('median_peak_rss_mb')} MB\")
stamps = [r.get('started_at') for b in d['backends'] for r in b.get('runs', [])]
print('    per-run started_at:', 'present' if all(stamps) else 'MISSING', stamps[:2])
"
}

echo "=== fastq.qc.v1 N=10 (the R leg is what the CI jobs overlapped) ==="
log "start fastq.qc.v1 N=10"
$BIN benchmark run fastq.qc.v1 fasta="$W/reads200.fq" --backends rust,python,r --repeat 10 \
  --output "$W/out-fastq.qc.v1" --dataset-class other --json 2>/dev/null | show
log "end fastq.qc.v1 loadavg=$(cut -d' ' -f1-3 /proc/loadavg)"

echo "=== sequence.ssr.v1 N=$SSR_REPEATS (deferred from the first pass) ==="
log "start sequence.ssr.v1 N=$SSR_REPEATS"
$BIN benchmark run sequence.ssr.v1 fasta=/tmp/lxg/big.fa --backends rust,python,r --repeat "$SSR_REPEATS" \
  --output "$W/out-sequence.ssr.v1" --dataset-class sequence --json 2>/dev/null | show
log "end sequence.ssr.v1 loadavg=$(cut -d' ' -f1-3 /proc/loadavg)"
log "runner state at end: $(systemctl --user is-active gitea-runner 2>/dev/null || true)"

cp "$LOG" "$W/inputs.sha256" 2>/dev/null || true
echo "=== archive (summary, tables, bilingual figures, figure check) ==="
bash scripts/benchmark/windows/2026-10-11/archive.sh
cp "$LOG" benchmark-results/2026-10-11/window-log.txt
echo RERUN-CLEAN-DONE
