#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-13/resume-after-reboot.sh
# Role: finish the 2026-10-13 window after the measuring host rebooted with /tmp (tmpfs) wiped: restore the staged reports, regenerate the seeded inputs, measure the tool arms and the environment-isolation cells, then build the archive
# Windows: 2026-10-13 (resume, executed 2026-10-14)
# Direction: detached on the measuring host; the repo-relative outputs land in benchmark-results/2026-10-13/
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
# Why a resume script exists at all: the window's eight capabilities finished on
# 2026-10-13 and their reports were copied out of tmpfs before the host was handed
# to the maintainer's night jobs, but the tool arms and the archive step still had
# to run. The host rebooted the next morning, so the scratch directory and the
# handover guard's log are gone; the staged copy is the source of truth, and the
# inputs are regenerated from the same seeds (their digests are what the archive
# records, so regenerating is the point of the generator, not a workaround).
#
# Order matters and is intentional: measurements first (they need an idle host and
# must not run beside anything, including an observing ssh session), post-processing
# afterwards (the archive only reads reports and needs the renderers, not the lock).
#
# Run detached:
#     cat scripts/benchmark/windows/2026-10-13/resume-after-reboot.sh \
#       | ssh <host> 'cat > ~/resume-1013.sh && chmod +x ~/resume-1013.sh &&
#                     setsid nohup ~/resume-1013.sh > ~/resume-1013.out 2>&1 & echo detached'
set -u
set -o pipefail

# ``--post-only``: the measurements of this window are already done (they were run in
# one idle slot); re-running them would burn the same slot again and change numbers that
# are already recorded. The flag restores the inputs and rebuilds the archive only.
POST_ONLY=0
[ "${1:-}" = "--post-only" ] && POST_ONLY=1
export LC_ALL=C
export PATH="$HOME/bin:$HOME/.cargo/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1

W=/tmp/lxn13
S="$HOME/lxn-window-2026-10-13-staged"
OUT=benchmark-results/2026-10-13
LOG="$HOME/lxn-resume-2026-10-14.log"
mkdir -p "$W" "$OUT"
log() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" | tee -a "$LOG"; }

log "resume: repo $(git rev-parse --abbrev-ref HEAD) $(git rev-parse --short HEAD) ($(git describe --tags --always --dirty))"

log "restoring the staged reports into $W (tmpfs was wiped by the 2026-10-14 reboot)"
for d in "$S"/out-*; do cp -r "$d" "$W"/; done
cp "$S/run-window-log.txt" "$S/code-label.txt" "$W"/ 2>/dev/null || true
log "reports staged: $(ls -d "$W"/out-* 2>/dev/null | wc -l)"

missing=0
for f in big.fa reads200.fq matrix.tsv sets.tsv structure.pdb variants.vcf genes.txt associations.tsv g_one.fa g_twenty.fa g_many.fa; do
  [ -f "$W/$f" ] || missing=1
done
if [ "$missing" = "1" ]; then
  log "regenerating the seeded inputs (the archive records their digests, so they must be the same bytes)"
  # The granularity FASTAs come from the generator's numpy branch (default_rng(42)), so
  # that set needs an interpreter that has numpy - the locked one does, the ambient
  # python3 need not (and did not, which is why the first attempt left them missing).
  PY_NUMPY="$HOME/lx-locked-venv/bin/python3"
  [ -x "$PY_NUMPY" ] || PY_NUMPY="$HOME/lx-venv/bin/python"
  [ -x "$PY_NUMPY" ] || PY_NUMPY=python3
  log "granularity inputs use: $PY_NUMPY ($("$PY_NUMPY" -c 'import numpy; print("numpy", numpy.__version__)' 2>&1 | tail -1))"
  cargo build --release -p linxira-bio-cli -p linxira-bio-worker 2>&1 | tail -2 | tee -a "$LOG"
  python3 scripts/benchmark/inputs/generate-inputs.py --set synthetic --out "$W" 2>&1 | tail -2 | tee -a "$LOG"
  "$PY_NUMPY" scripts/benchmark/inputs/generate-inputs.py --set granularity --out "$W" 2>&1 | tail -2 | tee -a "$LOG"
  python3 scripts/benchmark/inputs/generate-inputs.py --set sequence --out "$W" --binary "$PWD/target/release/linxira-bio" 2>&1 | tail -2 | tee -a "$LOG"
  python3 scripts/benchmark/inputs/generate-inputs.py --set reads --out "$W" --binary "$PWD/target/release/linxira-bio" 2>&1 | tail -2 | tee -a "$LOG"
else
  log "inputs already present"
fi
log "scratch: $(du -sh "$W" | cut -f1)"

# A measurement window must not contain an observer: the connection that launched
# this script is still open for a moment, and its shell shows up as >5% CPU when a
# gate samples at exactly that instant.
log "waiting 60 s before measuring, so no observing session is inside the window"
sleep 60

if [ "$POST_ONLY" = "1" ]; then
  log "post-only: skipping the tool arms and the environment-isolation cells (already measured in one idle slot)"
else
  if bash scripts/benchmark/windows/2026-10-13/tool-arms.sh; then
    log "tool arms: ok"
  else
    log "tool arms: FAILED (exit $?)"
  fi

  if bash scripts/benchmark/windows/2026-10-13/env-isolation.sh; then
    log "env-isolation: ok"
  else
    log "env-isolation: FAILED (exit $?)"
  fi
fi

# Post-processing is standard-library only - the figure renderer writes SVG by hand,
# so there is no matplotlib dependency anywhere in the archive chain.

if bash scripts/benchmark/windows/2026-10-13/archive.sh; then
  log "archive: ok"
else
  log "archive: FAILED (exit $?)"
fi

# The tool arms are their own class: a separate table, never merged into the parity
# tables (ROADMAP 7.1).
if [ -d "$OUT/tool-arms" ]; then
  log "writing the class-labelled tool-arm table"
  python3 scripts/benchmark/analysis/statistics-tables.py \
    --summary "$OUT/summary.json" --reports "$OUT" --output-dir "$OUT" \
    --comparison benchmark-results/2026-10-11/summary.json \
    --tool-arms "$OUT/tool-arms" 2>&1 | tail -3 | tee -a "$LOG"
fi

# The window's own label and log belong in its archive; the guard log lived in
# tmpfs and was lost in the reboot, so the handover time is evidenced by the staged
# copy's timestamps instead (recorded in the archive README by hand).
cp "$S/code-label.txt" "$OUT/code-label.txt" 2>/dev/null || true
cp "$S/run-window-log.txt" "$OUT/window-log.txt" 2>/dev/null || true
log "archive contents: $(ls "$OUT" | tr '\n' ' ')"
log "RESUME-DONE"
