"""sequence.stats.v1 — independent Biopython implementation.

Field-for-field counterpart of the Rust engine's ``fasta_stats``
(engine/crates/linxira-bio-core/src/sequence.rs). The benchmark harness diffs
the ``result`` object of both backends with a 1e-6 relative tolerance, so every
definition below is spelled out rather than delegated to a helper whose
semantics might drift:

* ``sequence_count``  number of ``>`` records
* ``total_bases``     sum of record lengths; a length counts every
                      non-whitespace character of the record body
* ``min_length`` / ``max_length`` / ``mean_length``
* ``n50`` / ``l50``   shortest length (and its 1-based rank) at which the
                      cumulative length of the longest-first ordering reaches
                      ``(total_bases + 1) // 2``
* ``au_n``            area under the Nx curve, ``sum(len^2) / total_bases``
* ``gc_percent``      ``100 * (G + C) / (A + C + G + T + U)`` — ambiguity codes
                      and gaps are excluded from the denominator
* ``n_count`` / ``n_percent``  ``N`` characters, the percentage over
                      ``total_bases``

Parsing is delegated to Biopython's ``SimpleFastaParser``; the Rust engine's
stricter error behaviour (empty identifier, sequence text before the first
header, no records at all) is reproduced explicitly because the parser alone
tolerates some of those inputs.
"""

from __future__ import annotations

import gzip
from pathlib import Path
from typing import Any, Iterator, TextIO

# Imported at module level so library loading counts as start-up (visible to
# the outer timer) rather than as analysis time in the self-reported figure.
try:
    import Bio
    from Bio.SeqIO.FastaIO import SimpleFastaParser
except ImportError:  # reported as a clean error envelope by run()
    Bio = None
    SimpleFastaParser = None

CAPABILITY = "sequence.stats.v1"
INPUT_ROLES = ("fasta",)
# Parameters the native implementation accepts (none besides the output
# directory the worker injects); anything else is a contract violation.
PARAMETERS: tuple[str, ...] = ()
GZIP_MAGIC = b"\x1f\x8b"


class ImplementationError(ValueError):
    """An input the native engine would also reject, reported the same way."""


def software() -> list[dict[str, str]]:
    if Bio is None:
        return []
    return [{"name": "Biopython", "version": Bio.__version__, "package_id": "biopython"}]


def _open_text(path: Path) -> TextIO:
    with path.open("rb") as probe:
        magic = probe.read(2)
    if magic == GZIP_MAGIC:
        return gzip.open(path, "rt", encoding="utf-8", errors="strict", newline="")
    return path.open("r", encoding="utf-8", errors="strict", newline="")


def _records(handle: TextIO) -> Iterator[tuple[str, str]]:
    if SimpleFastaParser is None:
        raise RuntimeError("locked dependency Biopython is not installed")
    # The Rust parser fails on sequence text before the first header, while
    # SimpleFastaParser skips it; peek at the first non-blank line to agree.
    position = handle.tell()
    line_number = 0
    for line in handle:
        line_number += 1
        if line.strip():
            if not line.lstrip().startswith(">"):
                raise ImplementationError(
                    f"sequence data appears before a FASTA header at line {line_number}"
                )
            break
    handle.seek(position)
    for title, sequence in SimpleFastaParser(handle):
        if not title.split():
            raise ImplementationError("FASTA header has no identifier")
        # SimpleFastaParser already drops spaces and carriage returns; the
        # engine ignores every ASCII whitespace character.
        yield title, sequence.replace("\t", "").replace("\x0b", "").replace("\x0c", "")


def run(inputs: dict[str, Path], parameters: dict[str, Any]) -> dict[str, Any]:
    del parameters  # no tunables: the contract of sequence.stats.v1 is fixed
    path = inputs["fasta"]
    lengths: list[int] = []
    gc_bases = 0
    canonical_bases = 0
    n_count = 0
    with _open_text(path) as handle:
        for _title, sequence in _records(handle):
            lengths.append(len(sequence))
            upper = sequence.upper()
            gc = upper.count("G") + upper.count("C")
            gc_bases += gc
            canonical_bases += gc + upper.count("A") + upper.count("T") + upper.count("U")
            n_count += upper.count("N")
    if not lengths:
        raise ImplementationError("FASTA contains no records")

    sequence_count = len(lengths)
    total_bases = sum(lengths)
    lengths.sort(reverse=True)
    threshold = (total_bases + 1) // 2
    cumulative = 0
    n50 = 0
    l50 = 0
    if threshold > 0:
        for index, length in enumerate(lengths):
            cumulative += length
            if cumulative >= threshold:
                n50 = length
                l50 = index + 1
                break
    squared_length_sum = 0.0
    for length in lengths:
        squared_length_sum += float(length) * float(length)

    return {
        "sequence_count": sequence_count,
        "total_bases": total_bases,
        "min_length": lengths[-1],
        "max_length": lengths[0],
        "mean_length": _ratio(total_bases, sequence_count),
        "n50": n50,
        "l50": l50,
        "au_n": squared_length_sum / float(total_bases) if total_bases else 0.0,
        "gc_percent": _ratio(gc_bases, canonical_bases) * 100.0,
        "n_count": n_count,
        "n_percent": _ratio(n_count, total_bases) * 100.0,
    }


def _ratio(numerator: int, denominator: int) -> float:
    if denominator == 0:
        return 0.0
    return float(numerator) / float(denominator)
