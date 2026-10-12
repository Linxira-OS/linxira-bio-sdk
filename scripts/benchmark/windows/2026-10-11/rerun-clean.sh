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
# Run from a checkout on the measuring host. Long windows must NOT be tied to the
# ssh session (a dropped connection killed the driver on 2026-10-12 while its child
# kept running), so upload and detach:
#     cat scripts/benchmark/windows/2026-10-11/rerun-clean.sh \
#       | ssh <host> 'cat > /tmp/rerun-clean.sh && chmod +x /tmp/rerun-clean.sh &&
#                     setsid nohup /tmp/rerun-clean.sh > /tmp/rerun-clean.log 2>&1 & echo detached'
#     ssh <host> 'while ! grep -q RERUN-CLEAN-DONE /tmp/rerun-clean.log; do sleep 60; done; tail -40 /tmp/rerun-clean.log'
#
# ALLOW_CI=1      proceed even if a CI job is running or queued (records the fact)
# SSR_REPEATS=N   timed repeats for sequence.ssr (default 10)
# STAGES="..."    which stages to run (default: the whole window; PARTIAL=1 selects
#                 only what the contamination audit flagged: fastq ssr)
set -u
export LC_ALL=C
export PATH="$HOME/.cargo/bin:$HOME/lx-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1

W=/tmp/lxn10
LOG="$W/rerun-window-log.txt"
SSR_REPEATS="${SSR_REPEATS:-10}"
STAGES="${STAGES:-fast granularity fastq ssr}"
[ "${PARTIAL:-0}" = "1" ] && STAGES="fastq ssr"
mkdir -p "$W"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" | tee -a "$LOG"; }

echo "=== preflight: exclusive host ==="
runner_state="$(systemctl --user is-active gitea-runner 2>/dev/null || true)"
# A running service is not the same as a running job: what contaminates a
# measurement is a job executing (status 6) or waiting to execute (5/7).
job_state() {
  sudo -n -u postgres psql -d gitea -t -A -c \
    "select count(*) from action_run where status in (5,6,7)" 2>/dev/null || echo unknown
}
job_count="$(job_state)"
log "preflight: gitea-runner=${runner_state:-unknown} jobs_pending_or_running=${job_count} loadavg=$(cut -d' ' -f1-3 /proc/loadavg)"
if [ "${job_count:-unknown}" != "0" ] && [ "${ALLOW_CI:-0}" != "1" ]; then
  echo "refusing to start: $job_count CI job(s) running or queued on this host." >&2
  echo "wait for them, stop the runner, or pass ALLOW_CI=1 to record the contamination." >&2
  exit 2
fi
for attempt in $(seq 1 24); do
  load="$(cut -d' ' -f1 /proc/loadavg)"
  case "$load" in 0.*) log "load settled at $load"; break;; esac
  log "waiting for load to settle (now $load)"; sleep 5
done

echo "=== inputs (the host's /tmp is tmpfs, so a reboot wipes them) ==="
missing=0
for f in big.fa reads200.fq matrix.tsv sets.tsv structure.pdb variants.vcf genes.txt associations.tsv g_one.fa g_twenty.fa g_many.fa; do
  [ -f "$W/$f" ] || missing=1
done
if [ "$missing" = "1" ]; then
  log "regenerating inputs (seeded; digests land in the archive's inputs.sha256)"
  python3 scripts/benchmark/inputs/generate-inputs.py --set synthetic --out "$W" | tail -2
  python3 scripts/benchmark/inputs/generate-inputs.py --set granularity --out "$W" | tail -2
  python3 scripts/benchmark/inputs/generate-inputs.py --set sequence --out "$W" --binary "$HOME/linxira-bio-sdk/target/release/linxira-bio" | tail -2
  python3 scripts/benchmark/inputs/generate-inputs.py --set reads --out "$W" --binary "$HOME/linxira-bio-sdk/target/release/linxira-bio" | tail -2
else
  log "inputs already present"
fi
log "input sizes: $(du -sh "$W" | cut -f1)"

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

run() {
  cap="$1"; shift; cls="$1"; shift
  echo "=== $cap (class $cls) ==="
  log "start $cap"
  $BIN benchmark run "$cap" "$@" --backends rust,python,r --repeat 10 \
    --output "$W/out-$cap" --dataset-class "$cls" --json 2>/dev/null | show
  log "end $cap loadavg=$(cut -d' ' -f1-3 /proc/loadavg) jobs=$(job_state)"
}

case "$STAGES" in *fast*)
  run sequence.stats.v1 sequence "fasta=$W/big.fa"
  run expression.pca.v1 matrix "matrix=$W/matrix.tsv"
  run set.venn.v1 other "table=$W/sets.tsv"
  run enrichment.overrepresentation.v1 annotation "genes=$W/genes.txt" "associations=$W/associations.tsv"
  run structure.pdb.summary.v1 structure "pdb=$W/structure.pdb"
  run variant.stats.v1 other "vcf=$W/variants.vcf"
;; esac

case "$STAGES" in *granularity*)
  for shape in one twenty many; do
    echo "=== sequence.stats.v1 on g_$shape.fa ==="
    log "start granularity $shape"
    $BIN benchmark run sequence.stats.v1 fasta="$W/g_$shape.fa" --backends rust,python,r --repeat 10 \
      --output "$W/out-gran-$shape" --dataset-class sequence --json 2>/dev/null | show
    log "end granularity $shape loadavg=$(cut -d' ' -f1-3 /proc/loadavg) jobs=$(job_state)"
  done
;; esac

echo "=== fastq.qc.v1 N=10 (the R leg is what the CI jobs overlapped) ==="
log "start fastq.qc.v1 N=10"
case "$STAGES" in *fastq*)
  $BIN benchmark run fastq.qc.v1 fastq="$W/reads200.fq" --backends rust,python,r --repeat 10 \
    --output "$W/out-fastq.qc.v1" --dataset-class other --json 2>/dev/null | show
  log "end fastq.qc.v1 loadavg=$(cut -d' ' -f1-3 /proc/loadavg) jobs=$(job_state)"
;; esac

echo "=== sequence.ssr.v1 N=$SSR_REPEATS (deferred from the first pass) ==="
log "start sequence.ssr.v1 N=$SSR_REPEATS"
case "$STAGES" in *ssr*)
  $BIN benchmark run sequence.ssr.v1 fasta="$W/big.fa" --backends rust,python,r --repeat "$SSR_REPEATS" \
    --output "$W/out-sequence.ssr.v1" --dataset-class sequence --json 2>/dev/null | show
  log "end sequence.ssr.v1 loadavg=$(cut -d' ' -f1-3 /proc/loadavg) jobs=$(job_state)"
;; esac
log "runner state at end: $(systemctl --user is-active gitea-runner 2>/dev/null || true)"

echo "=== archive (summary, tables, bilingual figures, figure check) ==="
bash scripts/benchmark/windows/2026-10-11/archive.sh
cp "$LOG" benchmark-results/2026-10-11/window-log.txt
echo RERUN-CLEAN-DONE
