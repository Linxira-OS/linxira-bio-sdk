"""set.venn.v1 — independent implementation (exact set arithmetic).

Counterpart of the Rust engine's Venn analysis
(engine/crates/linxira-bio-core/src/set_analysis.rs). The membership table
is parsed with the engine's rules — header row names the sets (2..64 unique,
non-empty), each further row contributes one item per non-empty cell, items
are trimmed, short rows are accepted, and a mask bit per column records the
membership; pure set arithmetic follows:

* ``set_sizes``  per-column membership counts
* ``union_size`` number of distinct items
* ``intersections`` one entry per occupancy mask, ordered by the mask bits
  (lexicographic by the bitset, like the engine's ``BTreeMap``), each with
  ``sets`` (mask order), ``degree`` (popcount), ``count``, and ``items``
  (only with ``include_items``, sorted by item because the engine iterates
  its ordered map)

No third-party library is involved: matplotlib-venn is a *plotting* concern
and the analysis itself is exact set arithmetic.
"""

from __future__ import annotations

import csv
import gzip
from pathlib import Path
from typing import Any

CAPABILITY = "set.venn.v1"
INPUT_ROLES = ("table",)
PARAMETERS: tuple[str, ...] = ("include_items", "max_intersections")
GZIP_MAGIC = b"\x1f\x8b"
MAX_SET_COLUMNS = 64
MAX_VENN_COLUMNS = 6
MAX_SET_ROWS = 1_000_000
MAX_UNIQUE_SET_ITEMS = 1_000_000


class ImplementationError(ValueError):
    """An input the native engine would also reject, reported the same way."""


def software() -> list[dict[str, str]]:
    return [{"name": "CPython", "version": _python_version()}]


def _python_version() -> str:
    import sys

    return sys.version.split()[0]


def _open_text(path: Path):
    with path.open("rb") as probe:
        magic = probe.read(2)
    if magic == GZIP_MAGIC:
        return gzip.open(path, "rt", encoding="utf-8", newline="")
    return path.open("r", encoding="utf-8", newline="")


def _delimiter(path: Path) -> str:
    name = path.name.lower()
    if name.endswith(".gz"):
        name = name[:-3]
    if name.endswith(".csv"):
        return ","
    if name.endswith((".tsv", ".tab")):
        return "\t"
    with _open_text(path) as handle:
        header = handle.readline()
    return "\t" if header.count("\t") >= header.count(",") else ","


def _read_membership_table(path: Path) -> tuple[list[str], dict[str, int]]:
    delimiter = _delimiter(path)
    names: list[str] = []
    memberships: dict[str, int] = {}
    with _open_text(path) as handle:
        rows = csv.reader(handle, delimiter=delimiter)
        header = next(rows, None)
        if header is None:
            raise ImplementationError("set table is empty")
        for cell in header:
            name = cell.strip()
            if not name:
                raise ImplementationError("set names must not be empty")
            if name in names:
                raise ImplementationError(f"duplicate set name {name!r}")
            names.append(name)
        if not 2 <= len(names) <= MAX_SET_COLUMNS:
            raise ImplementationError(
                f"expected 2 to {MAX_SET_COLUMNS} named columns; found {len(names)}"
            )
        for row_number, record in enumerate(rows, start=2):
            if row_number - 1 > MAX_SET_ROWS:
                raise ImplementationError("row count exceeds the local analysis limit")
            if len(record) > len(names):
                raise ImplementationError(
                    f"data row {row_number} has {len(record)} fields but the "
                    f"header has {len(names)}"
                )
            for column, value in enumerate(record):
                item = value.strip()
                if not item:
                    continue
                if item not in memberships and len(memberships) >= MAX_UNIQUE_SET_ITEMS:
                    raise ImplementationError(
                        "unique item count exceeds the local analysis limit"
                    )
                memberships[item] = memberships.get(item, 0) | (1 << column)
    if not memberships:
        raise ImplementationError("no non-empty set items were found")
    return names, memberships


def _summarize(
    names: list[str], memberships: dict[str, int], include_items: bool
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    sizes = [0] * len(names)
    groups: dict[int, tuple[int, list[str]]] = {}
    for item, mask in memberships.items():
        for column in range(len(names)):
            if mask & (1 << column):
                sizes[column] += 1
        entry = groups.setdefault(mask, [0, []])
        entry[0] += 1
        if include_items:
            entry[1].append(item)
    set_sizes = [
        {"name": name, "count": count} for name, count in zip(names, sizes)
    ]
    intersections = []
    for mask in sorted(groups):
        count, items = groups[mask]
        intersections.append(
            {
                "sets": [
                    name
                    for column, name in enumerate(names)
                    if mask & (1 << column)
                ],
                "degree": bin(mask).count("1"),
                "count": count,
                "items": sorted(items) if include_items else [],
            }
        )
    return set_sizes, intersections


def run(inputs: dict[str, Path], parameters: dict[str, Any]) -> dict[str, Any]:
    include_items = bool(parameters.get("include_items", False))
    max_intersections = int(parameters.get("max_intersections", 50))
    if not 1 <= max_intersections <= 10_000:
        raise ImplementationError(
            "max_intersections must be between 1 and 10000"
        )
    names, memberships = _read_membership_table(inputs["table"])
    if len(names) > MAX_VENN_COLUMNS:
        raise ImplementationError(
            f"Venn analysis accepts 2 to {MAX_VENN_COLUMNS} set columns; "
            f"found {len(names)}"
        )
    set_sizes, intersections = _summarize(names, memberships, include_items)
    return {
        "set_count": len(names),
        "union_size": len(memberships),
        "set_sizes": set_sizes,
        "intersections": intersections,
    }
