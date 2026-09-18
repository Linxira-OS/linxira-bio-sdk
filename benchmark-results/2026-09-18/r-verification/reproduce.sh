#!/usr/bin/env bash
set -uo pipefail
WS=<sdk-workspace>
SDK=$WS/engine/target/release/linxira-bio
FD=$WS/tools-env/bin/fasterq-dump
KEY=/home/bhys/nas_key
W=$WS/tmp/r-demo
export LINXIRA_BIO_SALMON=<sdk-workspace>/tools-env/bin/salmon
mkdir -p "$W"
echo "$(date +%T) pull" >> $WS/bench/r-demo.log
scp -q -B -i "$KEY" -r <user>@<lan-host>:/mnt/disk1/tier23/SRR1460477 "$W/" || echo PULL_FAIL
sra=$W/SRR1460477
[ -s "$sra" ] || sra=$(find "$W" -type f -size +1G | head -1)
echo "$(date +%T) fasterq" >> $WS/bench/r-demo.log
"$FD" -O "$W" --split-files "$sra" >> $WS/bench/r-demo.log 2>&1
gzip -f "$W"/*_1.fastq "$W"/*_2.fastq
echo "$(date +%T) quantify with bias flags" >> $WS/bench/r-demo.log
nice -n 10 "$SDK" expression quantify "$W"/SRR1460477_1.fastq.gz "$W"/SRR1460477_2.fastq.gz \
  --index <study-data>/analysis/02_tier1/salmon_idx --threads 6 \
  --seq-bias --gc-bias --output "$W/quant_bias.sf" --json > "$W/quant_summary.json" 2>&1
echo "$(date +%T) compare" >> $WS/bench/r-demo.log
python3 - <<'PYEOF'
import numpy as np
def load(p):
    t, q = {}, {}
    with open(p) as fh:
        next(fh)
        for line in fh:
            n, _, _, tpm, nr = line.rstrip("\n").split("\t")
            t[n] = float(tpm); q[n] = float(nr)
    return t, q
a, aq = load("<sdk-workspace>/tmp/r-demo/quant_bias.sf")
b, bq = load("<study-data>/analysis/15_tier23_quant/SRR1460477/quant.sf")
common = sorted(set(a) & set(b))
va = np.array([a[k] for k in common]); vb = np.array([b[k] for k in common])
print("LIVE_R=%.9f" % np.corrcoef(va, vb)[0, 1])
print("MAX_ABS_DTPM=%.3g" % float(np.max(np.abs(va - vb))))
ident = sum(1 for k in common if aq[k] == bq[k])
print("NUMREADS_IDENTICAL=%d/%d" % (ident, len(common)))
PYEOF
echo "$(date +%T) DEMO DONE" >> $WS/bench/r-demo.log
