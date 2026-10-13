# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-13/env-isolation.sh
# Role: separate the two variables behind the python arm's cross-window swing — the CPython version and the library pin versions — before any attribution is written into prose
# Windows: 2026-10-13 (follow-up experiment to the 2026-10-13 window)
# Direction: runs on the measuring host in an idle daytime slot, writes benchmark-results/2026-10-13/env-isolation/
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
# Why: the 2026-10-13 window and the 2026-10-11 window differ in two things at
# once — CPython 3.12 vs 3.14, and the pinned libraries (biopython 1.85 vs 1.88,
# numpy 2.2.4 vs 2.5.4). The python arm is the only arm that moved (rust <=0.6%,
# R <=1.2%), and it moved in both directions (-19.0% to +123.6%), so a single
# cause cannot be claimed from the two windows alone.
#
# The experiment is a 2x2 with one cell that cannot exist, which is itself part
# of the finding: biopython 1.85 and numpy 2.2.4 publish no cp314 wheels, so
# "3.14 + locked pins" is not a runnable environment (see the corrections entry
# of 2026-10-13). Three cells remain:
#
#   A  3.12 + locked pins (biopython 1.85 / numpy 2.2.4)   = the window's environment
#   B  3.12 + newest pins (biopython 1.88 / numpy 2.5.4)   = A vs B isolates the pins
#   C  3.14 + newest pins                                  = B vs C isolates the interpreter
#
# A vs C is the total difference the windows already show, so the three cells
# together say which of the two variables carries it.
#
# Cost: the python legs only, two capabilities, N=10 each — about three minutes
# of measurement on this host. This is a follow-up experiment, not part of the
# window: its reports live beside the window archive under env-isolation/ and are
# never merged into the window summary or the figures, because they are measured
# in environments the pack lock does not describe (that is the point of the test).
#
# Run (detached, in an idle slot):
#     cat scripts/benchmark/windows/2026-10-13/env-isolation.sh \
#       | ssh <host> 'cat > /tmp/env-isolation.sh && chmod +x /tmp/env-isolation.sh &&
#                     setsid nohup /tmp/env-isolation.sh > /tmp/env-isolation.out 2>&1 & echo detached'
#
# Note: stdout/err go to /tmp/env-isolation.out while log() appends to
# /tmp/env-isolation.log; they are deliberately different files so that two writers
# never share one fd (the archived copy is env-isolation-log.txt).
set -u
set -o pipefail   # without this, `cmd | tail -1 || guard` never fires: the status is tail's
export LC_ALL=C
export PATH="$HOME/bin:$HOME/.cargo/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1

W=/tmp/lxn13
OUT=benchmark-results/2026-10-13/env-isolation
LOCK=workflows/org.linxira.benchmark-python/requirements.lock
LOG=/tmp/env-isolation.log
CAPS="sequence.stats.v1 expression.pca.v1"
mkdir -p "$OUT"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" | tee -a "$LOG"; }
busy_load() { ps -eo pcpu,comm --sort=-pcpu --no-headers 2>/dev/null | awk '$1 > 5 {printf "%s %s; ", $1, $2}' | head -c 200; }

# The host is shared with overnight jobs; a contended slot would reproduce the
# very problem this experiment exists to explain, so refuse rather than guess.
heavy="$(busy_load)"
if [ -n "$heavy" ] || pgrep -x aria2c >/dev/null 2>&1; then
  echo "refusing to measure: heavy='$heavy' aria2c='$(pgrep -c aria2c 2>/dev/null || echo 0)'" >&2
  echo "run this in an idle daytime slot; this experiment does not implement a wait mode." >&2
  exit 3
fi

echo "=== inputs (regenerate if tmpfs was wiped; seeded, so digests match the window) ==="
missing=0
for f in big.fa matrix.tsv; do [ -f "$W/$f" ] || missing=1; done
if [ "$missing" = "1" ]; then
  python3 scripts/benchmark/inputs/generate-inputs.py --set synthetic --out "$W" | tail -1
  python3 scripts/benchmark/inputs/generate-inputs.py --set sequence --out "$W" --binary "$HOME/linxira-bio-sdk/target/release/linxira-bio" | tail -1
else
  log "inputs already present"
fi

echo "=== build ==="
cargo build --release -p linxira-bio-cli -p linxira-bio-worker 2>&1 | tail -1
BIN=./target/release/linxira-bio
export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"
log "built at $(git rev-parse --abbrev-ref HEAD) $(git rev-parse --short HEAD)"

cell_env() {
  case "$1" in
    A) echo "3.12|$LOCK|locked" ;;
    B) echo "3.12|biopython==1.88 numpy==2.5.4|newest" ;;
    C) echo "3.14|biopython==1.88 numpy==2.5.4|newest" ;;
  esac
}

for cell in A B C; do
  IFS='|' read -r py _spec kind <<<"$(cell_env "$cell")"
  VENV="$HOME/lx-iso-$cell"
  echo "=== cell $cell: python $py + $kind pins ==="
  rm -rf "$VENV"
  # Each step's status is checked before any pipe: a cell whose interpreter or pins
  # failed to install must NOT be measured, because PATH would fall back to the host
  # python and the report would be filed under a cell label whose environment was
  # never used - exactly the confusion this experiment exists to remove.
  if ! uv venv --python "$py" "$VENV" >"$VENV.log" 2>&1; then
    log "cell $cell: could not create a $py venv (see $VENV.log); cell skipped, not measured"
    continue
  fi
  if [ "$kind" = "locked" ]; then
    if ! uv pip install --python "$VENV/bin/python" --require-hashes -r "$LOCK" >"$VENV.install.log" 2>&1; then
      log "cell $cell: locked install failed (see $VENV.install.log); cell skipped, not measured"
      continue
    fi
  else
    if ! uv pip install --python "$VENV/bin/python" biopython==1.88 numpy==2.5.4 >"$VENV.install.log" 2>&1; then
      log "cell $cell: newest-pin install failed (see $VENV.install.log); cell skipped, not measured"
      continue
    fi
  fi
  # Record the realised environment, not the requested one, and refuse to measure when
  # the probe fails.
  if ! "$VENV/bin/python" -c "import sys, Bio, numpy; print('cell $cell realised:', sys.version.split()[0], 'biopython', Bio.__version__, 'numpy', numpy.__version__)" 2>&1 | tee -a "$LOG"; then
    log "cell $cell: interpreter probe failed; cell skipped, not measured"
    continue
  fi
  for cap in $CAPS; do
    case "$cap" in
      sequence.stats.v1) input="fasta=$W/big.fa"; cls=sequence ;;
      expression.pca.v1) input="matrix=$W/matrix.tsv"; cls=matrix ;;
    esac
    log "cell $cell $cap: start"
    PATH="$VENV/bin:$PATH" $BIN benchmark run "$cap" "$input" --backends python --repeat 10 \
      --output "$OUT/out-$cell-$cap" --dataset-class "$cls" --json 2>&1 | tail -3 | tee -a "$LOG"
    log "cell $cell $cap: done (loadavg $(cut -d' ' -f1 /proc/loadavg))"
  done
done

echo "=== measured cells (python leg only; never merged into the window summary) ==="
for cell in A B C; do
  for cap in $CAPS; do
    f="$OUT/out-$cell-$cap/$cap.benchmark.json"
    [ -f "$f" ] || { log "cell $cell $cap: MISSING report"; continue; }
    python3 - "$cell" "$cap" "$f" <<'PY' | tee -a "$LOG"
import json, sys
cell, cap, path = sys.argv[1], sys.argv[2], sys.argv[3]
r = json.load(open(path, encoding='utf-8'))
b = r['backends'][0]
env = r['environment']
print(f"  cell {cell}  {cap:22} median {b['median_wall_ms']:>8.0f} ms  rss {b['median_peak_rss_mb'] or float('nan'):>6.1f} MB  n={b['repeats']}  py={env.get('python_version')}")
PY
  done
done
cp "$LOG" "$OUT/env-isolation-log.txt" 2>/dev/null || true
echo ENV-ISOLATION-DONE
