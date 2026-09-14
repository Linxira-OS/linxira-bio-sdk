#!/usr/bin/env bash
# quantify-bench — cross-check `linxira-bio expression quantify` against an
# existing salmon quant directory.
#
# Usage:
#   quantify-bench --cli PATH --index DIR --inputs-dir DIR --reference-dir DIR \
#                  --output-dir DIR [--runs RUN ...] [--csv PATH] [--pin CORESET]
#
# Every sample is quantified through the public CLI (`expression quantify`)
# and compared against a reference quant.sf on three acceptability lines:
#   1. row/identifier parity (same transcript universe),
#   2. NumReads total within 1%,
#   3. TPM Pearson r >= 0.995 over all rows.
# The reference implementation is disclosed per run (cmd_info.json), so a
# reference produced by a salmon re-implementation rather than upstream
# salmon shows up in the report instead of being silently blended in.
#
# No site-specific paths are hardcoded: everything comes from arguments or
# environment (LINXIRA_BIO_SALMON for the salmon binary, as documented in
# docs/capabilities/expression.quantify.v1).
set -euo pipefail

CLI=""
INDEX=""
INPUTS_DIR=""
REFERENCE_DIR=""
OUTPUT_DIR=""
CSV=""
PIN=""
RUNS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --cli) CLI="${2:?--cli requires a path}"; shift 2 ;;
    --index) INDEX="${2:?--index requires a directory}"; shift 2 ;;
    --inputs-dir) INPUTS_DIR="${2:?--inputs-dir requires a directory}"; shift 2 ;;
    --reference-dir) REFERENCE_DIR="${2:?--reference-dir requires a directory}"; shift 2 ;;
    --output-dir) OUTPUT_DIR="${2:?--output-dir requires a directory}"; shift 2 ;;
    --csv) CSV="${2:?--csv requires a path}"; shift 2 ;;
    --pin) PIN="${2:?--pin requires a core list, e.g. 8-15}"; shift 2 ;;
    --runs) shift; while [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; do RUNS+=("$1"); shift; done ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

[ -x "$CLI" ] || { echo "CLI not executable: $CLI" >&2; exit 4; }
[ -d "$INDEX" ] || { echo "index directory missing: $INDEX" >&2; exit 4; }
[ -d "$INPUTS_DIR" ] || { echo "inputs directory missing: $INPUTS_DIR" >&2; exit 4; }
[ -d "$REFERENCE_DIR" ] || { echo "reference directory missing: $REFERENCE_DIR" >&2; exit 4; }
CSV="${CSV:-$OUTPUT_DIR/benchmark.csv}"
mkdir -p "$OUTPUT_DIR" "$(dirname "$CSV")"
[ -f "$CSV" ] || echo "run,impl,wall_s,quant_lines,numreads_total,tpm_pearson_r,rows_match,verdict,reference" > "$CSV"

RUNS=${RUNS[@]:-$(cd "$INPUTS_DIR" && ls | sed -E 's/_(1|2)?\.fastq\.gz$//' | sort -u)}

for run in $RUNS; do
  outdir="$OUTPUT_DIR/$run"
  if [ -f "$outdir/quant.sf" ]; then echo "SKIP $run"; continue; fi
  mkdir -p "$outdir"

  m1="$INPUTS_DIR/${run}_1.fastq.gz"
  m2="$INPUTS_DIR/${run}_2.fastq.gz"
  single="$INPUTS_DIR/${run}.fastq.gz"
  start=$(date +%s)
  set +e
  if [ -s "$m1" ] && [ -s "$m2" ]; then
    $PIN "$CLI" expression quantify "$m1" "$m2" --index "$INDEX" --threads 8 \
      --output "$outdir/quant.sf" --json > "$outdir/quantify.json" 2> "$outdir/quantify.log"
  elif [ -s "$single" ]; then
    $PIN "$CLI" expression quantify "$single" --index "$INDEX" --threads 8 \
      --output "$outdir/quant.sf" --json > "$outdir/quantify.json" 2> "$outdir/quantify.log"
  else
    echo "MISS $run (no fastq in $INPUTS_DIR)" >&2
    set -e
    continue
  fi
  rc=$?
  set -e
  end=$(date +%s)
  if [ $rc -ne 0 ]; then
    echo "$run,rust,$((end-start)),0,,,0,FAILED,none" >> "$CSV"
    echo "FAIL $run (exit $rc): $(tail -1 "$outdir/quantify.log")"
    continue
  fi

  lines=$(($(wc -l < "$outdir/quant.sf") - 1))
  ref=""
  [ -f "$REFERENCE_DIR/$run/quant.sf" ] && ref="$REFERENCE_DIR/$run/quant.sf"
  if [ -z "$ref" ]; then
    echo "$run,rust,$((end-start)),$lines,,,,NO-REFERENCE,none" >> "$CSV"
    echo "DONE $run wall=$((end-start))s (no reference quant.sf)"
    continue
  fi

  cmpout=$(python3 - "$outdir/quant.sf" "$ref" "$REFERENCE_DIR/$run" <<'PY'
import json
import math
import sys

def load(path):
    rows = {}
    with open(path) as handle:
        next(handle)
        for line in handle:
            parts = line.rstrip("\n").split("\t")
            rows[parts[0]] = (float(parts[3]), float(parts[4]))
    return rows

ours = load(sys.argv[1])
theirs = load(sys.argv[2])
rows_match = int(set(ours) == set(theirs))
num_ours = sum(v[1] for v in ours.values())
num_theirs = sum(v[1] for v in theirs.values())
num_diff = abs(num_ours - num_theirs) / max(num_theirs, 1.0)
n = len(ours)
sx = sy = sxx = syy = sxy = 0.0
for key in ours:
    x = ours[key][0]; y = theirs[key][0]
    sx += x; sy += y; sxx += x*x; syy += y*y; sxy += x*y
cov = (n*sxx - sx*sx) * (n*syy - sy*sy)
r = ((n*sxy - sx*sy) / math.sqrt(cov)) if cov > 0 else 0.0
impl = "unknown"
try:
    with open(sys.argv[3] + "/cmd_info.json") as handle:
        impl = json.load(handle).get("salmon_version", "unknown")
except OSError:
    pass
print(f"{r:.6f} {rows_match} {num_diff:.6f} {impl}")
PY
  )
  r=$(echo "$cmpout" | cut -d' ' -f1)
  rowsm=$(echo "$cmpout" | cut -d' ' -f2)
  numdiff=$(echo "$cmpout" | cut -d' ' -f3)
  refimpl=$(echo "$cmpout" | cut -d' ' -f4)
  if python3 -c "import sys; sys.exit(0 if ('${rowsm}'=='1' and float('${numdiff}')<0.01 and float('${r}')>=0.995) else 1)"; then
    verdict=PASS
  else
    verdict=FAIL
  fi
  echo "$run,rust,$((end-start)),$lines,,${r},${rowsm},${verdict},${refimpl}" >> "$CSV"
  echo "DONE $run wall=$((end-start))s r=$r rows=$rowsm numdiff=$numdiff ref=$refimpl $verdict"
done

echo "=== BATCH_DONE $(date) ==="