"""fastq.qc.v1 — independent stdlib implementation (exact counter arithmetic).

Counterpart of the Rust engine's FASTQ quality-control scan
(engine/crates/linxira-bio-core/src/fastq.rs). The parser reproduces the
engine's line-level rules exactly:

* CRLF line endings are tolerated (one trailing ``\\r`` is dropped);
* the header must begin with ``@`` and its first whitespace-delimited field
  is the identifier; a ``+`` separator may repeat it (mismatch = error);
* the sequence spans every line until the ``+`` separator line, each byte
  must be ASCII graphic, and an empty sequence is rejected;
* quality lines accumulate until the sequence length is reached, each byte
  must lie in 33..=126, overflow past the sequence length is rejected, and
  an explicit Phred+64 mode rejects bytes below the offset;
* an input with no records is rejected.

Counting follows the engine: GC and N are counted case-insensitively on the
uppercase base, per-cycle metrics are capped at ``max_cycles`` cycles, and
both Phred+33 and Phred+64 Q20/Q30 counters are tracked for every byte.
Encoding auto-detection is the engine's rule: an observed minimum quality
byte below 59 selects Phred+33; otherwise the encoding is reported as
``ambiguous`` with a warning (Phred+33 semantics still apply). All
arithmetic is exact counter arithmetic — no third-party library.
"""

from __future__ import annotations

import gzip
import sys
from pathlib import Path
from typing import Any

CAPABILITY = "fastq.qc.v1"
INPUT_ROLES = ("fastq",)
# The reader dispatches on gzip magic bytes; declared gzip inputs are part
# of the contract (mirrors the rust engine's transparent decompression).
INPUT_COMPRESSION = {"fastq": ("none", "gzip")}
PARAMETERS: tuple[str, ...] = ("max_cycles", "quality_encoding")
GZIP_MAGIC = b"\x1f\x8b"
DEFAULT_MAX_CYCLES = 500


class ImplementationError(ValueError):
    """An input the native engine would also reject, reported the same way."""


def software() -> list[dict[str, str]]:
    return [{"name": "CPython", "version": sys.version.split()[0]}]


def run(inputs: dict[str, Any], parameters: dict[str, Any]) -> dict[str, Any]:
    path = Path(inputs["fastq"])
    max_cycles = parameters.get("max_cycles", DEFAULT_MAX_CYCLES)
    if isinstance(max_cycles, bool) or not isinstance(max_cycles, int) or max_cycles < 0:
        raise ImplementationError("max_cycles must be a non-negative integer")
    encoding = parameters.get("quality_encoding", "auto")
    if encoding not in ("auto", "phred+33", "phred+64"):
        raise ImplementationError(f"unsupported quality encoding: {encoding}")

    with _open_bytes(path) as handle:
        return _scan(handle, max_cycles, encoding)


def _open_bytes(path: Path):
    with path.open("rb") as probe:
        magic = probe.read(2)
    if magic == GZIP_MAGIC:
        return gzip.open(path, "rb")
    return path.open("rb")


class _Cycle:
    """Per-cycle counters (the engine's ``CycleAccumulator``)."""

    __slots__ = ("base_count", "gc_count", "n_count", "quality_sum", "q20_33", "q30_33", "q20_64", "q30_64")

    def __init__(self) -> None:
        self.base_count = 0
        self.gc_count = 0
        self.n_count = 0
        self.quality_sum = 0
        self.q20_33 = 0
        self.q30_33 = 0
        self.q20_64 = 0
        self.q30_64 = 0


class _State:
    """File-level counters (the engine's ``QcAccumulator``)."""

    def __init__(self) -> None:
        self.read_count = 0
        self.total_bases = 0
        self.min_length: int | None = None
        self.max_length = 0
        self.gc_count = 0
        self.n_count = 0
        self.quality_sum = 0
        self.minimum_quality = 256
        self.q20_33 = 0
        self.q30_33 = 0
        self.q20_64 = 0
        self.q30_64 = 0
        self.cycles: list[_Cycle] = []
        self.cycles_truncated = False

    def add_base(self, cycle: int, byte: int, max_cycles: int) -> None:
        upper = byte - 0x20 if byte in (0x67, 0x63, 0x6E) else byte  # g c n → G C N
        if upper in (0x47, 0x43):  # G C
            self.gc_count += 1
        elif upper == 0x4E:  # N
            self.n_count += 1
        if cycle < max_cycles:
            cycle_state = self._cycle(cycle)
            cycle_state.base_count += 1
            if upper in (0x47, 0x43):
                cycle_state.gc_count += 1
            elif upper == 0x4E:
                cycle_state.n_count += 1

    def add_quality(self, cycle: int, byte: int, max_cycles: int) -> None:
        self.quality_sum += byte
        if byte < self.minimum_quality:
            self.minimum_quality = byte
        if byte >= 53:
            self.q20_33 += 1
        if byte >= 63:
            self.q30_33 += 1
        if byte >= 84:
            self.q20_64 += 1
        if byte >= 94:
            self.q30_64 += 1
        if cycle < max_cycles:
            cycle_state = self._cycle(cycle)
            cycle_state.quality_sum += byte
            if byte >= 53:
                cycle_state.q20_33 += 1
            if byte >= 63:
                cycle_state.q30_33 += 1
            if byte >= 84:
                cycle_state.q20_64 += 1
            if byte >= 94:
                cycle_state.q30_64 += 1

    def _cycle(self, index: int) -> _Cycle:
        while len(self.cycles) <= index:
            self.cycles.append(_Cycle())
        return self.cycles[index]


def _scan(handle, max_cycles: int, encoding: str) -> dict[str, Any]:
    state = _State()
    lines = _LineReader(handle)
    while True:
        header_line = lines.next_line()
        if header_line is None:
            break
        record = state.read_count + 1
        identifier = _parse_header(header_line, record, lines.line_number)
        sequence_length = _parse_sequence(lines, identifier, record, state, max_cycles)
        _parse_quality(lines, record, sequence_length, state, encoding, max_cycles)

        state.read_count += 1
        state.total_bases += sequence_length
        if state.min_length is None or sequence_length < state.min_length:
            state.min_length = sequence_length
        state.max_length = max(state.max_length, sequence_length)
        if sequence_length > max_cycles:
            state.cycles_truncated = True

    if state.read_count == 0:
        raise ImplementationError("FASTQ contains no records")

    if encoding == "phred+33":
        quality_encoding, applied_quality_offset = "phred+33", 33
    elif encoding == "phred+64":
        quality_encoding, applied_quality_offset = "phred+64", 64
    elif state.minimum_quality < 59:
        quality_encoding, applied_quality_offset = "phred+33", 33
    else:
        quality_encoding, applied_quality_offset = "ambiguous", 33

    warnings: list[str] = []
    if quality_encoding == "ambiguous":
        warnings.append(
            "quality bytes are compatible with both Phred+33 and legacy Phred+64/Solexa "
            "encodings; metrics use Phred+33 unless quality_encoding is explicitly overridden"
        )
    if state.cycles_truncated:
        warnings.append(f"per-cycle metrics are capped at {max_cycles} cycle(s)")

    use_phred64 = applied_quality_offset == 64
    q20_count = state.q20_64 if use_phred64 else state.q20_33
    q30_count = state.q30_64 if use_phred64 else state.q30_33

    return {
        "read_count": state.read_count,
        "total_bases": state.total_bases,
        "min_length": state.min_length,
        "max_length": state.max_length,
        "mean_length": _ratio(state.total_bases, state.read_count),
        "gc_percent": _percent(state.gc_count, state.total_bases),
        "n_percent": _percent(state.n_count, state.total_bases),
        "mean_quality": _mean_quality(state.quality_sum, state.total_bases, applied_quality_offset),
        "q20_percent": _percent(q20_count, state.total_bases),
        "q30_percent": _percent(q30_count, state.total_bases),
        "quality_encoding": quality_encoding,
        "applied_quality_offset": applied_quality_offset,
        "per_cycle": [
            _cycle_metrics(index, cycle, applied_quality_offset)
            for index, cycle in enumerate(state.cycles)
        ],
        "warnings": warnings,
    }


class _LineReader:
    """Byte lines with one trailing ``\\n`` and one trailing ``\\r`` dropped, like the engine."""

    def __init__(self, handle):
        self._handle = handle
        self.line_number = 0

    def next_line(self) -> bytes | None:
        line = self._handle.readline()
        if line == b"":
            return None
        self.line_number += 1
        if line.endswith(b"\n"):
            line = line[:-1]
        if line.endswith(b"\r"):
            line = line[:-1]
        return line

    def next_expected_line(self) -> int:
        return self.line_number + 1


def _parse_header(line: bytes, record: int, line_number: int) -> bytes:
    if not line.startswith(b"@"):
        raise ImplementationError(
            _malformed(record, line_number, "expected a header beginning with '@'")
        )
    identifier = _first_ascii_field(line[1:])
    if identifier is None:
        raise ImplementationError(_malformed(record, line_number, "header has no identifier"))
    return identifier


def _parse_sequence(
    lines: _LineReader,
    identifier: bytes,
    record: int,
    state: _State,
    max_cycles: int,
) -> int:
    sequence_length = 0
    while True:
        line = lines.next_line()
        if line is None:
            raise ImplementationError(
                f"truncated FASTQ record {record} at line {lines.next_expected_line()}: "
                "expected a '+' separator line"
            )
        if line.startswith(b"+"):
            if sequence_length == 0:
                raise ImplementationError(
                    _malformed(record, lines.line_number, "sequence is empty")
                )
            _validate_separator(line[1:], identifier, record, lines.line_number)
            return sequence_length
        if not line:
            raise ImplementationError(
                _malformed(record, lines.line_number, "sequence line is empty")
            )
        for column, byte in enumerate(line):
            if not (0x20 < byte < 0x7F):
                raise ImplementationError(
                    _malformed(
                        record,
                        lines.line_number,
                        f"invalid sequence byte 0x{byte:02x} at column {column + 1}",
                    )
                )
            state.add_base(sequence_length, byte, max_cycles)
            sequence_length += 1


def _validate_separator(separator: bytes, identifier: bytes, record: int, line: int) -> None:
    separator_identifier = _first_ascii_field(separator)
    if separator_identifier is not None and separator_identifier != identifier:
        raise ImplementationError(
            _malformed(record, line, "separator identifier does not match the header identifier")
        )


def _parse_quality(
    lines: _LineReader,
    record: int,
    sequence_length: int,
    state: _State,
    encoding: str,
    max_cycles: int,
) -> None:
    quality_length = 0
    while quality_length < sequence_length:
        line = lines.next_line()
        if line is None:
            raise ImplementationError(
                f"truncated FASTQ record {record} at line {lines.next_expected_line()}: "
                f"sequence length is {sequence_length}, but only {quality_length} "
                "quality values were present"
            )
        if not line:
            raise ImplementationError(
                _malformed(record, lines.line_number, "quality line is empty")
            )
        resulting_length = quality_length + len(line)
        if resulting_length > sequence_length:
            raise ImplementationError(
                _malformed(
                    record,
                    lines.line_number,
                    f"sequence length is {sequence_length}, but quality length is {resulting_length}",
                )
            )
        for column, byte in enumerate(line):
            if not (33 <= byte <= 126):
                raise ImplementationError(
                    _malformed(
                        record,
                        lines.line_number,
                        f"invalid quality byte 0x{byte:02x} at column {column + 1}",
                    )
                )
            if encoding == "phred+64" and byte < 64:
                raise ImplementationError(
                    _malformed(
                        record,
                        lines.line_number,
                        f"quality byte 0x{byte:02x} at column {column + 1} is below the Phred+64 offset",
                    )
                )
            state.add_quality(quality_length + column, byte, max_cycles)
        quality_length = resulting_length


def _cycle_metrics(index: int, cycle: _Cycle, quality_offset: int) -> dict[str, Any]:
    use_phred64 = quality_offset == 64
    q20_count = cycle.q20_64 if use_phred64 else cycle.q20_33
    q30_count = cycle.q30_64 if use_phred64 else cycle.q30_33
    return {
        "cycle": index + 1,
        "base_count": cycle.base_count,
        "gc_percent": _percent(cycle.gc_count, cycle.base_count),
        "n_percent": _percent(cycle.n_count, cycle.base_count),
        "mean_quality": _mean_quality(cycle.quality_sum, cycle.base_count, quality_offset),
        "q20_percent": _percent(q20_count, cycle.base_count),
        "q30_percent": _percent(q30_count, cycle.base_count),
    }


def _first_ascii_field(line: bytes) -> bytes | None:
    whitespace = b" \t\n\r\x0b\x0c"
    start = 0
    end = len(line)
    while start < end and line[start] in whitespace:
        start += 1
    while end > start and line[end - 1] in whitespace:
        end -= 1
    if start == end:
        return None
    for index in range(start, end):
        if line[index] in whitespace:
            return line[start:index]
    return line[start:end]


def _malformed(record: int, line: int, message: str) -> str:
    return f"malformed FASTQ record {record} at line {line}: {message}"


def _ratio(numerator: int, denominator: int) -> float:
    if denominator == 0:
        return 0.0
    return numerator / denominator


def _percent(numerator: int, denominator: int) -> float:
    return _ratio(numerator, denominator) * 100.0


def _mean_quality(quality_ascii_sum: int, base_count: int, quality_offset: int) -> float:
    if base_count == 0:
        return 0.0
    return quality_ascii_sum / base_count - quality_offset
