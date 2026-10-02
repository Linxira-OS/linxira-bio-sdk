#!/usr/bin/env python3
"""Strict, local-only ComBat batch-effect correction (parametric empirical Bayes).

Reference: Johnson, Li & Rabinovic, "Adjusting batch effects in microarray
expression data using empirical Bayes methods", Biostatistics 8(1), 2007,
doi:10.1093/biostatistics/kxj037. This pack implements the parametric
posterior-mode variant with an alternating fixed-point iteration; the R pack
org.linxira.batch-combat-r implements the identical specification and the two
backends are held to byte-identical TSV output on the same inputs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np

PACK_ID = "org.linxira.batch-combat-py"
PACK_VERSION = "0.1.0"
CAPABILITY = "expression.batch-correct.v1"
METHOD = "combat-parametric"
MISSING_TOKENS = {"", "na", "nan", "null", "none"}
MAX_ITERATIONS = 5000
CONVERGENCE_TOLERANCE = 1e-8
FLOAT_FORMAT = "%.10g"


class RequestError(ValueError):
    """A stable, user-correctable request validation failure."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def core_version() -> str:
    return os.environ.get("LINXIRA_BIO_CORE_VERSION", "unknown")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_request(request_path: Path) -> dict[str, Any]:
    try:
        with request_path.open("r", encoding="utf-8") as handle:
            request = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise RequestError(f"cannot read request: {error}") from error
    if not isinstance(request, dict):
        raise RequestError("request must be an object")
    return request


def artifact_files(request: dict[str, Any], role: str) -> list[Path]:
    inputs = request.get("inputs")
    if not isinstance(inputs, list):
        raise RequestError("request inputs must be a list")
    matches = [
        artifact
        for artifact in inputs
        if isinstance(artifact, dict) and artifact.get("role") == role
    ]
    if len(matches) != 1:
        raise RequestError(f"request must contain exactly one {role} input artifact")
    files = matches[0].get("files")
    if not isinstance(files, list) or len(files) != 1:
        raise RequestError(f"{role} artifact must contain exactly one file")
    path = files[0].get("path")
    if not isinstance(path, str) or not path:
        raise RequestError(f"{role} file path is missing")
    return [Path(path)]


def delimiter_for(path: Path, label: str) -> str:
    suffix = path.suffix.lower()
    if suffix in (".tsv", ".txt"):
        return "\t"
    if suffix == ".csv":
        return ","
    raise RequestError(f"{label} must be .csv or .tsv: {path.name}")


def read_table(path: Path, label: str) -> list[list[str]]:
    delimiter = delimiter_for(path, label)
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        raise RequestError(f"cannot read {label}: {error}") from error
    rows = [line.split(delimiter) for line in text.splitlines() if line.strip()]
    if not rows:
        raise RequestError(f"{label} is empty: {path.name}")
    width = len(rows[0])
    if width < 2:
        raise RequestError(f"{label} must have a header and at least one data column")
    for index, row in enumerate(rows):
        if len(row) != width:
            raise RequestError(
                f"{label} row {index + 1} has {len(row)} fields, expected {width}"
            )
    return rows


def check_identifier(value: str, label: str) -> str:
    if any(token in value for token in ("\t", "\n", "\r")):
        raise RequestError(f"{label} contains a delimiter character: {value!r}")
    return value


def parse_float(value: str, label: str) -> float:
    token = value.strip()
    if token.lower() in MISSING_TOKENS:
        raise RequestError(f"missing value in {label}: {value!r}")
    try:
        number = float(token)
    except ValueError as error:
        raise RequestError(f"{label} is not numeric: {value!r}") from error
    if number != number or number in (float("inf"), float("-inf")):
        raise RequestError(f"{label} is not finite: {value!r}")
    return number


def read_matrix(
    path: Path,
) -> tuple[list[str], list[str], np.ndarray]:
    rows = read_table(path, "expression matrix")
    header = [check_identifier(cell.strip(), "sample id") for cell in rows[0]]
    samples = header[1:]
    if len(set(samples)) != len(samples):
        raise RequestError("expression matrix has duplicate sample ids")
    features: list[str] = []
    values = np.empty((len(rows) - 1, len(samples)), dtype=np.float64)
    for row_index, row in enumerate(rows[1:]):
        feature = check_identifier(row[0].strip(), "feature id")
        features.append(feature)
        for column_index, cell in enumerate(row[1:]):
            values[row_index, column_index] = parse_float(
                cell, f"expression value for {feature}/{samples[column_index]}"
            )
    if not features:
        raise RequestError("expression matrix contains no features")
    return features, samples, values


def read_sample_table(
    path: Path,
    batch_column: str,
) -> tuple[list[str], list[str], list[str], list[np.ndarray], list[list[str]]]:
    rows = read_table(path, "sample table")
    header = [cell.strip() for cell in rows[0]]
    if batch_column not in header:
        raise RequestError(f"sample table has no column named {batch_column!r}")
    sample_index = 0
    batch_index = header.index(batch_column)
    if batch_index == sample_index:
        raise RequestError("batch column must not be the sample-id column")
    covariate_indices = [
        index for index in range(1, len(header)) if index not in (sample_index, batch_index)
    ]
    sample_ids: list[str] = []
    batches: list[str] = []
    numeric_columns: list[np.ndarray] = []
    raw_columns: list[list[str]] = []
    for row in rows[1:]:
        sample_ids.append(check_identifier(row[sample_index].strip(), "sample id"))
        batch = row[batch_index].strip()
        if batch.lower() in MISSING_TOKENS:
            raise RequestError(f"missing batch label for sample {sample_ids[-1]!r}")
        batches.append(batch)
        for position, index in enumerate(covariate_indices):
            raw = row[index].strip()
            if position >= len(numeric_columns):
                numeric_columns.append(np.empty(len(rows) - 1, dtype=np.float64))
                raw_columns.append([])
            raw_columns[position].append(raw)
    if len(set(sample_ids)) != len(sample_ids):
        raise RequestError("sample table has duplicate sample ids")
    for position, raw in enumerate(raw_columns):
        header_name = header[covariate_indices[position]]
        numeric = True
        for value in raw:
            if value.lower() in MISSING_TOKENS:
                raise RequestError(
                    f"missing value in covariate {header_name!r} for sample {sample_ids[raw.index(value)]!r}"
                )
            try:
                float(value)
            except ValueError:
                numeric = False
                break
        if numeric:
            for row_index, value in enumerate(raw):
                numeric_columns[position][row_index] = parse_float(
                    value, f"covariate {header_name!r}"
                )
        else:
            levels = sorted(set(raw))
            if len(levels) != 2:
                raise RequestError(
                    f"covariate {header_name!r} must be numeric or have exactly two levels "
                    f"(found {len(levels)})"
                )
            for row_index, value in enumerate(raw):
                numeric_columns[position][row_index] = 0.0 if value == levels[0] else 1.0
    return sample_ids, batches, [header[i] for i in covariate_indices], numeric_columns, raw_columns


def combat_adjust(
    values: np.ndarray,
    batches: list[str],
    covariates: np.ndarray,
) -> tuple[np.ndarray, int, dict[str, float]]:
    """Parametric ComBat; values are features x samples, covariates n x m."""
    sample_count = values.shape[1]
    labels = sorted(set(batches))
    if len(labels) < 2:
        raise RequestError("batch correction requires at least two batches")
    counts = {label: batches.count(label) for label in labels}
    for label, count in counts.items():
        if count < 2:
            raise RequestError(
                f"batch {label!r} has {count} sample(s); at least two are required"
            )
    design = np.zeros((sample_count, len(labels) + covariates.shape[1]), dtype=np.float64)
    for column, label in enumerate(labels):
        design[:, column] = [1.0 if batch == label else 0.0 for batch in batches]
    design[:, len(labels):] = covariates
    if np.linalg.matrix_rank(design) < design.shape[1]:
        raise RequestError("batch design matrix is rank deficient (covariates collinear with batch)")
    coefficients = np.linalg.solve(design.T @ design, design.T @ values.T)
    fitted = (design @ coefficients).T
    residuals = values - fitted
    grand_mean = counts[labels[0]] / sample_count * coefficients[0, :]
    for column, label in enumerate(labels[1:], start=1):
        grand_mean = grand_mean + counts[label] / sample_count * coefficients[column, :]
    covariate_part = (covariates @ coefficients[len(labels):, :]).T
    pooled_variance = np.mean(residuals**2, axis=1)
    sigma = np.sqrt(pooled_variance)
    if np.any(sigma <= 0.0):
        offenders = ", ".join(str(index) for index in np.flatnonzero(sigma <= 0.0)[:5])
        raise RequestError(f"features with zero residual variance cannot be corrected: rows {offenders}")
    standardized = (values - grand_mean[:, None] - covariate_part) / sigma[:, None]

    gamma_hat = np.empty((len(labels), values.shape[0]), dtype=np.float64)
    delta_hat_squared = np.empty_like(gamma_hat)
    for column, label in enumerate(labels):
        indices = [index for index, batch in enumerate(batches) if batch == label]
        gamma_hat[column, :] = np.mean(standardized[:, indices], axis=1)
        delta_hat_squared[column, :] = np.var(standardized[:, indices], axis=1, ddof=1)

    gamma_bar = np.mean(gamma_hat, axis=1)
    tau_squared = np.var(gamma_hat, axis=1, ddof=1)
    delta_bar = np.mean(delta_hat_squared, axis=1)
    delta_variance = np.var(delta_hat_squared, axis=1, ddof=1)
    a_prior = (2.0 * delta_variance + delta_bar**2) / delta_variance
    b_prior = (delta_bar * delta_variance + delta_bar**3) / delta_variance

    iterations_used = 0
    gamma_star = np.empty_like(gamma_hat)
    delta_star = np.empty_like(delta_hat_squared)
    for column, label in enumerate(labels):
        indices = [index for index, batch in enumerate(batches) if batch == label]
        count = float(counts[label])
        for feature in range(values.shape[0]):
            observations = standardized[feature, indices]
            batch_mean = gamma_hat[column, feature]
            batch_variance = delta_hat_squared[column, feature]
            gamma = batch_mean
            delta_squared = batch_variance
            for iteration in range(MAX_ITERATIONS):
                denominator = count * tau_squared[column] + delta_squared
                if denominator <= 0.0:
                    raise RequestError(f"degenerate empirical-Bayes prior for batch {label!r}")
                gamma_new = (
                    count * tau_squared[column] * batch_mean + delta_squared * gamma_bar[column]
                ) / denominator
                sum_squares = float(np.sum((observations - gamma_new) ** 2))
                delta_new = (0.5 * sum_squares + b_prior[column]) / (
                    count / 2.0 + a_prior[column] - 1.0
                )
                gamma_change = abs(gamma_new - gamma) / (1.0 + abs(gamma))
                delta_change = abs(delta_new - delta_squared) / (1.0 + abs(delta_squared))
                gamma = gamma_new
                delta_squared = delta_new
                if gamma_change < CONVERGENCE_TOLERANCE and delta_change < CONVERGENCE_TOLERANCE:
                    iterations_used = max(iterations_used, iteration + 1)
                    break
            else:
                raise RequestError(
                    f"empirical-Bayes iteration did not converge for batch {label!r} "
                    f"within {MAX_ITERATIONS} iterations"
                )
            gamma_star[column, feature] = gamma
            delta_star[column, feature] = delta_squared

    adjusted = np.empty_like(values)
    for column, label in enumerate(labels):
        indices = [index for index, batch in enumerate(batches) if batch == label]
        gamma_row = gamma_star[column][:, None]
        delta_row = np.sqrt(delta_star[column])[:, None]
        adjusted[:, indices] = (
            sigma[:, None] * (standardized[:, indices] - gamma_row) / delta_row
            + grand_mean[:, None]
            + covariate_part[:, indices]
        )
    priors = {
        "gamma_bar": {label: float(gamma_bar[index]) for index, label in enumerate(labels)},
        "tau_squared": {label: float(tau_squared[index]) for index, label in enumerate(labels)},
        "a_prior": {label: float(a_prior[index]) for index, label in enumerate(labels)},
        "b_prior": {label: float(b_prior[index]) for index, label in enumerate(labels)},
    }
    return adjusted, iterations_used, priors


def write_corrected_matrix(
    path: Path,
    features: list[str],
    samples: list[str],
    adjusted: np.ndarray,
) -> None:
    lines = ["\t".join(["feature"] + samples)]
    for row_index, feature in enumerate(features):
        cells = [feature]
        cells.extend(FLOAT_FORMAT % value for value in adjusted[row_index, :])
        lines.append("\t".join(cells))
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def success_result(
    job_id: str,
    output_path: Path,
    started_at: str,
    input_sha256: dict[str, str],
    feature_count: int,
    sample_count: int,
    batch_counts: dict[str, int],
    covariate_columns: list[str],
    iterations_used: int,
    priors: dict[str, dict[str, float]],
) -> dict[str, Any]:
    lock_path = Path(__file__).resolve().parents[1] / "requirements.lock"
    return {
        "schema_version": "2",
        "job_id": job_id,
        "capability": CAPABILITY,
        "status": "ok",
        "result": {
            "method": METHOD,
            "feature_count": feature_count,
            "sample_count": sample_count,
            "batch_count": len(batch_counts),
            "batch_counts": batch_counts,
            "covariate_columns": covariate_columns,
            "eb_iterations_max": iterations_used,
            "eb_priors": priors,
        },
        "artifacts": [
            {
                "artifact_id": "corrected-matrix",
                "role": "corrected-expression",
                "kind": "domain-file",
                "path": str(output_path),
                "format": "tsv",
                "media_type": "text/tab-separated-values",
                "size_bytes": output_path.stat().st_size,
                "sha256": sha256_file(output_path),
            }
        ],
        "provenance": {
            "engine_version": PACK_VERSION,
            "execution_mode": "local-cpu",
            "core_version": core_version(),
            "started_at": started_at,
            "finished_at": utc_now(),
            "software": [
                {"name": "CPython", "version": sys.version.split()[0]},
                {"name": "NumPy", "version": np.__version__, "package_id": "numpy"},
            ],
            "input_sha256": input_sha256,
            "command": [
                "python",
                "src/run_combat.py",
                "--request",
                "<request>",
                "--result",
                "<result>",
            ],
            "dependency_lock_sha256": sha256_file(lock_path),
        },
        "diagnostics": [],
    }


def error_result(job_id: str, message: str, started_at: str) -> dict[str, Any]:
    return {
        "schema_version": "2",
        "job_id": job_id,
        "capability": CAPABILITY,
        "status": "error",
        "result": {},
        "artifacts": [],
        "provenance": {
            "engine_version": PACK_VERSION,
            "execution_mode": "local-cpu",
            "core_version": core_version(),
            "started_at": started_at,
            "finished_at": utc_now(),
        },
        "diagnostics": [{"code": "workflow_failed", "severity": "error", "message": message}],
    }


def parse_arguments(arguments: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="ComBat batch-effect correction")
    parser.add_argument("--request", required=True, type=Path, help="artifact-aware request JSON")
    parser.add_argument("--result", required=True, type=Path, help="machine-readable result JSON")
    return parser.parse_args(arguments)


def output_path_from(request: dict[str, Any]) -> Path:
    parameters = request.get("parameters")
    if not isinstance(parameters, dict):
        raise RequestError("request parameters must be an object")
    output_directory = parameters.get("output_directory")
    if not isinstance(output_directory, str) or not output_directory:
        raise RequestError("parameters.output_directory is required")
    output_filename = parameters.get("output_filename")
    if not isinstance(output_filename, str) or not output_filename:
        output_filename = "corrected.tsv"
    return Path(output_directory) / output_filename


def batch_column_from(request: dict[str, Any]) -> str:
    parameters = request.get("parameters")
    if not isinstance(parameters, dict):
        raise RequestError("request parameters must be an object")
    batch_column = parameters.get("batch_column", "batch")
    if not isinstance(batch_column, str) or not batch_column.strip():
        raise RequestError("parameters.batch_column must be a non-empty string")
    return batch_column.strip()


def method_from(request: dict[str, Any]) -> None:
    parameters = request.get("parameters")
    method = parameters.get("method", "combat") if isinstance(parameters, dict) else "combat"
    if method != "combat":
        raise RequestError(f"unsupported batch-correction method {method!r}; expected 'combat'")


def main(arguments: list[str] | None = None) -> int:
    options = parse_arguments(arguments)
    started_at = utc_now()
    request: dict[str, Any] = {}
    try:
        request = load_request(options.request)
        job_id = request.get("job_id")
        if not isinstance(job_id, str) or not job_id:
            raise RequestError("job_id is required")
        method_from(request)
        matrix_path, sample_table_path = (
            artifact_files(request, "expression-matrix")[0],
            artifact_files(request, "sample-metadata")[0],
        )
        for role, path in (
            ("expression-matrix", matrix_path),
            ("sample-metadata", sample_table_path),
        ):
            if not path.is_file():
                raise RequestError(f"{role} file does not exist: {path}")
        batch_column = batch_column_from(request)
        features, samples, values = read_matrix(matrix_path)
        sample_ids, batches, covariate_names, covariate_values, _ = read_sample_table(
            sample_table_path, batch_column
        )
        if sorted(sample_ids) != sorted(samples):
            raise RequestError(
                "sample table ids do not match the expression matrix columns exactly"
            )
        order = [sample_ids.index(sample) for sample in samples]
        batches = [batches[index] for index in order]
        covariates = np.column_stack([column[order] for column in covariate_values]) if covariate_names else np.empty((len(samples), 0))
        output_path = output_path_from(request)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        adjusted, iterations_used, priors = combat_adjust(values, batches, covariates)
        write_corrected_matrix(output_path, features, samples, adjusted)
        counts: dict[str, int] = {}
        for batch in batches:
            counts[batch] = counts.get(batch, 0) + 1
        payload = success_result(
            job_id,
            output_path,
            started_at,
            {
                "expression-matrix": sha256_file(matrix_path),
                "sample-metadata": sha256_file(sample_table_path),
            },
            len(features),
            len(samples),
            counts,
            covariate_names,
            iterations_used,
            priors,
        )
    except RequestError as error:
        job_id = str(request.get("job_id")) if "request" in locals() else "unknown"
        payload = error_result(job_id, str(error), started_at)
        options.result.parent.mkdir(parents=True, exist_ok=True)
        options.result.write_text(json.dumps(payload), encoding="utf-8")
        print(json.dumps(payload))
        return 2
    options.result.parent.mkdir(parents=True, exist_ok=True)
    options.result.write_text(json.dumps(payload), encoding="utf-8")
    print(json.dumps(payload))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
