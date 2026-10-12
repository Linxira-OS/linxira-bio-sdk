#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/reproduce.sh
# Role: the one-command reproduction entry point for the published benchmark windows
# Windows: 2026-10-11 (clean N=10 three-backend window, archive bench-20261011-001)
# Direction: an outside reader must be able to reach a verdict in minutes, and to reproduce
#            the whole window in hours, without reading our scripts first
# Provenance index: scripts/benchmark/README.md · REPRODUCE.md
# ---------------------------------------------------------------------------
#
# Two levels, on purpose:
#
#   --smoke  (default, ~2-5 minutes, no benchmarking; the granularity set dominates)
#       regenerates the archived inputs from the pinned seed, compares their sha256 with
#       benchmark-results/2026-10-11/inputs.sha256, reports the committed revision the archive
#       was produced with, and prints a loud diff when the installed interpreter packages
#       differ from the pack locks. This is the check that catches "their inputs are not
#       reproducible" / "their environment is not what the lock says".
#
#   --full   (hours; run it on the pinned machine, not on a laptop)
#       runs the same digest check, then prints the exact window commands and the expected
#       headline medians so a reproducer can compare their own numbers with ours row by row.
#       It deliberately does NOT hide the long run inside this script: an N=10 three-backend
#       window is a scheduled measurement, not a convenience command.
#
# Exit status: 0 when every check performed here passes, 1 otherwise. Nothing is written
# outside --work (default: a temporary directory).
set -u

LEVEL=smoke
WORK="${TMPDIR:-/tmp}/lx-reproduce"
ARCHIVE="benchmark-results/2026-10-11"
PY="${PYTHON:-python3}"

while [ $# -gt 0 ]; do
  case "$1" in
    --smoke) LEVEL=smoke ;;
    --full)  LEVEL=full ;;
    --work)  WORK="$2"; shift ;;
    --help|-h)
      sed -n '2,30p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
  shift
done

fail=0
say()  { printf '%s\n' "$*"; }
ok()   { printf '  \033[32mPASS\033[0m %s\n' "$*"; }
bad()  { printf '  \033[31mFAIL\033[0m %s\n' "$*"; fail=1; }
warn() { printf '  \033[33mWARN\033[0m %s\n' "$*"; }

command -v "$PY" >/dev/null 2>&1 || { say "python not found: set PYTHON=/path/to/python3"; exit 2; }
[ -d "$ARCHIVE" ] || { say "run this from the repository root (missing $ARCHIVE)"; exit 2; }

say "== 1. which revision produced the archive =="
rev="$($PY - "$ARCHIVE" <<'PY'
import json, sys, pathlib
for report in sorted(pathlib.Path(sys.argv[1]).glob("*.benchmark.json")):
    env = json.loads(report.read_text(encoding="utf-8")).get("environment", {})
    if env.get("code_revision"):
        print(env["code_revision"]); break
PY
)"
head_rev="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
say "  archived code_revision : ${rev:-<not recorded>}"
say "  your HEAD              : $head_rev"
if [ -n "${rev:-}" ] && [ "$head_rev" != "$rev" ]; then
  warn "HEAD differs from the archived revision - reproduce with: git checkout $rev"
else
  ok "checkout matches the archived revision"
fi

say "== 2. inputs regenerate byte-identically =="
mkdir -p "$WORK"
"$PY" scripts/benchmark/inputs/generate-inputs.py --set synthetic --out "$WORK" >/dev/null || bad "synthetic inputs could not be generated"
"$PY" scripts/benchmark/inputs/generate-inputs.py --set granularity --out "$WORK" >/dev/null 2>&1 || warn "granularity inputs skipped (needs numpy)"
"$PY" - "$ARCHIVE/inputs.sha256" "$WORK" <<'PY'
import hashlib, pathlib, sys
archived = {}
for line in pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    if line.strip():
        digest, name = line.split()[0], line.split()[-1].lstrip("*")
        archived[pathlib.Path(name).name] = digest
work = pathlib.Path(sys.argv[2])
checked = matched = missing = 0
for name, digest in sorted(archived.items()):
    candidate = work / name
    if not candidate.is_file():
        print(f"  SKIP {name} (produced by the --set sequence/reads family)")
        missing += 1
        continue
    checked += 1
    got = hashlib.sha256(candidate.read_bytes()).hexdigest()
    if got == digest:
        matched += 1
        print(f"  PASS {name} {got[:12]}")
    else:
        print(f"  FAIL {name} regenerated {got[:12]} != archived {digest[:12]}")
print(f"  {matched}/{checked} regenerated inputs match the archive ({missing} not in this set)")
raise SystemExit(0 if matched == checked and checked else 1)
PY
[ $? -eq 0 ] || fail=1

say "== 3. interpreter arms: installed packages vs the pack locks =="
"$PY" scripts/benchmark/analysis/verify-lock-env.py \
  --lock workflows/org.linxira.benchmark-python/requirements.lock || \
  warn "the lock does not describe this environment - see REPRODUCE.md > Known gaps"

if [ "$LEVEL" = full ]; then
  say "== 4. the full window (scheduled measurement, not a convenience command) =="
  cat <<'TXT'
  On the pinned machine (Linux, governor=performance or the disclosed powersave floor), after
  regenerating the remaining inputs (--set sequence / --set reads, needs a release build):

    cargo build --release
    python3 scripts/benchmark/inputs/generate-inputs.py --set sequence --out "$WORK" --binary target/release/linxira-bio
    python3 scripts/benchmark/inputs/generate-inputs.py --set reads    --out "$WORK" --binary target/release/linxira-bio
    bash scripts/benchmark/windows/2026-10-11/fast.sh          # six fast capabilities, N=10
    bash scripts/benchmark/windows/2026-10-11/fastq.sh
    bash scripts/benchmark/windows/2026-10-11/ssr.sh           # ~37 min per R repeat
    bash scripts/benchmark/analysis/statistics-tables.py --summary ... --reports ... --comparison ...
    python3 scripts/benchmark/figures/verify-figures.py --figures benchmark-results/2026-10-11/blog-mirror/figures/zh ...

  Compare row by row against benchmark-results/2026-10-11/statistics.tsv:

TXT
  "$PY" - "$ARCHIVE/statistics.tsv" <<'PY'
import csv, pathlib, sys
rows = list(csv.DictReader(pathlib.Path(sys.argv[1]).open(encoding="utf-8"), delimiter="\t"))
for row in rows:
    print(f"    {row['capability']:34s} {row['backend']:7s} median {row['median_wall_ms']:>10s} ms "
          f"ratio vs rust {row['ratio_vs_rust']:>8s} CI [{row['ratio_ci_low']}, {row['ratio_ci_high']}] "
          f"{row['verdict']}")
PY
  say "  Expected mismatches are the interesting part: report them at"
  say "  https://github.com/Linxira-OS/linxira-bio-sdk/issues with the reproduction command and your environment."
fi

say ""
if [ "$fail" -eq 0 ]; then
  say "smoke reproduction: PASS (inputs regenerate from the pinned seed; see REPRODUCE.md for the full window)"
else
  say "smoke reproduction: FAIL - nothing above should be trusted until this is explained"
fi
exit "$fail"
