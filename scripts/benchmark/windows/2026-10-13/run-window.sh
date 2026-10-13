# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-13/run-window.sh
# Role: driver of the locked-environment re-run: preflight, code label, inputs, build, the four measurement stages, the tool arms, then the archive
# Windows: 2026-10-13
# Direction: run detached on the measuring host; ends by calling archive.sh and copying the code label into the archive
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
# Why this window exists: the archived 2026-10-11 numbers were measured in an
# interpreter environment that did not match requirements.lock (biopython 1.88 /
# numpy 2.5.4 in the environment against 1.85 / 2.2.4 in the lock). This window
# re-measures the same eight capabilities on the same host and inputs with the python
# arm running inside a virtualenv installed from the lock, so the published numbers
# belong to the environment the lock claims. Ecosystem-tool arms (arm_class=tool) run
# in the same session and are archived separately: ROADMAP 7.1 keeps the two arm
# classes out of one table, and a tool arm stores no ratio against our engine.
#
# The window labels itself. code-label.txt records the branch, the full commit and
# `git describe --tags --always --dirty` - no release tag exists yet, so that string
# is the short commit hash and the label states the project's release status by
# itself - plus the lock digest, the tree cleanliness and the interpreter versions
# the measurement actually used.
#
# Detach, because a dropped ssh session killed a driver on 2026-10-12 while its child
# kept running:
#     cat scripts/benchmark/windows/2026-10-13/run-window.sh \
#       | ssh <host> 'cat > /tmp/run-window.sh && chmod +x /tmp/run-window.sh &&
#                     setsid nohup /tmp/run-window.sh > /tmp/run-window-1013.log 2>&1 & echo detached'
#
# ALLOW_CI=1     proceed even if CI jobs run or are queued (records the fact)
# ALLOW_LOAD=1   proceed even if an unrelated process is above 5% CPU (records it)
# STAGES="..."   which stages to run (default: fast granularity fastq ssr toolarms)
set -u
export LC_ALL=C
export PATH="$HOME/lx-locked-venv/bin:$HOME/.cargo/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1

W=/tmp/lxn13
LOG="$W/run-window-log.txt"
STAGES="${STAGES:-fast granularity fastq ssr toolarms}"
mkdir -p "$W"

log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" | tee -a "$LOG"; }
stage() { case " $STAGES " in *" $1 "*) return 0;; *) log "skipping stage $1"; return 1;; esac; }

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
# A busy process on this 4-thread host means the machine is in a state other than the
# one the window describes: the first 2026-10-13 attempt found an unrelated bulk
# downloader (aria2c, ~16% CPU) started outside this window, and measuring beside it
# would silently publish a contended machine as an idle one.
heavy="$(ps -eo pcpu,comm --sort=-pcpu --no-headers 2>/dev/null | awk '$1 > 5 {printf "%s %s; ", $1, $2}' | head -c 200)"
if [ -n "$heavy" ] && [ "${ALLOW_LOAD:-0}" != "1" ]; then
  echo "refusing to start: process(es) above 5% CPU: $heavy" >&2
  echo "wait them out, or pass ALLOW_LOAD=1 to record the contention in the log." >&2
  exit 3
fi
log "busy processes at start: ${heavy:-none}"
for attempt in $(seq 1 24); do
  load="$(cut -d' ' -f1 /proc/loadavg)"
  case "$load" in 0.*) log "load settled at $load"; break;; esac
  log "waiting for load to settle (now $load)"; sleep 5
done

echo "=== code label (branch, commit, release status, tree cleanliness) ==="
LABEL="$W/code-label.txt"
{
  echo "branch:     $(git rev-parse --abbrev-ref HEAD)"
  echo "commit:     $(git rev-parse HEAD)"
  echo "describe:   $(git describe --tags --always --dirty)"
  echo "release:    no release tag exists for this project yet; describe carries the short commit hash instead of a tag"
  echo "tree:       $(if [ -z "$(git status --porcelain)" ]; then echo clean; else echo "DIRTY:"; git status --porcelain; fi)"
  echo "lock:       sha256 $(sha256sum workflows/org.linxira.benchmark-python/requirements.lock | cut -d' ' -f1)"
  echo "python:     $(python3 -V 2>&1) at $(command -v python3)"
  echo "biopython:  $(python3 -c 'import Bio; print(Bio.__version__)' 2>/dev/null || echo unavailable)"
  echo "numpy:      $(python3 -c 'import numpy; print(numpy.__version__)' 2>/dev/null || echo unavailable)"
  echo "R:          $(R --version | head -1)"
  echo "Biostrings: $(Rscript --vanilla -e 'cat(as.character(packageVersion("Biostrings")))' 2>/dev/null)"
} | tee "$LABEL"

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

echo "=== build (the binary records its own branch, commit and describe label) ==="
cargo build --release -p linxira-bio-cli -p linxira-bio-worker 2>&1 | tail -2
log "built at branch=$(git rev-parse --abbrev-ref HEAD) $(git rev-parse --short HEAD) describe=$(git describe --tags --always --dirty)"

echo "=== stages: $STAGES ==="
if stage fast; then bash scripts/benchmark/windows/2026-10-13/fast.sh; fi
if stage granularity; then bash scripts/benchmark/windows/2026-10-13/granularity.sh; fi
if stage fastq; then bash scripts/benchmark/windows/2026-10-13/fastq.sh; fi
if stage ssr; then bash scripts/benchmark/windows/2026-10-13/ssr.sh; fi
if stage toolarms; then bash scripts/benchmark/windows/2026-10-13/tool-arms.sh; fi
log "loadavg at end: $(cut -d' ' -f1-3 /proc/loadavg)"

echo "=== archive (summary, tables, bilingual figures, figure check) ==="
bash scripts/benchmark/windows/2026-10-13/archive.sh
cp "$LOG" benchmark-results/2026-10-13/window-log.txt
cp "$LABEL" benchmark-results/2026-10-13/code-label.txt
echo RUN-WINDOW-DONE
