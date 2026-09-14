#!/usr/bin/env python3
"""Summarize a tier23/quantify cross-check CSV into a 3-sigma archival
summary (bench-YYYYMMDD-NNN.summary.json).

Reads the per-run CSV produced by tier23-bench.sh / quantify-bench.sh,
computes distribution statistics over the passing runs (TPM Pearson r,
NumReads relative difference, wall times), applies the 3-sigma principle
(flags any run beyond 3 sigma of its metric distribution), and writes the
archival summary JSON with full provenance. Everything site-specific comes
from CLI arguments.

Usage:
  python3 tier23-summarize.py --csv bench.csv --report-id bench-20260914-004 \
      --generated-at 2026-09-14 --output bench-20260914-004.summary.json \
      --engine 1.0.3 --code-revision <sha> \
      --os-name linux --kernel <ver> --distro <name> --cpu-model <model> \
      --total-memory-mb 31744 --gpu <gpu> --memory <dimm> --storage <disks> \
      --purpose "..." --dataset-class srr
"""
from __future__ import annotations

import argparse
import csv
import json
import math
import statistics
from pathlib import Path


def stats_block(values: list[float]) -> dict:
    if not values:
        return {"n": 0}
    mean = statistics.fmean(values)
    stdev = statistics.stdev(values) if len(values) > 1 else 0.0
    lower, upper = mean - 3 * stdev, mean + 3 * stdev
    outliers = [v for v in values if not (lower <= v <= upper)]
    return {
        "n": len(values),
        "mean": mean,
        "stdev": stdev,
        "sigma_3_interval": [lower, upper],
        "outliers_beyond_3sigma": len(outliers),
        "min": min(values),
        "max": max(values),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--csv", required=True)
    parser.add_argument("--report-id", required=True)
    parser.add_argument("--generated-at", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--engine", required=True)
    parser.add_argument("--code-revision", required=True)
    parser.add_argument("--os-name", required=True)
    parser.add_argument("--kernel", required=True)
    parser.add_argument("--distro", required=True)
    parser.add_argument("--cpu-model", required=True)
    parser.add_argument("--total-memory-mb", type=int, required=True)
    parser.add_argument("--gpu", required=True)
    parser.add_argument("--memory", required=True)
    parser.add_argument("--storage", required=True)
    parser.add_argument("--purpose", required=True)
    parser.add_argument("--dataset-class", default="srr")
    parser.add_argument(
        "--data-source", action="append", default=[],
        help="id=origin=reference=note (repeatable)")
    args = parser.parse_args()

    with open(args.csv, newline="") as handle:
        rows = list(csv.DictReader(handle))
    passed = [r for r in rows if r["verdict"] == "PASS"]
    failed = [r for r in rows if r["verdict"] not in ("PASS", "SKIP")]

    # Tolerate both header generations: CPU-accounting rows carry
    # numdiff_rel/quant_user_s/...; the first pre-CPU rows use numdiff_rel
    # too but may lack the CPU columns entirely.
    def field(row, name):
        value = row.get(name, "")
        return float(value) if value not in ("", None) else 0.0

    r_values = [float(r["tpm_pearson_r"]) for r in passed]
    numdiffs = [float(r["numdiff_rel"]) for r in passed]
    quant_walls = [float(r["quant_wall_s"]) for r in passed]
    unpack_walls = [float(r["unpack_wall_s"]) for r in passed]

    r_stats = stats_block(r_values)
    # 3-sigma acceptance on the correlation distribution: the batch passes
    # only if every run is within 3 sigma and the worst r still clears the
    # 0.995 acceptance line.
    worst_r = min(r_values) if r_values else 0.0
    sigma_ok = r_stats.get("outliers_beyond_3sigma", 1) == 0 and worst_r >= 0.995

    entries = []
    for r in passed:
        entries.append({
            "id": r["run"],
            "capability": "expression.quantify.v1",
            "dataset_class": args.dataset_class,
            "dataset": (f"{r['run']} SRA -> fasterq-dump -> CLI quantify "
                        f"(-l A -p 8 --validateMappings --seqBias --gcBias)"),
            "data_source_id": r["run"],
            "status": "ok",
            "consistency": (f"TPM r={r['tpm_pearson_r']}, NumReads relative "
                            f"difference {r['numreads_diff']}, rows {r['quant_lines']}"),
            "speedup": None,
            "memory_saving": None,
            "precision": "second-resolution wall clock",
            "report": None,
            "backends": {
                "rust": {
                    "median_wall_ms": float(r["quant_wall_s"]) * 1000.0,
                    "median_peak_rss_mb": None,
                    "ok_runs": 1,
                    "repeats": 1,
                }
            },
            "cpu": {
                "model": r.get("cpu_model", ""),
                "cores_pinned": r.get("cores_pinned", ""),
                "threads": r.get("threads", ""),
                "quant_user_s": field(r, "quant_user_s"),
                "quant_sys_s": field(r, "quant_sys_s"),
                "quant_util_pct": field(r, "cpu_util_pct"),
            },
        })

    data_sources = []
    for raw in args.data_source:
        source_id, origin, reference, *rest = raw.split("=", 3)
        data_sources.append({
            "id": source_id,
            "origin": origin,
            "reference": reference,
            "note": rest[0] if rest else "",
        })

    summary = {
        "schema_version": "1",
        "report_id": args.report_id,
        "generated_at": args.generated_at,
        "purpose": args.purpose,
        "methodology": {
            "backends_flag": "rust-orchestrated upstream salmon (see engine)",
            "repeat_flag": "1 per run (batched serial cross-check)",
            "warmup": "none; cold cache per segment",
            "page_cache": "cold",
            "disclosure": ("3-sigma principle: per-metric distributions over "
                           "passing runs are reported as mean/stdev/3-sigma "
                           "interval; batch-level PASS requires zero outliers "
                           "beyond 3 sigma and worst TPM r >= 0.995"),
        },
        "engine": args.engine,
        "environment": {
            "os": args.os_name,
            "kernel": args.kernel,
            "cpu_model": args.cpu_model,
            "total_memory_mb": args.total_memory_mb,
            "distro": args.distro,
            "gpu": args.gpu,
            "memory": args.memory,
            "storage": args.storage,
            "engine_version": args.engine,
            "code_revision": args.code_revision,
            "in_container": False,
            "page_cache": "cold",
            "precision": "high (second-resolution wall clock, bash time user/sys)",
        },
        "data_sources": data_sources,
        "entries": entries,
        "statistics": {
            "tpm_pearson_r": r_stats,
            "numreads_rel_diff": stats_block(numdiffs),
            "quant_wall_s": stats_block(quant_walls),
            "unpack_wall_s": stats_block(unpack_walls),
            "worst_tpm_pearson_r": worst_r,
            "sigma_3_acceptance": "PASS" if sigma_ok else "FAIL",
            "runs_passed": len(passed),
            "runs_failed": len(failed),
        },
    }
    Path(args.output).write_text(
        json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"wrote {args.output}: {len(passed)} passed, {len(failed)} failed, "
          f"3-sigma acceptance {'PASS' if sigma_ok else 'FAIL'} "
          f"(worst r {worst_r:.6f})")


if __name__ == "__main__":
    main()
