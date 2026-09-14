#!/usr/bin/env bash
# Auto-resume the tier26 SDK benchmark batch after host reboot/power-on.
# Installed as a user-level systemd oneshot (Linger=yes) — does not touch
# the mainline pipeline or any system unit.
set -u
WS=/mnt/F/biosdk_ws
LOG=$WS/bench/boot-resume.log
CSV=$WS/bench/tier26-benchmark.csv
LOCK=$WS/tmp/tier26.lock
RUNS=$(tr '\n' ' ' < "$WS/pending26.txt")
exec 9>"$LOCK"
flock -n 9 || { echo "$(date) another instance holds the lock" >> "$LOG"; exit 0; }
echo "=== BOOT-RESUME $(date) ===" >> "$LOG"
# Wait for network + NAS availability (max ~10 min).
for _ in $(seq 1 30); do
  ping -c1 -W3 192.168.11.172 >/dev/null 2>&1 && break
  sleep 20
done
# Settle window: let the mainline worker come up first.
sleep 120
done_count=$(grep -cE ',(PASS|NO-REFERENCE)$' "$CSV" 2>/dev/null || true)
done_count=${done_count:-0}
if [ "${done_count}" -ge 26 ]; then
  echo "BATCH-COMPLETE (26 rows accepted); nothing to do" >> "$LOG"
  exit 0
fi
echo "resuming: ${done_count}/26 accepted" >> "$LOG"
export LINXIRA_BIO_SALMON="$WS/tools-env/bin/salmon"
bash "$WS/linxira-bio-sdk/scripts/tier23-bench.sh" \
  --cli "$WS/linxira-bio-sdk/target/release/linxira-bio" \
  --index /mnt/G/thesis_dwf4/analysis/02_tier1/salmon_idx \
  --sra-dir "$WS/sra-inbox" \
  --reference-dir /mnt/G/thesis_dwf4/analysis/15_tier23_quant \
  --output-dir "$WS/results" \
  --tmp-dir "$WS/tmp" \
  --csv "$CSV" \
  --pin "taskset -c 8-15 nice -n 10" \
  --cores 8 --threads 8 \
  --pull-source "bhys@192.168.11.172:/mnt/disk1/tier23" \
  --scp-identity /home/bhys/nas_key \
  --fasterq "$WS/tools-env/bin/fasterq-dump" \
  --runs $RUNS >> "$LOG" 2>&1
echo "=== BOOT-RESUME-END $(date) ===" >> "$LOG"
