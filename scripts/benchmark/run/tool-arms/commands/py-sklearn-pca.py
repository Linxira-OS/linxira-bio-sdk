#!/usr/bin/env python3
"""Ecosystem-tool arm for `expression.pca.v1`: scikit-learn's PCA, as a user would call it.

The parity arm in this repository hand-writes the power iteration (turn `features x samples`
into the sample Gram matrix and iterate), because a parity arm exists to isolate the language
runtime. This script answers the other question - "what does a user get if they just import
sklearn?" - so it reads the same file with pandas-free stdlib parsing, transposes to
`samples x features`, and calls `sklearn.decomposition.PCA`.

The invocation is recorded verbatim in the report, so a difference against the parity arm is
attributable to the call rather than guessed.
"""

from __future__ import annotations

import sys


def read_matrix(path: str) -> tuple[list[str], list[str], list[list[float]]]:
    """Rows are features (first column is the identifier), columns are samples."""
    delimiter = "\t" if path.endswith((".tsv", ".txt", ".gz")) else ","
    opener = open
    if path.endswith(".gz"):
        import gzip

        opener = gzip.open  # type: ignore[assignment]
    with opener(path, "rt", encoding="utf-8") as handle:
        lines = [line.rstrip("\n") for line in handle if line.strip()]
    if not lines:
        raise SystemExit("expression matrix is empty")
    if path.endswith((".csv", ".csv.gz")):
        delimiter = ","
    header = lines[0].split(delimiter)
    samples = header[1:]
    features: list[str] = []
    values: list[list[float]] = []
    for row in lines[1:]:
        cells = row.split(delimiter)
        if len(cells) != len(header):
            raise SystemExit(f"row {row[:32]!r} has {len(cells)} fields, expected {len(header)}")
        features.append(cells[0])
        try:
            values.append([float(cell) for cell in cells[1:]])
        except ValueError as error:
            raise SystemExit(f"non-numeric value in row {cells[0]!r}: {error}") from error
    if len(samples) < 2:
        raise SystemExit("PCA requires at least two samples")
    return samples, features, values


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        raise SystemExit("usage: py-sklearn-pca.py <matrix.tsv|matrix.csv[.gz]>")
    samples, features, values = read_matrix(argv[0])

    from sklearn.decomposition import PCA  # imported late so --help stays fast

    sample_by_feature = [[values[feature][sample] for feature in range(len(features))]
                         for sample in range(len(samples))]
    components = min(len(samples) - 1, len(features))
    model = PCA(n_components=components, svd_solver="full")
    model.fit(sample_by_feature)
    print(
        f"sklearn PCA samples={len(samples)} features={len(features)} "
        f"components={components} explained_variance={model.explained_variance_.sum():.6f}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
