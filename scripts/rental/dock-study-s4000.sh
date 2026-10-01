#!/bin/bash
# S4000 (15-core Xeon Gold 6430) docking study: same design as vGPU instance,
# thread sweep capped at 15 cores. Same bundle, same seeds, same analysis.
exec > /root/dock-s4000.log 2>&1
set -x
echo $$ > /root/dock-s4000.pid
cd /root/autodl-tmp || exit 8
BIN=/root/autodl-tmp/linxira-bio-sdk/target/release/linxira-bio
REC=/root/autodl-tmp/bundle/docking/1iep_receptor.pdbqt
LIG=/root/autodl-tmp/bundle/docking/1iep_ligand.pdbqt
CENTER="--center-x 15.190 --center-y 53.903 --center-z 16.917 --size-x 20 --size-y 20 --size-z 20"
mkdir -p study && cd study
run() {
  local label=$1; shift
  local t0=$(date +%s.%N)
  $BIN chemistry dock $REC $LIG ${label}.pdbqt $CENTER "$@" --json > ${label}.json
  local rc=$?
  echo "WALL_${label}=$(echo "$(date +%s.%N) $t0" | awk '{printf "%.2f", $1-$2}')s" >> walls.txt
  echo "${label}-EXIT=$rc"
}
> walls.txt
for E in 8 16 32 64; do
  for SEED in 42 43 44; do
    run A_e${E}_s${SEED} --exhaustiveness $E --seed $SEED --cpu 15
  done
done
for CPU in 1 4 15; do
  run B_cpu${CPU} --exhaustiveness 8 --seed 42 --cpu $CPU
done
python3 - <<'PYEOF'
import sys
sys.argv = ["x", "/root/autodl-tmp/bundle/docking", "/root/autodl-tmp/study"]
import glob, json, os
import numpy as np
def parse_pdbqt_models(path):
    models, coords = [], []
    for line in open(path):
        if line.startswith(("ATOM", "HETATM")):
            coords.append((float(line[30:38]), float(line[38:46]), float(line[46:54])))
        elif line.startswith("MODEL"):
            coords = []
        elif line.startswith(("ENDMDL", "END")):
            if coords: models.append(np.array(coords)); coords = []
    if coords: models.append(np.array(coords))
    return models
ref = parse_pdbqt_models(os.path.join(sys.argv[1], "1iep_ligand_vina_out.pdbqt"))[0]
rows = ["run\trmsd_vs_official_A"]
for p in sorted(glob.glob("*.pdbqt")):
    m = parse_pdbqt_models(p)
    if m and m[0].shape == ref.shape:
        rows.append("%s\t%.3f" % (p[:-6], float(np.sqrt(np.mean(np.sum((m[0]-ref)**2, axis=1))))))
open("rmsd_vs_official.tsv", "w").write("\n".join(rows) + "\n")
print("\n".join(rows))
PYEOF
echo "ANALYZE-EXIT=$?"
echo "S4000-STUDY-DONE $(date +%H:%M:%S)"
