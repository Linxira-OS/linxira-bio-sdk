"""expression.pca.v1 — independent NumPy implementation.

A line-for-line port of the Rust engine's PCA
(engine/crates/linxira-bio-core/src/expression.rs) so the two backends are
comparable field by field at the 1e-6 relative tolerance:

* the matrix reader follows the same rules (delimiter probed from the header,
  all fields trimmed, non-empty unique feature ids and sample names, missing
  values ``""``/``.``/``na``/``nan`` rejected, strictly finite floats)
* centering per feature, optional per-feature scaling by the sample standard
  deviation, constant features zeroed and counted
* the components come from the same deterministic power iteration on the
  sample Gram matrix (deterministic sinusoidal start vector, Gram-Schmidt
  against earlier components, the same convergence thresholds), so the
  eigenvector *signs* agree with the engine: the element with the largest
  absolute value is made positive
* loadings are sorted the same way (loading descending, feature ascending on
  ties) and the top-10 positive/negative lists carry that exact order

scikit-learn is deliberately not used here: its SVD sign convention differs,
and the point of this backend is a field-for-field comparison against the
engine. ``numpy.linalg`` is used only for elementwise math and ``math`` for
the sinusoidal start vector, both identical to the Rust port.
"""

from __future__ import annotations

import csv
import gzip
import math
from pathlib import Path
from typing import Any

import numpy as np

CAPABILITY = "expression.pca.v1"
INPUT_ROLES = ("matrix",)
PARAMETERS: tuple[str, ...] = ("components", "scale_features")
GZIP_MAGIC = b"\x1f\x8b"
MAX_COMPONENTS = 64
F64_EPSILON = 2.220446049250313e-16

_WARMUP_LINES = 8


class ImplementationError(ValueError):
    """An input the native engine would also reject, reported the same way."""


def software() -> list[dict[str, str]]:
    return [{"name": "NumPy", "version": np.__version__, "package_id": "numpy"}]


def _open_text(path: Path):
    with path.open("rb") as probe:
        magic = probe.read(2)
    if magic == GZIP_MAGIC:
        return gzip.open(path, "rt", encoding="utf-8", newline="")
    return path.open("r", encoding="utf-8", newline="")


def _delimiter(header_line: str) -> str:
    tabs = header_line.count("\t")
    commas = header_line.count(",")
    return "\t" if tabs > commas else ","


def _is_missing(value: str) -> bool:
    return (
        value == ""
        or value == "."
        or value.lower() in ("na", "nan")
    )


def _read_matrix(path: Path) -> tuple[list[str], list[str], list[list[float]]]:
    with _open_text(path) as handle:
        first_line = ""
        for line in handle:
            if line.strip():
                first_line = line
                break
        delimiter = _delimiter(first_line)
        handle.seek(0)
        rows_iter = csv.reader(handle, delimiter=delimiter)
        header = next(rows_iter, None)
        if header is None:
            raise ImplementationError("expression matrix is empty")
        headers = [cell.strip() for cell in header]
        if len(headers) < 2 or not headers[0]:
            raise ImplementationError(
                "expected a named feature identifier column and at least one sample"
            )
        sample_names = headers[1:]
        if any(not name for name in sample_names) or len(set(sample_names)) != len(sample_names):
            raise ImplementationError("sample names must be non-empty and unique")
        feature_ids: list[str] = []
        values: list[list[float]] = []
        seen_features: set[str] = set()
        for record_index, record in enumerate(rows_iter):
            row_number = record_index + 1
            if len(record) != len(headers):
                raise ImplementationError(
                    f"record {row_number} has {len(record)} fields, expected {len(headers)}"
                )
            feature_id = record[0].strip()
            if not feature_id or feature_id in seen_features:
                raise ImplementationError(
                    f"feature identifier {feature_id!r} is empty or duplicated"
                )
            seen_features.add(feature_id)
            row: list[float] = []
            for sample_index, cell in enumerate(record[1:]):
                value = cell.strip()
                if _is_missing(value):
                    raise ImplementationError(
                        f"sample {sample_names[sample_index]!r} contains a missing value; "
                        "impute or filter it before this analysis"
                    )
                try:
                    parsed = float(value)
                except ValueError:
                    raise ImplementationError(
                        f"sample {sample_names[sample_index]!r} contains non-numeric "
                        f"value {value!r}"
                    ) from None
                if not math.isfinite(parsed):
                    raise ImplementationError(
                        f"sample {sample_names[sample_index]!r} contains a non-finite value"
                    )
                row.append(parsed)
            feature_ids.append(feature_id)
            values.append(row)
    if not values:
        raise ImplementationError("expression matrix contains no feature rows")
    return feature_ids, sample_names, values


def _centered_rows(
    values: list[list[float]], scale: bool
) -> tuple[list[list[float]], int]:
    constant_count = 0
    rows: list[list[float]] = []
    for row in values:
        mean = sum(row) / len(row)
        centered = [value - mean for value in row]
        sum_squares = sum(value * value for value in centered)
        if sum_squares <= F64_EPSILON:
            constant_count += 1
            centered = [0.0] * len(centered)
        elif scale:
            denominator = float(max(len(row) - 1, 1))
            standard_deviation = math.sqrt(sum_squares / denominator)
            centered = [value / standard_deviation for value in centered]
        rows.append(centered)
    return rows, constant_count


def _dot(left: list[float], right: list[float]) -> float:
    return sum(a * b for a, b in zip(left, right))


def _orthogonalize(vector: list[float], basis: list[list[float]]) -> None:
    for direction in basis:
        projection = _dot(vector, direction)
        for index, direction_value in enumerate(direction):
            vector[index] -= projection * direction_value


def _normalize(vector: list[float]) -> float:
    norm = math.sqrt(_dot(vector, vector))
    if norm > F64_EPSILON:
        for index in range(len(vector)):
            vector[index] /= norm
    return norm


def _gram(rows: list[list[float]], vector: list[float], denominator: float) -> list[float]:
    result = [0.0] * len(vector)
    for row in rows:
        projection = _dot(row, vector) / denominator
        for index, row_value in enumerate(row):
            result[index] += row_value * projection
    return result


def _leading_eigenpairs(
    rows: list[list[float]], component_count: int, denominator: float
) -> list[tuple[float, list[float]]]:
    sample_count = len(rows[0])
    eigenvectors: list[list[float]] = []
    eigenpairs: list[tuple[float, list[float]]] = []
    for component in range(component_count):
        vector = [
            math.sin((index + 1) * (component + 2))
            + math.cos(((index + 1) * (component + 2)) * 0.37)
            for index in range(sample_count)
        ]
        _orthogonalize(vector, eigenvectors)
        if _normalize(vector) <= F64_EPSILON:
            found = False
            for basis in range(sample_count):
                vector = [0.0] * sample_count
                vector[basis] = 1.0
                _orthogonalize(vector, eigenvectors)
                if _normalize(vector) > F64_EPSILON:
                    found = True
                    break
            if not found:
                break
        for _ in range(500):
            nxt = _gram(rows, vector, denominator)
            _orthogonalize(nxt, eigenvectors)
            if _normalize(nxt) <= 1e-14:
                break
            alignment = abs(_dot(vector, nxt))
            vector = nxt
            if 1.0 - alignment < 1e-11:
                break
        projected = _gram(rows, vector, denominator)
        eigenvalue = _dot(vector, projected)
        if not math.isfinite(eigenvalue) or eigenvalue <= 1e-12:
            break
        # The engine makes the largest-magnitude element positive. Rust's
        # `max_by` returns the LAST maximum on ties, so scan with >= to keep
        # the tie rule identical across backends.
        largest_index = 0
        for index in range(1, len(vector)):
            if abs(vector[index]) >= abs(vector[largest_index]):
                largest_index = index
        if vector[largest_index] < 0.0:
            vector = [-value for value in vector]
        eigenvectors.append(list(vector))
        eigenpairs.append((eigenvalue, vector))
    return eigenpairs


def run(inputs: dict[str, Path], parameters: dict[str, Any]) -> dict[str, Any]:
    components_requested = int(parameters.get("components", 2))
    if components_requested <= 0:
        raise ImplementationError("components must be at least 1")
    scale_features = bool(parameters.get("scale_features", False))
    feature_ids, sample_names, values = _read_matrix(inputs["matrix"])
    if len(sample_names) < 2:
        raise ImplementationError("PCA requires at least two samples")

    rows, constant_features = _centered_rows(values, scale_features)
    denominator = float(len(sample_names) - 1)
    total_variance = (
        sum(value * value for row in rows for value in row) / denominator
    )
    if total_variance <= F64_EPSILON:
        raise ImplementationError(
            "PCA requires at least one feature with non-zero variance"
        )

    component_limit = min(
        components_requested,
        len(sample_names) - 1,
        len(feature_ids),
    )
    eigenpairs = _leading_eigenpairs(rows, component_limit, denominator)
    if not eigenpairs:
        raise ImplementationError("PCA could not resolve a non-zero component")

    samples = {sample: [] for sample in sample_names}
    components: list[dict[str, Any]] = []
    for component_index, (eigenvalue, vector) in enumerate(eigenpairs):
        singular_value = math.sqrt(eigenvalue * denominator)
        for sample, coordinate in zip(sample_names, vector):
            samples[sample].append(coordinate * singular_value)
        loadings = [
            {"feature": feature, "loading": _dot(row, vector) / singular_value}
            for feature, row in zip(feature_ids, rows)
        ]
        loadings.sort(key=lambda item: (-item["loading"], item["feature"]))
        top_positive = [item for item in loadings if item["loading"] > 0.0][:10]
        top_negative = [item for item in reversed(loadings) if item["loading"] < 0.0][:10]
        components.append(
            {
                "component": component_index + 1,
                "eigenvalue": eigenvalue,
                "explained_variance_percent": min(
                    max(eigenvalue / total_variance * 100.0, 0.0), 100.0
                ),
                "top_positive_loadings": top_positive,
                "top_negative_loadings": top_negative,
            }
        )

    warnings: list[str] = []
    if constant_features != 0:
        warnings.append(
            f"{constant_features} constant features contributed no PCA variance"
        )
    if component_limit < components_requested:
        warnings.append(
            f"requested {components_requested} components but matrix rank permits "
            f"at most {component_limit}"
        )

    return {
        "feature_count": len(feature_ids),
        "sample_count": len(sample_names),
        "scaled_features": scale_features,
        "total_variance": total_variance,
        "components": components,
        "samples": [
            {"sample": sample, "scores": scores} for sample, scores in samples.items()
        ],
        "warnings": warnings,
    }
