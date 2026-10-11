#!/usr/bin/env python3
# ---------------------------------------------------------------------------
# Benchmark tooling — scripts/benchmark/analysis/summarize-reports.py
# Role: build `benchmark-results/<date>/summary.json` + `summary.md` from per-capability reports and an environment descriptor, validating against the summary schema
# Windows: 2026-09-13, 2026-09-14, 2026-09-18, 2026-10-02, 2026-10-11
# Direction: archive assembly (the entry point every window uses)
# Provenance index: scripts/benchmark/README.md
# ---------------------------------------------------------------------------
"""Aggregate `benchmark run` reports into an archival summary.

`benchmark-results/<date>/summary.json` is the machine-checked archival unit
(`schemas/benchmark-summary.schema.json`): a unique report id, one-line
purpose, methodology, full environment disclosure, and a data-source trail in
which every entry points at a declared source. `run-benchmark-linux.sh` writes
it from its own manifest-driven sweep; this script does the same for reports
that were produced elsewhere (a rented box, a NAS, an ad-hoc window) and only
needs the per-capability metadata a human has to state anyway.

Usage:
    python3 scripts/benchmark/analysis/summarize-reports.py \
        --descriptor descriptor.json --reports /tmp/lxthree --date 2026-10-11 \
        --output-dir benchmark-results/2026-10-11

Descriptor (JSON):
    {
      "purpose": "one sentence: what was measured and why",
      "methodology": {"backends_flag": "...", "repeat_flag": "..."},
      "data_sources": [{"id": ..., "origin": ..., "reference": ..., "note": ...}],
      "entries": [{"capability": ..., "id": ..., "dataset": ...,
                   "data_source_id": ..., "report": "<capability>.benchmark.json"}]
    }

Every entry inherits `dataset_class`, per-backend medians, `speedup`,
`memory_saving`, `consistency` and `precision` from its report, so the numbers
in the summary can never drift from the archived reports. Report files and
their `.md` siblings are copied next to the summary.
"""
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

REPORT_ID_PATTERN = "bench-{date}-{sequence:03d}"

# `schemas/benchmark-summary.schema.json` requires these environment keys, but
# a capability report does not always carry them (older engines omit
# `code_revision`; RAM generation, storage topology, and the GPU are facts only
# a human states). The descriptor therefore supplies them explicitly and we
# refuse to write a summary that would fail the schema.
REQUIRED_ENVIRONMENT_KEYS = (
    "os",
    "engine_version",
    "in_container",
    "page_cache",
    "precision",
    "code_revision",
    "gpu",
    "memory",
    "storage",
)


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def next_report_id(results_root: Path, date: str) -> str:
    date8 = date.replace("-", "")
    highest = 0
    for summary in results_root.glob("*/summary.json"):
        try:
            report_id = load_json(summary).get("report_id", "")
        except (OSError, ValueError):
            continue
        parts = report_id.rsplit("-", 1)
        if len(parts) == 2 and parts[0] == f"bench-{date8}" and parts[1].isdigit():
            highest = max(highest, int(parts[1]))
    if highest == 0:
        # Same-day numbered reports also live under a matching date directory.
        for summary in results_root.glob(f"{date}/*.summary.json"):
            parts = summary.stem.rsplit(".", 1)[0].rsplit("-", 1)
            if len(parts) == 2 and parts[1].isdigit():
                highest = max(highest, int(parts[1]))
    return REPORT_ID_PATTERN.format(date=date8, sequence=highest + 1)


def backends_block(report: dict) -> dict:
    block = {}
    for backend in report.get("backends", []):
        ok_runs = sum(1 for run in backend.get("runs", []) if run.get("ok"))
        block[backend["backend"]] = {
            "median_wall_ms": backend.get("median_wall_ms"),
            "median_peak_rss_mb": backend.get("median_peak_rss_mb"),
            "ok_runs": ok_runs,
            "repeats": backend.get("repeats"),
        }
    return block


def build_summary(
    descriptor: dict, reports_directory: Path, results_root: Path, date: str
) -> tuple[dict, list[tuple[Path, Path]]]:
    entries = []
    report_paths = []
    environment: dict = {}
    for specification in descriptor["entries"]:
        report_path = reports_directory / specification["report"]
        report = load_json(report_path)
        environment = environment or dict(report.get("environment") or {})
        entries.append(
            {
                "id": specification["id"],
                "capability": report["capability"],
                "dataset_class": report.get("dataset_class", "other"),
                "dataset": specification["dataset"],
                "data_source_id": specification["data_source_id"],
                "status": "ok" if report.get("consistency") == "consistent" else "partial",
                "consistency": report.get("consistency"),
                "speedup": report.get("speedup"),
                "memory_saving": report.get("memory_saving"),
                "precision": (report.get("environment") or {}).get("precision"),
                "report": specification["report"],
                "backends": backends_block(report),
            }
        )
        report_paths.append((report_path, report_path.with_suffix(".md")))
    environment.update(descriptor.get("environment") or {})
    missing = [key for key in REQUIRED_ENVIRONMENT_KEYS if environment.get(key) in (None, "")]
    if missing:
        raise SystemExit(
            "the summary schema requires these environment fields, which the "
            f"reports do not supply: {', '.join(missing)} — add them to the "
            "descriptor's `environment` block"
        )
    summary = {
        "schema_version": "1",
        "report_id": next_report_id(results_root, date),
        "generated_at": date,
        "purpose": descriptor["purpose"],
        "methodology": descriptor["methodology"],
        "engine": environment.get("engine_version", "unknown"),
        "environment": environment,
        "data_sources": descriptor["data_sources"],
        "entries": entries,
    }
    return summary, report_paths


def render_markdown(summary: dict) -> str:
    lines = [
        f"# Benchmark summary `{summary['report_id']}` — {summary['generated_at']}",
        "",
        f"> {summary['purpose']}",
        "",
        "## Environment",
        "",
    ]
    environment = summary["environment"]
    for key in ("os", "distro", "kernel", "cpu_model", "total_memory_mb", "engine_version",
                "python_version", "r_version", "precision", "code_revision"):
        if environment.get(key) is not None:
            lines.append(f"- {key}: `{environment[key]}`")
    lines += [
        "",
        "## Method",
        "",
    ]
    for key, value in summary["methodology"].items():
        lines.append(f"- {key}: {value}")
    lines += [
        "",
        "## Results",
        "",
        "| capability | dataset | class | rust | python | r | speedup | memory saving | verdict |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]

    def milliseconds(block: dict, backend: str) -> str:
        value = (block.get(backend) or {}).get("median_wall_ms")
        return "—" if value is None else f"{value:,.0f} ms"

    for entry in summary["entries"]:
        backends = entry["backends"]
        saving = entry.get("memory_saving")
        lines.append(
            "| `{capability}` | {dataset} | {dataset_class} | {rust} | {python} | {r} | {speedup} | {saving} | {consistency} |".format(
                capability=entry["capability"],
                dataset=entry["dataset"],
                dataset_class=entry["dataset_class"],
                rust=milliseconds(backends, "rust"),
                python=milliseconds(backends, "python"),
                r=milliseconds(backends, "r"),
                speedup="—" if entry.get("speedup") is None else f"{entry['speedup']:.2f}×",
                saving="—" if saving is None else f"{saving * 100:.1f}%",
                consistency=entry.get("consistency"),
            )
        )
    lines += ["", "## Data sources", ""]
    for source in summary["data_sources"]:
        lines.append(
            f"- `{source['id']}` ({source['origin']}): {source['reference']} — {source.get('note', '')}"
        )
    lines += ["", "## Reports", ""]
    for entry in summary["entries"]:
        lines.append(f"- `{entry['report']}` ({entry['capability']})")
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--descriptor", required=True)
    parser.add_argument("--reports", required=True, help="directory holding <capability>.benchmark.json")
    parser.add_argument("--date", required=True, help="YYYY-MM-DD; must match the output directory name")
    parser.add_argument("--output-dir", required=True)
    arguments = parser.parse_args()

    if Path(arguments.output_dir).name != arguments.date:
        raise SystemExit("--output-dir must be named after --date")
    descriptor = load_json(Path(arguments.descriptor))
    results_root = Path(arguments.output_dir).resolve().parent
    summary, report_paths = build_summary(
        descriptor, Path(arguments.reports), results_root, arguments.date
    )
    declared = {source["id"] for source in summary["data_sources"]}
    missing = [entry["data_source_id"] for entry in summary["entries"] if entry["data_source_id"] not in declared]
    if missing:
        raise SystemExit(f"entries reference undeclared data sources: {missing}")

    output = Path(arguments.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    for source, markdown in report_paths:
        shutil.copy2(source, output / source.name)
        if markdown.is_file():
            shutil.copy2(markdown, output / markdown.name)
    (output / "summary.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    (output / "summary.md").write_text(render_markdown(summary), encoding="utf-8")
    print(f"wrote {summary['report_id']} with {len(summary['entries'])} entries to {output}")


if __name__ == "__main__":
    main()
