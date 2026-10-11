#!/usr/bin/env python3
"""Emit the standalone statistics tables for an archived benchmark run.

The blog article must be verifiable from the repository: this script writes
tab-separated tables (no spreadsheet formats) next to the summary so any
reader can re-derive every number in the article, compare this window against
an earlier report, and see the evidence behind each "the gap is/is not
resolvable" verdict.

Outputs (all LF, tab-separated, deterministic order):
    statistics.tsv             per capability x backend: n, median, IQR, CV, min, max,
                               peak RSS, ratio vs rust, bootstrap 95% CI, noise band, verdict
    statistics-comparison.tsv  this window vs an earlier report, per capability x metric
    statistics-consistency.tsv cross-backend agreement basis per capability

Usage:
    python3 scripts/benchmark-statistics.py --summary benchmark-results/<date>/summary.json \
        --reports /path/to/reports --output-dir benchmark-results/<date> \
        --comparison benchmark-results/2026-09-13/summary.json
"""
from __future__ import annotations

import argparse
import json
import statistics
from pathlib import Path

BOOTSTRAP_SEED = 20261011
BOOTSTRAP_ROUNDS = 10_000
CONSISTENCY_TOLERANCE = "1e-6 relative (benchmark.run.v1 default)"
# Two-part decision rule, fixed before looking at the numbers:
#   1. a gap is *detectable* when the bootstrap 95% CI of the ratio excludes 1.0;
#   2. it is *material* only when the whole CI sits at or above this ratio.
# A spread-based "noise band" is reported for transparency but is NOT the
# decision rule: run-to-run dispersion is not an effect size, so using it as a
# threshold would label a consistent 2% gap "noise" and a lucky 15% gap
# "signal".
MATERIAL_RATIO = 1.10


def classify_ratio(ratio_low: float, ratio_high: float) -> str:
    if ratio_high <= 1.0:
        return "within-noise"          # CI includes 1.0 (or is entirely below it)
    if ratio_low <= 1.0:
        return "within-noise"
    return "material" if ratio_low >= MATERIAL_RATIO else "detectable-immaterial"


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def median_of(values: list[float]) -> float:
    ordered = sorted(values)
    count = len(ordered)
    if count % 2:
        return ordered[count // 2]
    return statistics.fmean(ordered[count // 2 - 1:count // 2 + 1])


def quartiles(values: list[float]) -> tuple[float, float]:
    ordered = sorted(values)
    count = len(ordered)
    if count < 2:
        return ordered[0], ordered[0]
    middle = count // 2
    lower = ordered[:middle]
    upper = ordered[middle + 1:] if count % 2 else ordered[middle:]
    return median_of(lower or ordered), median_of(upper or ordered)


def cv_percent(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    mean = statistics.fmean(values)
    return statistics.stdev(values) / mean * 100 if mean else 0.0


def bootstrap_ratio(first: list[float], second: list[float], seed: int = BOOTSTRAP_SEED,
                    rounds: int = BOOTSTRAP_ROUNDS) -> tuple[float, float]:
    generator = __import__("random").Random(seed)
    ratios = []
    for _ in range(rounds):
        a = median_of(generator.choices(first, k=len(first)))
        b = median_of(generator.choices(second, k=len(second)))
        ratios.append(b / a if a else float("inf"))
    ratios.sort()
    return ratios[int(0.025 * rounds)], ratios[int(0.975 * rounds)]


def runs_per_backend(report: dict) -> dict[str, list[float]]:
    result = {}
    for backend in report.get("backends", []):
        times = [run["wall_ms"] for run in backend.get("runs", []) if run.get("ok")]
        if times:
            result[backend["backend"]] = times
    return result


def write_tsv(path: Path, header: list[str], rows: list[list[str]]) -> None:
    lines = ["\t".join(header)]
    lines += ["\t".join(row) for row in rows]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def statistics_rows(summary: dict, reports: Path) -> list[list[str]]:
    rows = []
    for entry in summary["entries"]:
        report = load(reports / entry["report"])
        times = runs_per_backend(report)
        baseline = times.get("rust", [])
        for backend in ("rust", "python", "r"):
            values = times.get(backend)
            if not values:
                continue
            lower, upper = quartiles(values)
            ratio, ratio_low, ratio_high, noise, verdict = 1.0, 1.0, 1.0, 0.0, "-"
            if backend != "rust" and baseline:
                ratio = median_of(values) / median_of(baseline)
                ratio_low, ratio_high = bootstrap_ratio(baseline, values)
                noise = 2 * (cv_percent(baseline) + cv_percent(values)) / 100
                verdict = classify_ratio(ratio_low, ratio_high)
            peak = next((b.get("median_peak_rss_mb") for b in report["backends"]
                         if b["backend"] == backend), None)
            rows.append([
                entry["capability"],
                entry["dataset_class"],
                backend,
                str(len(values)),
                f"{median_of(values):.3f}",
                f"{upper - lower:.3f}",
                f"{cv_percent(values):.2f}",
                f"{min(values):.3f}",
                f"{max(values):.3f}",
                "" if peak is None else f"{peak:.3f}",
                f"{ratio:.4f}",
                "" if backend == "rust" else f"{ratio_low:.4f}",
                "" if backend == "rust" else f"{ratio_high:.4f}",
                f"{noise:.4f}",
                verdict,
            ])
    return rows


def comparison_rows(summary: dict, reports: Path, earlier: dict,
                    earlier_reports: Path | None) -> list[list[str]]:
    rows = []
    earlier_by_capability = {entry["capability"]: entry for entry in earlier["entries"]}
    for entry in summary["entries"]:
        capability = entry["capability"]
        previous = earlier_by_capability.get(capability)
        if not previous:
            continue
        for backend in ("rust", "python", "r"):
            now = (entry["backends"].get(backend) or {}).get("median_wall_ms")
            then = (previous["backends"].get(backend) or {}).get("median_wall_ms")
            if now and then:
                rows.append([capability, f"median_wall_ms[{backend}]", earlier["report_id"],
                             f"{then:.3f}", f"{now:.3f}", f"{now / then:.4f}"])
            now_rss = (entry["backends"].get(backend) or {}).get("median_peak_rss_mb")
            then_rss = (previous["backends"].get(backend) or {}).get("median_peak_rss_mb")
            if now_rss and then_rss:
                rows.append([capability, f"peak_rss_mb[{backend}]", earlier["report_id"],
                             f"{then_rss:.3f}", f"{now_rss:.3f}", f"{now_rss / then_rss:.4f}"])
        if entry.get("speedup") and previous.get("speedup"):
            rows.append([capability, "speedup[python]", earlier["report_id"],
                         f"{previous['speedup']:.4f}", f"{entry['speedup']:.4f}",
                         f"{entry['speedup'] / previous['speedup']:.4f}"])
        if entry.get("memory_saving") is not None and previous.get("memory_saving") is not None:
            rows.append([capability, "memory_saving", earlier["report_id"],
                         f"{previous['memory_saving']:.4f}", f"{entry['memory_saving']:.4f}", ""])
    return rows


def consistency_rows(summary: dict, reports: Path, archive_dir: Path) -> list[list[str]]:
    rows = []
    for entry in summary["entries"]:
        report = load(reports / entry["report"])
        backends = ",".join(b["backend"] for b in report.get("backends", []) if
                            any(run.get("ok") for run in b.get("runs", [])))
        relative = f"{archive_dir.as_posix().rstrip('/')}/{entry['report']}"
        rows.append([
            entry["capability"],
            str(report.get("consistency")),
            CONSISTENCY_TOLERANCE,
            str((report.get("environment") or {}).get("precision")),
            backends,
            entry["report"],
            relative,
            "no differing field paths" if not report.get("findings") else
            "; ".join(str(f.get("field")) for f in report["findings"][:5]),
        ])
    return rows


def trimmed_rows(summary: dict, reports: Path) -> tuple[list[list[str]], list[list[str]]]:
    """Robustness of the headline numbers against the trimming rule.

    `benchmark run` already performs one *untimed* warm-up per backend (the
    report only ever contains the timed repeats), so the recorded runs are
    warm. Reviewers may still want the answer to "would dropping the first
    and/or last repeat change the conclusion?" — these two tables make that
    check explicit instead of leaving it to trust.
    """
    medians, verdicts = [], []
    for entry in summary["entries"]:
        report = load(reports / entry["report"])
        times = runs_per_backend(report)
        baseline = times.get("rust", [])
        baseline_variants = {
            "all_n": baseline,
            "drop_first": baseline[1:],
            "middle": baseline[1:-1],
        }
        for backend in ("rust", "python", "r"):
            values = times.get(backend)
            if not values:
                continue
            medians.append([
                entry["capability"], entry["dataset_class"], backend,
                f"{median_of(values):.3f}", f"{median_of(values[1:]):.3f}",
                f"{median_of(values[1:-1]):.3f}",
                f"{cv_percent(values):.2f}", f"{cv_percent(values[1:]):.2f}",
                f"{cv_percent(values[1:-1]):.2f}",
            ])
            if backend == "rust":
                continue
            row = [entry["capability"], backend]
            labels = []
            for label in ("all_n", "drop_first", "middle"):
                reference = baseline_variants[label]
                sample = {"all_n": values, "drop_first": values[1:], "middle": values[1:-1]}[label]
                ratio = median_of(sample) / median_of(reference)
                low, high = bootstrap_ratio(reference, sample)
                noise = 2 * (cv_percent(reference) + cv_percent(sample)) / 100
                verdict = classify_ratio(low, high)
                row += [f"{ratio:.4f}", f"{low:.4f}", f"{high:.4f}", f"{noise:.4f}", verdict]
                labels.append(label)
            verdicts.append(row)
    return medians, verdicts


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--summary", required=True)
    parser.add_argument("--reports", required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--comparison", help="earlier summary.json for the comparison table")
    arguments = parser.parse_args()

    summary = load(Path(arguments.summary))
    reports = Path(arguments.reports)
    output = Path(arguments.output_dir)
    output.mkdir(parents=True, exist_ok=True)

    write_tsv(
        output / "statistics.tsv",
        ["capability", "dataset_class", "backend", "n", "median_wall_ms", "iqr_wall_ms",
         "cv_percent", "min_wall_ms", "max_wall_ms", "median_peak_rss_mb",
         "ratio_vs_rust", "ratio_ci_low", "ratio_ci_high", "noise_band", "verdict"],
        statistics_rows(summary, reports),
    )
    write_tsv(
        output / "statistics-consistency.tsv",
        ["capability", "verdict", "tolerance", "precision", "backends_with_ok_runs",
         "report", "report_path", "differences"],
        consistency_rows(summary, reports, output),
    )
    if arguments.comparison:
        earlier = load(Path(arguments.comparison))
        write_tsv(
            output / "statistics-comparison.tsv",
            ["capability", "metric", "earlier_report", "earlier_value", "current_value",
             "current_over_earlier"],
            comparison_rows(summary, reports, earlier, None),
        )
    trimmed_medians, trimmed_verdicts = trimmed_rows(summary, reports)
    write_tsv(
        output / "statistics-trimmed-medians.tsv",
        ["capability", "dataset_class", "backend",
         "median_all_n", "median_drop_first", "median_middle",
         "cv_all", "cv_drop_first", "cv_middle"],
        trimmed_medians,
    )
    write_tsv(
        output / "statistics-trimmed-verdicts.tsv",
        ["capability", "backend",
         "ratio_all_n", "ci_low_all_n", "ci_high_all_n", "noise_all_n", "verdict_all_n",
         "ratio_drop_first", "ci_low_drop_first", "ci_high_drop_first", "noise_drop_first",
         "verdict_drop_first",
         "ratio_middle", "ci_low_middle", "ci_high_middle", "noise_middle", "verdict_middle"],
        trimmed_verdicts,
    )
    print("wrote statistics.tsv, statistics-consistency.tsv, "
          "statistics-trimmed-medians.tsv, statistics-trimmed-verdicts.tsv"
          + (", statistics-comparison.tsv" if arguments.comparison else ""))


if __name__ == "__main__":
    main()
