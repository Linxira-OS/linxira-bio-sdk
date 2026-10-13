# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/windows/2026-10-13/tool-arms.sh
# Role: the ecosystem-tool arms (arm_class=tool) measured in the same session as the window: pyfastx, R prcomp, scikit-learn PCA, and seqkit once its argv is verified
# Windows: 2026-10-13
# Direction: run by run-window.sh after the parity stages; reports land in benchmark-results/2026-10-13/tool-arms/
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
# These are the calls a user would actually make for the same job, measured on the same
# host, inputs and protocol as the parity arms. They are archived separately: ROADMAP 7.1
# keeps the two arm classes out of one table, and a tool-arm report stores no ratio
# against our engine (speedup/memory_saving stay null). Each report now carries the
# measured input's path, size and sha256, so the number can be re-derived from the file.
set -u
export LC_ALL=C
export PATH="$HOME/bin:$HOME/lx-locked-venv/bin:$PATH"
cd "$HOME/linxira-bio-sdk" || exit 1
W=/tmp/lxn13
OUT=benchmark-results/2026-10-13/tool-arms
mkdir -p "$OUT"

python3 scripts/benchmark/analysis/verify-lock-env.py \
  --lock workflows/org.linxira.benchmark-python/requirements.lock || {
    echo "refusing to measure: interpreter environment differs from requirements.lock" >&2
    exit 2
  }

arm() {
  key="$1"; input="$2"; cls="$3"
  echo "=== tool arm: $key on $input ==="
  if ! python3 scripts/benchmark/run/tool-arms.py --tool "$key" --input "$input" \
       --repeat 10 --dataset-class "$cls" --output "$OUT"; then
    echo "tool arm $key failed (recorded; the window continues)" >&2
  fi
}

arm pyfastx-stat "$W/big.fa" sequence
arm r-prcomp "$W/matrix.tsv" matrix
arm py-sklearn-pca "$W/matrix.tsv" matrix

# seqkit joins only through a verified registry entry: the registry accepts an argv
# after it has been checked against the installed version, so a missing entry means
# "not verified yet", not "absent".
if python3 scripts/benchmark/run/tool-arms.py --list 2>/dev/null | grep -q seqkit; then
  arm seqkit-stats "$W/big.fa" sequence
else
  echo "seqkit: no verified registry entry yet; skipped (argv is checked against the installed version first)"
fi
echo TOOL-ARMS-DONE
