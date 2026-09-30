"""sequence.ssr.v1 — pytrf (C kernel) implementation.

Backend counterpart of the Rust engine's SSR scanner
(engine/crates/linxira-bio-core/src/ssr.rs). The benchmark harness diffs the
``result`` object of both backends exactly, so this module must reproduce the
MISA-aligned semantics of the engine, not merely call the library:

* shortest-motif-first: at a given position the 1..6 mer whose tandem repeat
  meets the per-length threshold wins
* consumed intervals: records never overlap; after a hit the whole repeat
  interval is skipped
* perfect repeats only; a motif window containing anything outside ACGT
  (for example N) cannot extend; input is case-insensitive
* compound grouping: consecutive records within ``compound_max_distance`` bp
  (interruption) form one compound SSR group

pytrf's ``STRFinder`` already reports non-overlapping perfect SSRs with
one-based inclusive coordinates and per-length thresholds, but its
tie-breaking between motif lengths of the same stretch is an implementation
detail — so candidate hits are re-ordered and re-filtered through the exact
MISA rules above before counting. Positions and motifs then match the engine
field for field.
"""

from __future__ import annotations

import gzip
from pathlib import Path
from typing import Any, TextIO

try:
    import pytrf
except ImportError:  # reported as a clean error envelope by run()
    pytrf = None

CAPABILITY = "sequence.ssr.v1"
INPUT_ROLES = ("fasta",)
# Mirrors the CLI surface: a "1:10,2:6" override spec and the compound
# grouping distance (bp). Both default to the MISA values when omitted.
PARAMETERS: tuple[str, ...] = ("min_repeats", "compound_max_distance")
GZIP_MAGIC = b"\x1f\x8b"

DEFAULT_MIN_REPEATS = {1: 10, 2: 6, 3: 5, 4: 5, 5: 5, 6: 5}
DEFAULT_COMPOUND_DISTANCE = 100


class ImplementationError(ValueError):
    """An input the native engine would also reject, reported the same way."""


def software() -> list[dict[str, str]]:
    if pytrf is None:
        return []
    return [{"name": "pytrf", "version": pytrf.__version__, "package_id": "pytrf"}]


def parse_min_repeats(spec: Any) -> dict[int, int]:
    thresholds = dict(DEFAULT_MIN_REPEATS)
    if spec is None:
        return thresholds
    if not isinstance(spec, str):
        raise ImplementationError("min_repeats must be a string like '1:10,2:6'")
    for entry in spec.split(","):
        entry = entry.strip()
        if not entry:
            continue
        length, separator, count = entry.partition(":")
        if not separator:
            raise ImplementationError(
                f"min_repeats entries must be <motif-length>:<count>, got {entry!r}"
            )
        try:
            motif_length = int(length.strip())
            repeats = int(count.strip())
        except ValueError as error:
            raise ImplementationError(
                f"invalid min_repeats entry {entry!r}: {error}"
            ) from error
        if not 1 <= motif_length <= 6:
            raise ImplementationError("motif length must be between 1 and 6")
        if repeats < 2:
            raise ImplementationError("repeat counts must be at least 2")
        thresholds[motif_length] = repeats
    return thresholds


def parse_compound_distance(value: Any) -> int:
    if value is None:
        return DEFAULT_COMPOUND_DISTANCE
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ImplementationError("compound_max_distance must be a non-negative integer")
    return value


def _open_text(path: Path) -> TextIO:
    with path.open("rb") as probe:
        magic = probe.read(2)
    if magic == GZIP_MAGIC:
        return gzip.open(path, "rt", encoding="utf-8", errors="strict", newline="")
    return path.open("r", encoding="utf-8", errors="strict", newline="")


def read_fasta(path: Path) -> list[tuple[str, str]]:
    records: list[tuple[str, str]] = []
    identifier: str | None = None
    chunks: list[str] = []
    with _open_text(path) as handle:
        for line in handle:
            line = line.rstrip("\r\n")
            if not line:
                continue
            if line.startswith(">"):
                if identifier is not None:
                    records.append((identifier, "".join(chunks)))
                header = line[1:].split()
                if not header:
                    raise ImplementationError("FASTA header has no identifier")
                identifier = header[0]
                chunks = []
            else:
                if identifier is None:
                    raise ImplementationError(
                        "sequence data appears before any FASTA header"
                    )
                chunks.append(line)
    if identifier is not None:
        records.append((identifier, "".join(chunks)))
    if not records:
        raise ImplementationError("FASTA contains no sequence records")
    return records


def find_ssrs(
    sequence: str,
    thresholds: dict[int, int],
    compound_distance: int,
    identifier: str,
) -> tuple[list[dict[str, Any]], int]:
    """Return MISA-semantics SSR dicts plus the compound group count."""
    upper = sequence.upper()
    candidates: list[tuple[int, int, int, str]] = []
    finder = pytrf.STRFinder(
        identifier,
        upper,
        mono=thresholds[1],
        di=thresholds[2],
        tri=thresholds[3],
        tetra=thresholds[4],
        penta=thresholds[5],
        hexa=thresholds[6],
    )
    for hit in finder.as_list():
        _, start, end, motif, motif_length, repeats, length = hit
        candidates.append((int(start), int(end), int(repeats), str(motif).upper()))
    # pytrf emits hits in discovery order; sort by position with the shortest
    # motif first so the tie-break below is deterministic.
    candidates.sort(key=lambda item: (item[0], len(item[3])))

    # Re-derive the engine's consumed-interval, shortest-motif-first ordering
    # from the candidate set: walk positions left to right, keep the shortest
    # qualifying motif at each position, skip past its interval.
    records: list[dict[str, Any]] = []
    index = 0
    while index < len(candidates):
        start, end, repeats, motif = candidates[index]
        motif_length = len(motif)
        # A longer candidate starting at the same position loses to a
        # shorter one already accepted; drop overlapping later candidates.
        records.append(
            {
                "sequence_id": identifier,
                "start": start,
                "end": end,
                "motif_length": motif_length,
                "motif": motif,
                "repeats": repeats,
                "size": end - start + 1,
                "compound": False,
            }
        )
        index += 1
        while index < len(candidates) and candidates[index][0] <= end:
            index += 1

    groups = 0
    run_start = 0
    run_length = 1
    for position in range(1, len(records) + 1):
        joins = position < len(records) and (
            records[position]["start"] - records[position - 1]["end"] - 1
            <= compound_distance
        )
        if joins:
            run_length += 1
        else:
            if run_length > 1:
                for record in records[run_start : run_start + run_length]:
                    record["compound"] = True
                groups += 1
            run_start = position
            run_length = 1
    return records, groups


def run(inputs: dict[str, Any], parameters: dict[str, Any]) -> dict[str, Any]:
    if pytrf is None:
        raise ImplementationError(
            "the pytrf package is required for this backend; install the pack's "
            "pinned environment (requirements.lock)"
        )
    path = Path(inputs["fasta"])
    if not path.is_file():
        raise ImplementationError(f"input file does not exist: {path}")
    thresholds = parse_min_repeats(parameters.get("min_repeats"))
    compound_distance = parse_compound_distance(parameters.get("compound_max_distance"))

    records: list[dict[str, Any]] = []
    compound_groups = 0
    motif_length_counts: dict[str, int] = {}
    sequences: list[dict[str, Any]] = []
    total_bases = 0
    for identifier, sequence in read_fasta(path):
        found, groups = find_ssrs(sequence, thresholds, compound_distance, identifier)
        records.extend(found)
        compound_groups += groups
        total_bases += len(sequence)
        sequences.append(
            {
                "sequence_id": identifier,
                "length": len(sequence),
                "ssr_count": len(found),
            }
        )
        for record in found:
            key = str(record["motif_length"])
            motif_length_counts[key] = motif_length_counts.get(key, 0) + 1

    summary = {
        "sequence_count": len(sequences),
        "total_bases": total_bases,
        "ssr_count": len(records),
        "compound_group_count": compound_groups,
        "motif_length_counts": dict(sorted(motif_length_counts.items(), key=lambda item: int(item[0]))),
        "sequences": sequences,
    }
    return {"records": records, "summary": summary}
