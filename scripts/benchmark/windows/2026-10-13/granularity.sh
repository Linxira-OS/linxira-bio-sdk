# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-13/granularity.sh
# Role: the record-shape experiment (1x100 Mb / 20x5 Mb / 100k x 1 kb) of the locked-environment re-run
# Windows: 2026-10-13
# Direction: archived beside the window summary, read by the figures through --granularity
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
export LC_ALL=C
export PATH="$HOME/lx-locked-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1
BIN=./target/release/linxira-bio
export LINXIRA_BIO_WORKER="$PWD/target/release/linxira-bio-worker"
W=/tmp/lxn13
mkdir -p "$W"

echo "=== granularity experiment inputs (same ~100 Mb, three record shapes) ==="
python3 - <<'PY'
import pathlib
import numpy as np
W = pathlib.Path('/tmp/lxn13')
rng = np.random.default_rng(42)
letters = np.array(list("ACGT"))

def write_fasta(path, records, bases_each):
    sequence = rng.choice(letters, size=records * bases_each)
    with path.open('w') as fh:
        for index in range(records):
            fh.write(f">rec_{index}\n")
            chunk = "".join(sequence[index * bases_each:(index + 1) * bases_each])
            for offset in range(0, len(chunk), 60):
                fh.write(chunk[offset:offset + 60] + "\n")
    return path.stat().st_size

print("1x100Mb:", write_fasta(W/'g_one.fa', 1, 100_000_000), "bytes")
print("20x5Mb:", write_fasta(W/'g_twenty.fa', 20, 5_000_000), "bytes")
print("100k x 1kb:", write_fasta(W/'g_many.fa', 100_000, 1000), "bytes")
PY
ls -lh "$W"/g_*.fa | awk '{print "  ", $9, $5}'

show() {
python3 -c "
import json,sys
d=json.load(sys.stdin)['result']
print('  verdict', d['consistency'])
for b in d['backends']:
    runs=[r['wall_ms'] for r in b.get('runs',[]) if r.get('ok')]
    print(f\"    {b['backend']:7} median {b['median_wall_ms']:10.3f} ms  iqr {b['iqr_wall_ms']:8.3f}  min {min(runs) if runs else 0:9.3f}  max {max(runs) if runs else 0:9.3f}\")
"
}

for shape in one twenty many; do
  echo "=== sequence.stats.v1 on g_$shape.fa (N=10) ==="
  $BIN benchmark run sequence.stats.v1 fasta="$W/g_$shape.fa" --backends rust,python,r --repeat 10 \
    --output "$W/out-gran-$shape" --dataset-class sequence --json 2>/dev/null | show
done
echo GRANULARITY-DONE
