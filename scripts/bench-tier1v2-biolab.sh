#!/bin/bash
# Bio SDK tier1_v2 benchmark batch (bench-20260914-002).
# Parallel-implementation cross-check against the thesis pipeline's own
# salmon 2.7.0 outputs. Constraints honored: cores 8-15 nice 10, serial,
# never touches /mnt/G writes or the main queue; reads NAS read-only.
set -u
WS=/mnt/F/biosdk_ws
SDK=$WS/linxira-bio-sdk
CLI=$SDK/target/release/linxira-bio
IDX=/mnt/G/thesis_dwf4/analysis/02_tier1/salmon_idx
NAS=/mnt/disk2/tier1_v2
V2Q=/mnt/G/thesis_dwf4/analysis/04_v2_quant/quant
QUAR=/mnt/G/thesis_dwf4/analysis/quarantine/unverified_SRR
IN=$WS/input
OUT=$WS/results
BENCH=$WS/bench
export LINXIRA_BIO_SALMON=$WS/tools-env/bin/salmon

mkdir -p "$IN" "$OUT" "$BENCH"
CSV=$BENCH/benchmark.csv
[ -f "$CSV" ] || echo "run,impl,wall_s,peak_rss_kb,quant_lines,numreads_total,tpm_pearson_r,rows_match,verdict" > "$CSV"

PIN="taskset -c 8-15 nice -n 10"

verify() { # verify <ours.sf> <theirs.sf> -> appends verdict fields, echoes "r=..,match=.."
  python3 - "$1" "$2" <<'PY'
import math
import sys

def load(path):
    rows = {}
    with open(path) as handle:
        next(handle)
        for line in handle:
            parts = line.split("\t")
            rows[parts[0]] = (float(parts[3]), float(parts[4]))
    return rows

ours = load(sys.argv[1])
theirs = load(sys.argv[2])
rows_match = set(ours) == set(theirs)
num_ours = sum(v[1] for v in ours.values())
num_theirs = sum(v[1] for v in theirs.values())
num_diff = abs(num_ours - num_theirs) / max(num_theirs, 1.0)
common = sorted(set(ours) & set(theirs))
n = len(common)
sx = sy = sxx = syy = sxy = 0.0
for key in common:
    x = ours[key][0]; y = theirs[key][0]
    sx += x; sy += y; sxx += x*x; syy += y*y; sxy += x*y
denom = math.sqrt((n*sxx - sx*sx) * (n*syy - sy*sy)) if (n*sxx - sx*sx) > 0 and (n*syy - sy*sy) > 0 else 0.0
r = ((n*sxy - sx*sy) / denom) if denom else 0.0
print(f"{r:.6f},{int(rows_match)},{num_diff:.6f}")
PY
}

run_pair() { # run_pair <run> <mate1.nas> <mate2.nas-or-empty> <theirs.sf>
  local run=$1 m1=$2 m2=$3 theirs=$4
  local outdir=$OUT/$run
  [ -f "$outdir/quant.sf" ] && { echo "SKIP $run (already done)"; return 0; }
  mkdir -p "$outdir"
  echo "== $run =="
  local start=$(date +%s)
  if [ -n "$m2" ]; then
    $PIN "$CLI" expression quantify "$m1" "$m2" --index "$IDX" --threads 8 \
      --output "$outdir/quant.sf" --json > "$outdir/quantify.json" 2> "$outdir/quantify.log"
  else
    $PIN "$CLI" expression quantify "$m1" --index "$IDX" --threads 8 \
      --output "$outdir/quant.sf" --json > "$outdir/quantify.json" 2> "$outdir/quantify.log"
  fi
  local rc=$?
  local end=$(date +%s)
  if [ $rc -ne 0 ]; then
    echo "$run,rust,$((end-start)),,0,,,0,FAILED" >> "$CSV"
    echo "FAIL $run"; tail -1 "$outdir/quantify.log"; return 1
  fi
  local lines=$(($(wc -l < "$outdir/quant.sf") - 1))
  local verdict
  if [ -f "$theirs" ]; then
    local cmpout=$(verify "$outdir/quant.sf" "$theirs")
    local r=$(echo "$cmpout" | cut -d, -f1)
    local rowsm=$(echo "$cmpout" | cut -d, -f2)
    local numdiff=$(echo "$cmpout" | cut -d, -f3)
    local ok
    python3 -c "import sys; r=float('$r'); d=float('$numdiff'); rowsm='$rowsm'=='1'; sys.exit(0 if (rowsm and d<0.01 and r>=0.995) else 1)" && ok=PASS || ok=FAIL
    verdict="r=$r rows=$rowsm numdiff=$numdiff $ok"
    echo "$run,rust,$((end-start)),,$lines,,${r},${rowsm},${ok}" >> "$CSV"
  else
    verdict="no-legacy-reference"
    echo "$run,rust,$((end-start)),,$lines,,,," >> "$CSV"
  fi
  echo "DONE $run wall=$((end-start))s $verdict"
}

# 14 single-end runs staged on /mnt/G (the mainline's own quant inputs);
# paired NAS runs are deferred until a NAS->lab key path exists.
for run in SRR8205656 SRR8205657 SRR8205658 SRR8205659 SRR8205660 SRR8205661 SRR8205662 SRR8205663 \
           SRR28573920 SRR28573921 SRR28573922 SRR28573923 SRR28573924 SRR28573925; do
  m1="/mnt/G/thesis_dwf4/analysis/04_v2_quant/fastq/${run}.fastq.gz"
  [ -s "$m1" ] || { echo "MISS $m1"; continue; }
  theirs="$V2Q/$run/quant.sf"
  [ -f "$theirs" ] || theirs=""
  run_pair "$run" "$m1" "" "$theirs"
done

echo "=== BATCH_DONE $(date) ==="
