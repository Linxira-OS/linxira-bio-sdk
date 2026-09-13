#!/usr/bin/env bash
# run-benchmark-linux.sh — one-command benchmark matrix for the Linux server
# (ROADMAP M2-T6).
#
# Reads a local dataset manifest (`benchmark-datasets.json`, intentionally
# NOT committed: it maps capabilities to local 4TB data paths), runs
# `linxira-bio benchmark run` for every entry with warmup + timed repeats,
# writes consistent results back into the repo's runtime preference table,
# and aggregates everything into `benchmark-results/<date>/summary.json`
# plus a human-readable `summary.md` with the comparison table, the data
# advantage list, the environment disclosure, and the §7.1 methodology
# disclosure (warm-cache only; never mix with cold-cache numbers).
#
# Usage:
#   scripts/run-benchmark-linux.sh [--datasets FILE] [--output DIR]
#       [--backends rust,python,r] [--repeat N] [--cli PATH]
#       [--skip-writeback]
#
# Exit codes: 0 all benchmark commands ran, 3 at least one CLI invocation
# failed, 2 usage, 4 environment (missing CLI/python/manifest).

set -uo pipefail

usage() {
  sed -n '2,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

CLI=""
DATASETS="benchmark-datasets.json"
OUTPUT="benchmark-results"
REPEAT="3"
BACKENDS="rust,python,r"
SKIP_WRITEBACK=0

while [ $# -gt 0 ]; do
  case "$1" in
    --cli) CLI="${2:?--cli requires a path}"; shift 2 ;;
    --datasets) DATASETS="${2:?--datasets requires a path}"; shift 2 ;;
    --output) OUTPUT="${2:?--output requires a path}"; shift 2 ;;
    --repeat) REPEAT="${2:?--repeat requires a number}"; shift 2 ;;
    --backends) BACKENDS="${2:?--backends requires a list}"; shift 2 ;;
    --skip-writeback) SKIP_WRITEBACK=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage; exit 2 ;;
  esac
done

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
PYTHON="${PYTHON:-python3}"

if ! command -v "$PYTHON" >/dev/null 2>&1; then
  echo "python3 is required for planning and aggregation" >&2
  exit 4
fi
if [ ! -f "$DATASETS" ]; then
  echo "dataset manifest not found: $DATASETS" >&2
  echo "copy benchmark-datasets.example.json to benchmark-datasets.json and point" >&2
  echo "the inputs at your local data; the real manifest is intentionally not committed" >&2
  exit 4
fi
if [ -z "$CLI" ]; then
  if command -v linxira-bio >/dev/null 2>&1; then
    CLI="linxira-bio"
  elif [ -x "$ROOT/target/release/linxira-bio" ]; then
    CLI="$ROOT/target/release/linxira-bio"
  else
    echo "linxira-bio not found on PATH or target/release; pass --cli PATH" >&2
    exit 4
  fi
fi

STAMP=$(date -u +%Y%m%d-%H%M%S)
RUN_DIR="$OUTPUT/$STAMP"
mkdir -p "$RUN_DIR/plan" "$RUN_DIR/runs" || exit 4

# Turn the JSON manifest into one shell fragment per run; shlex quoting
# keeps paths with spaces safe. Each fragment defines plan_* variables.
"$PYTHON" - "$DATASETS" "$RUN_DIR/plan" "$REPEAT" "$BACKENDS" <<'PYPLAN'
import json
import pathlib
import shlex
import sys

manifest_path, plan_dir, default_repeat, default_backends = sys.argv[1:5]
manifest = json.loads(pathlib.Path(manifest_path).read_text(encoding="utf-8"))
entries = manifest.get("datasets")
if not isinstance(entries, list) or not entries:
    raise SystemExit("dataset manifest needs a non-empty 'datasets' array")
for index, entry in enumerate(entries):
    capability = entry.get("capability")
    inputs = entry.get("inputs")
    if not capability or not isinstance(inputs, dict) or not inputs:
        raise SystemExit(
            f"dataset entry {index} needs 'capability' and a non-empty 'inputs' object"
        )
    args = ["benchmark", "run", capability]
    for role, path in inputs.items():
        args.append(f"{role}={path}")
    name = entry.get("id") or capability
    fragment = (
        f"plan_slug={shlex.quote(f'{index:04d}-{name}')}\n"
        f"plan_capability={shlex.quote(capability)}\n"
        f"plan_class={shlex.quote(str(entry.get('dataset_class', 'other')))}\n"
        f"plan_repeat={shlex.quote(str(entry.get('repeat', default_repeat)))}\n"
        f"plan_backends={shlex.quote(str(entry.get('backends', default_backends)))}\n"
        f"plan_source={shlex.quote(str(entry.get('source_url', '')))}\n"
        f"plan_args=({' '.join(shlex.quote(argument) for argument in args)})\n"
    )
    safe_name = "".join(character if character.isalnum() or character in "-._" else "_"
                        for character in name)
    (pathlib.Path(plan_dir) / f"{index:04d}-{safe_name}.plan.sh").write_text(
        fragment, encoding="utf-8"
    )
PYPLAN
if [ $? -ne 0 ]; then
  echo "dataset manifest is invalid" >&2
  exit 4
fi

COMPLETED=0
COMMAND_FAILURES=0
BENCH_FAILURES=0
for fragment in "$RUN_DIR"/plan/*.plan.sh; do
  # shellcheck source=/dev/null
  source "$fragment"
  slug_dir="$RUN_DIR/runs/$plan_slug"
  echo "== $plan_capability [$plan_class] -> $slug_dir"
  mkdir -p "$slug_dir"
  # plan_args already carries the "benchmark run <capability>" subcommand.
  if "$CLI" "${plan_args[@]}" \
      --backends "$plan_backends" \
      --repeat "$plan_repeat" \
      --output "$slug_dir" \
      --dataset-class "$plan_class" > "$slug_dir/run.log" 2>&1; then
    COMPLETED=$((COMPLETED + 1))
  else
    COMMAND_FAILURES=$((COMMAND_FAILURES + 1))
    echo "   benchmark command failed (exit $?); see $slug_dir/run.log" >&2
  fi
  report="$slug_dir/$plan_capability.benchmark.json"
  if [ -f "$report" ]; then
    verdict=$("$PYTHON" -c 'import json,sys;print(json.load(open(sys.argv[1])).get("consistency",""))' "$report")
    if [ "$verdict" != "consistent" ]; then
      BENCH_FAILURES=$((BENCH_FAILURES + 1))
    fi
    if [ "$SKIP_WRITEBACK" -ne 1 ] && [ "$verdict" = "consistent" ]; then
      if "$PYTHON" "$SCRIPT_DIR/update-runtime-preferences.py" \
          "$report" --file "$ROOT/runtime-preferences.json"; then
        echo "   preference table updated"
      else
        echo "   preference write-back failed; continuing" >&2
      fi
    fi
  else
    BENCH_FAILURES=$((BENCH_FAILURES + 1))
    echo "   no benchmark report was produced" >&2
  fi
done

"$PYTHON" - "$RUN_DIR" "$DATASETS" "$BACKENDS" "$REPEAT" "$CLI" <<'PYAGG'
import json
import pathlib
import sys

run_dir = pathlib.Path(sys.argv[1])
manifest_path = sys.argv[2]
backends_flag = sys.argv[3]
repeat_flag = sys.argv[4]
cli = sys.argv[5]

manifest = json.loads(pathlib.Path(manifest_path).read_text(encoding="utf-8"))
entries_manifest = manifest.get("datasets", [])

entries = []
environment = None
engine_version = None
reports = sorted(run_dir.glob("runs/*/*.benchmark.json"))
for index, manifest_entry in enumerate(entries_manifest):
    name = manifest_entry.get("id") or manifest_entry.get("capability")
    matches = [report for report in reports if report.parent.name.endswith(f"-{name}")]
    report_path = matches[0] if matches else None
    record = {
        "id": name,
        "capability": manifest_entry.get("capability"),
        "dataset_class": manifest_entry.get("dataset_class", "other"),
        "source_url": manifest_entry.get("source_url", ""),
        "status": "missing-report",
        "report": str(report_path.relative_to(run_dir)) if report_path else None,
    }
    if report_path:
        report = json.loads(report_path.read_text(encoding="utf-8"))
        report_environment = report.get("environment") or {}
        environment = environment or report_environment
        engine_version = engine_version or report_environment.get("engine_version")
        record["status"] = "ok" if report.get("consistency") == "consistent" else "failed"
        record["consistency"] = report.get("consistency")
        record["speedup"] = report.get("speedup")
        record["memory_saving"] = report.get("memory_saving")
        record["precision"] = report.get("environment", {}).get("precision")
        record["backends"] = {
            backend.get("backend"): {
                "median_wall_ms": backend.get("median_wall_ms"),
                "median_peak_rss_mb": backend.get("median_peak_rss_mb"),
                "ok_runs": sum(1 for run in backend.get("runs", []) if run.get("ok")),
                "repeats": len(backend.get("runs", [])),
            }
            for backend in report.get("backends", [])
        }
    entries.append(record)

summary = {
    "schema_version": "1",
    "generated_at": run_dir.name,
    "methodology": {
        "backends_flag": backends_flag,
        "repeat_flag": repeat_flag,
        "warmup": "one untimed warmup per backend before the timed repeats",
        "page_cache": "warm",
        "disclosure": "warm-cache numbers must not be mixed with cold-cache runs (ROADMAP 7.1)",
    },
    "engine": engine_version or "unknown",
    "environment": environment,
    "entries": entries,
}
(run_dir / "summary.json").write_text(
    json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
)

lines = []
lines.append(f"# Benchmark summary — {run_dir.name}")
lines.append("")
lines.append(f"- backends: {backends_flag} — repeat: {repeat_flag}")
lines.append(f"- datasets: `{manifest_path}` (local, intentionally not committed)")
lines.append(f"- engine: {summary['engine']}")
lines.append("- methodology: one untimed warmup per backend, median of timed repeats, "
             "page_cache: warm; precision per report (`high` with /usr/bin/time -v, "
             "`degraded` otherwise). Warm-cache numbers must not be mixed with "
             "cold-cache runs (ROADMAP §7.1).")
lines.append("")

backend_names = [name.strip() for name in backends_flag.split(",") if name.strip()]
header = "| capability | class | consistency | " + " | ".join(
    f"{name} median (ms)" for name in backend_names
) + " | speedup | memory saving |"
lines.append(header)
lines.append("|" + "---|" * (3 + len(backend_names) + 2))
for record in entries:
    if record["status"] != "ok":
        lines.append(
            f"| {record['capability']} | {record['dataset_class']} | "
            f"**{record['status']}** | " + " | ".join(["n/a"] * len(backend_names))
            + " | n/a | n/a |"
        )
        continue
    cells = []
    for name in backend_names:
        backend = record["backends"].get(name)
        cells.append(f"{backend['median_wall_ms']:.1f}" if backend else "n/a")
    speedup = f"{record['speedup']:.2f}x" if record.get("speedup") else "n/a"
    saving = (
        f"{record['memory_saving'] * 100:.1f}%"
        if record.get("memory_saving") is not None else "n/a"
    )
    lines.append(
        f"| {record['capability']} | {record['dataset_class']} | "
        f"{record.get('consistency', 'n/a')} | " + " | ".join(cells) + f" | {speedup} | {saving} |"
    )

lines.append("")
lines.append("## Data advantages")
lines.append("")
advantages = [
    record for record in entries
    if record["status"] == "ok"
    and (record.get("speedup") or record.get("memory_saving") is not None)
]
if advantages:
    for record in advantages:
        parts = []
        if record.get("speedup"):
            parts.append(f"{record['speedup']:.2f}x faster than rust")
        if record.get("memory_saving") is not None:
            parts.append(f"{record['memory_saving'] * 100:.1f}% more peak RSS than rust")
        source = f" (source: {record['source_url']})" if record.get("source_url") else ""
        lines.append(f"- {record['capability']} [{record['dataset_class']}]: "
                     + ", ".join(parts) + source)
else:
    lines.append("- none recorded: every consistent run had rust as the fastest backend")

lines.append("")
lines.append("## Environment")
lines.append("")
if environment:
    kernel = environment.get("kernel") or ""
    lines.append(f"- os: {environment.get('os')}" + (f" {kernel}" if kernel else ""))
    lines.append(f"- cpu: {environment.get('cpu_model', 'unknown')}")
    memory = environment.get("total_memory_mb")
    lines.append(f"- memory: {memory / 1024:.1f} GiB" if memory else "- memory: unknown")
    lines.append(f"- engine: {environment.get('engine_version')}")
    lines.append(f"- python: {environment.get('python_version', 'n/a')}")
    lines.append(f"- R: {environment.get('r_version', 'n/a')}")
    lines.append(f"- container: {environment.get('in_container')}")
else:
    lines.append("- no benchmark produced a report; nothing to disclose")
lines.append("")

failures = [record for record in entries if record["status"] != "ok"]
if failures:
    lines.append("## Failures")
    lines.append("")
    for record in failures:
        lines.append(f"- {record['capability']} [{record['dataset_class']}]: {record['status']}; "
                     f"see {record['report'] or 'run.log'}")
    lines.append("")

(run_dir / "summary.md").write_text("\n".join(lines), encoding="utf-8")
print(f"wrote {run_dir / 'summary.json'}")
print(f"wrote {run_dir / 'summary.md'}")
PYAGG
if [ $? -ne 0 ]; then
  echo "aggregation failed" >&2
  exit 4
fi

echo "benchmark matrix: $COMPLETED run(s), $COMMAND_FAILURES command failure(s), $BENCH_FAILURES failed verdict(s)"
if [ "$COMMAND_FAILURES" -gt 0 ]; then
  exit 3
fi
exit 0
