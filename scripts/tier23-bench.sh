#!/usr/bin/env bash
# tier23-bench — batched cross-check of SDK expression.quantify.v1 against
# stored reference quant outputs, with per-run CPU accounting.
#
# Batched: every --runs run is pulled, unpacked, quantified, verified, and
# cleaned inside one serial pass; completed runs are skipped on re-invocation
# so a batch can be resumed after a reboot without redoing finished samples.
# Per run the CSV records wall/user/sys seconds (bash time keyword), the CPU
# model, the pinned core set, and the salmon thread count, so CPU utilization
# is reconstructible: util = (user+sys)/wall.
#
# Usage:
#   tier23-bench --cli PATH --index DIR --sra-dir DIR --reference-dir DIR \
#                --output-dir DIR --tmp-dir DIR --runs RUN [RUN ...] \
#                [--csv PATH] [--pin "taskset -c 8-15 nice -n 10"] \
#                [--cores 8] [--threads 8] [--pull-source DIR|HOST:DIR] \
#                [--scp-identity FILE] [--fasterq PATH]
set -euo pipefail

CLI=""; INDEX=""; SRA_DIR=""; REFERENCE_DIR=""; OUTPUT_DIR=""; TMP_DIR=""
CSV=""; PIN=""; PULL_SOURCE=""; SCP_IDENTITY=""; FASTERQ="fasterq-dump"
CORES=8; THREADS=8; CPU_MODEL=""
RUNS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --cli) CLI="$2"; shift 2 ;;
    --index) INDEX="$2"; shift 2 ;;
    --sra-dir) SRA_DIR="$2"; shift 2 ;;
    --reference-dir) REFERENCE_DIR="$2"; shift 2 ;;
    --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
    --tmp-dir) TMP_DIR="$2"; shift 2 ;;
    --csv) CSV="$2"; shift 2 ;;
    --pin) PIN="$2"; shift 2 ;;
    --cores) CORES="$2"; shift 2 ;;
    --threads) THREADS="$2"; shift 2 ;;
    --pull-source) PULL_SOURCE="$2"; shift 2 ;;
    --scp-identity) SCP_IDENTITY="$2"; shift 2 ;;
    --fasterq) FASTERQ="$2"; shift 2 ;;
    --runs) shift; while [ $# -gt 0 ] && [ "${1#-}" = "$1" ]; do RUNS+=("$1"); shift; done ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

[ -x "$CLI" ] || { echo "CLI not executable: $CLI" >&2; exit 4; }
[ -d "$INDEX" ] || { echo "index missing: $INDEX" >&2; exit 4; }
[ ${#RUNS[@]} -gt 0 ] || { echo "no --runs given" >&2; exit 4; }
read -ra PIN_WORDS <<< "$PIN"
CPU_MODEL=$(lscpu 2>/dev/null | awk '/Model name/ {$1="";$2=""; sub(/^  +/,""); print; exit}')
CSV="${CSV:-$OUTPUT_DIR/benchmark.csv}"
mkdir -p "$OUTPUT_DIR" "$TMP_DIR" "$(dirname "$CSV")"
[ -f "$CSV" ] || echo "run,unpack_wall_s,quant_wall_s,quant_user_s,quant_sys_s,cpu_util_pct,cpu_model,cores_pinned,threads,quant_lines,tpm_pearson_r,numreads_diff,verdict" > "$CSV"

validate() { # validate <ours.sf> <reference.sf> -> "r numdiff verdict"
  python3 - "$1" "$2" <<'PY'
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
rows_ok = set(ours) == set(theirs)
no = sum(v[1] for v in ours.values())
nt = sum(v[1] for v in theirs.values())
numdiff = abs(no - nt) / max(nt, 1.0)
n = len(ours)
sx = sy = sxx = syy = sxy = 0.0
for key in ours:
    x = ours[key][0]; y = theirs[key][0]
    sx += x; sy += y; sxx += x*x; syy += y*y; sxy += x*y
cov = (n*sxx - sx*sx) * (n*syy - sy*sy)
r = ((n*sxy - sx*sy) / math.sqrt(cov)) if cov > 0 else 0.0
ok = rows_ok and numdiff < 0.01 and r >= 0.995
print(f"{r:.6f} {numdiff:.3e} {'PASS' if ok else 'FAIL'}")
PY
}

for run in "${RUNS[@]}"; do
  outdir="$OUTPUT_DIR/$run"
  if [ -f "$outdir/quant.sf" ]; then echo "SKIP $run"; continue; fi
  mkdir -p "$outdir" "$TMP_DIR/$run"
  sra="$SRA_DIR/$run.sra"
  fq="$TMP_DIR/$run-fq"

  if [ -s "$sra" ]; then
    echo "== $run: sra present =="
  elif [ -n "$PULL_SOURCE" ]; then
    echo "== $run: pulling =="
    if [[ "$PULL_SOURCE" == *:* ]]; then
      scp_args=(-q)
      [ -n "$SCP_IDENTITY" ] && scp_args+=(-i "$SCP_IDENTITY")
      scp "${scp_args[@]}" "$PULL_SOURCE/$run" "$sra"
    else
      cp "$PULL_SOURCE/$run" "$sra"
    fi
  else
    echo "MISS $run (no local SRA and no --pull-source)" >&2
    rm -rf "$TMP_DIR/$run"
    continue
  fi

  start=$(date +%s)
  if ! "${PIN_WORDS[@]}" "$FASTERQ" -e 8 --force -O "$fq" "$sra" \
      > "$outdir/unpack.log" 2>&1; then
    end=$(date +%s)
    echo "$run,$((end-start)),0,0,0,0,$CPU_MODEL,$CORES,$THREADS,0,0,0,UNPACK-FAILED" >> "$CSV"
    echo "UNPACK-FAIL $run: $(tail -1 "$outdir/unpack.log")"
    rm -rf "$fq" "$TMP_DIR/$run"
    continue
  fi
  end=$(date +%s)
  unpack_wall=$((end-start))

  start=$(date +%s)
  set +e
  TIMEFORMAT='%3R %3U %3S'
  { time "${PIN_WORDS[@]}" "$CLI" expression quantify \
      "$fq/${run}_1.fastq" "$fq/${run}_2.fastq" \
      --index "$INDEX" --threads "$THREADS" --seq-bias --gc-bias \
      --output "$outdir/quant.sf" --json \
      > "$outdir/quantify.json" 2> "$outdir/quantify.log"; } 2> "$outdir/cpu.time"
  rc=$?
  set -e
  end=$(date +%s)
  read -r q_wall q_user q_sys < "$outdir/cpu.time"
  rm -rf "$fq" "$TMP_DIR/$run"
  if [ $rc -ne 0 ]; then
    echo "$run,$unpack_wall,$((end-start)),${q_user},${q_sys},0,$CPU_MODEL,$CORES,$THREADS,0,0,0,QUANT-FAILED" >> "$CSV"
    echo "QUANT-FAIL $run: $(tail -1 "$outdir/quantify.log")"
    continue
  fi
  quant_wall=$((end-start))
  util=$(python3 -c "print(f'{100*($q_user+$q_sys)/max($quant_wall,1):.1f}')")
  lines=$(($(wc -l < "$outdir/quant.sf") - 1))

  ref="$REFERENCE_DIR/$run/quant.sf"
  if [ ! -f "$ref" ]; then
    echo "$run,$unpack_wall,$quant_wall,${q_user},${q_sys},${util},$CPU_MODEL,$CORES,$THREADS,$lines,0,0,NO-REFERENCE" >> "$CSV"
    echo "DONE $run unpack=${unpack_wall}s quant=${quant_wall}s (no reference)"
    continue
  fi
  read -r r numdiff v <<< "$(validate "$outdir/quant.sf" "$ref")"
  echo "$run,$unpack_wall,$quant_wall,${q_user},${q_sys},${util},$CPU_MODEL,$CORES,$THREADS,$lines,${r},${numdiff},${v}" >> "$CSV"
  echo "DONE $run unpack=${unpack_wall}s quant=${quant_wall}s cpu=${util}% r=$r numdiff=$numdiff $v"
done

echo "=== BATCH_DONE $(date) ==="
