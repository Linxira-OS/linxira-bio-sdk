#!/bin/bash
# Docking study v2: /usr/bin/time absent in container -> date-based timing.
exec > /root/dock-study2.log 2>&1
set -x
echo $$ > /root/dock-study.pid
cd /root/autodl-tmp || exit 8
BIN=/root/autodl-tmp/linxira-bio-sdk/target/release/linxira-bio
REC=bundle/docking/1iep_receptor.pdbqt
LIG=bundle/docking/1iep_ligand.pdbqt
CENTER="--center-x 15.190 --center-y 53.903 --center-z 16.917 --size-x 20 --size-y 20 --size-z 20"
cd study || exit 8
run() { # label, extra args...
  local label=$1; shift
  local t0=$(date +%s.%N)
  $BIN chemistry dock $REC $LIG ${label}.pdbqt $CENTER "$@" --json > ${label}.json
  local rc=$?
  local t1=$(date +%s.%N)
  echo "WALL_${label}=$(echo "$t1 $t0" | awk '{printf "%.2f", $1-$2}')s" >> walls.txt
  echo "${label}-EXIT=$rc"
}
> walls.txt
for E in 8 16 32 64; do
  for SEED in 42 43 44; do
    run A_e${E}_s${SEED} --exhaustiveness $E --seed $SEED --cpu 32
  done
done
for CPU in 1 8 32 128; do
  run B_cpu${CPU} --exhaustiveness 8 --seed 42 --cpu $CPU
done
cd /root/autodl-tmp
$BIN simulate sequence big128.fa --count 1 --length 1000000 --seed 42 --json > sim128.json
for RUN in 1 2 3; do
  t0=$(date +%s.%N)
  $BIN sequence ssr big128.fa D_rust_${RUN}.tsv --backend rust --json > D_rust_${RUN}.json
  echo "WALL_D_rust_${RUN}=$(echo "$(date +%s.%N) $t0" | awk '{printf "%.3f", $1-$2}')s" >> study/walls.txt
done
pip3 install -q numpy pytrf -i https://pypi.tuna.tsinghua.edu.cn/simple 2>&1 | tail -1
for RUN in 1 2 3; do
  t0=$(date +%s.%N)
  $BIN sequence ssr big128.fa D_py_${RUN}.tsv --backend python --json > D_py_${RUN}.json
  echo "WALL_D_py_${RUN}=$(echo "$(date +%s.%N) $t0" | awk '{printf "%.3f", $1-$2}')s" >> study/walls.txt
done
cmp study/D_rust_1.tsv study/D_py_1.tsv && echo "D-1M-IDENTICAL"
python3 /root/autodl-tmp/analyze_dock.py /root/autodl-tmp/bundle/docking /root/autodl-tmp/study > analysis.txt 2>&1
echo "ANALYZE-EXIT=$?"
echo "STUDY2-DONE $(date +%H:%M:%S)"
