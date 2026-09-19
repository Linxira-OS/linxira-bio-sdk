"""variant.stats.v1 — independent stdlib-only implementation.

Field-for-field counterpart of the Rust engine's ``vcf_stats``
(engine/crates/linxira-bio-core/src/variant.rs). The benchmark harness diffs
the ``result`` object of both backends with a 1e-6 relative tolerance, so the
VCF semantics below mirror the engine line by line:

* the first line must be ``##fileformat=VCFv<major>.<minor>`` with ASCII
  digit components and no third component
* ``##key=value`` meta lines: key non-empty, alphanumeric plus ``_ . -``
* ``#CHROM`` header: at least the 8 fixed columns, optional ``FORMAT``
  (column 9) plus unique non-empty sample names; record column counts must
  match the header exactly
* ALT ``.`` means no alleles; any empty/``.`` entry inside a comma list is
  malformed
* allele classes: symbolic (``*``, ``<DEL>``-style, or containing ``[`` /
  ``]``), indel (length differs from REF), SNP (length 1 == REF length),
  MNV (equal length >= 2)
* transition/transversion only for single-allele SNP records with A/C/G/T
  first bases; ``ti_tv_ratio`` is ``None`` without transversions
* genotypes: GT must be the first FORMAT field when present; alleles split
  on ``/`` and ``|``; ``.`` marks a missing genotype; indices above the
  alternate count are malformed
* gzip inputs are detected by magic bytes and decompressed transparently,
  matching ``vcf_stats_path``
"""

from __future__ import annotations

import gzip
from pathlib import Path
from typing import Any, BinaryIO, TextIO

CAPABILITY = "variant.stats.v1"
INPUT_ROLES = ("vcf",)
PARAMETERS: tuple[str, ...] = ()
# The reader dispatches on gzip magic bytes; declared gzip inputs are part
# of the contract (mirrors the rust engine's transparent decompression).
INPUT_COMPRESSION = {"vcf": ("none", "gzip")}
GZIP_MAGIC = b"\x1f\x8b"

NO_SAMPLES_WARNING = (
    "VCF header declares no samples; genotype metrics are unavailable"
)
NO_RECORDS_WARNING = "VCF contains no variant records"


class ImplementationError(ValueError):
    """An input the native engine would also reject, reported the same way."""


def software() -> list[dict[str, str]]:
    return []  # stdlib only


def _open_text(path: Path) -> TextIO:
    with path.open("rb") as probe:
        magic = probe.read(2)
    if magic == GZIP_MAGIC:
        return gzip.open(path, "rt", encoding="utf-8", errors="strict", newline="")
    return path.open("r", encoding="utf-8", errors="strict", newline="")


def _invalid_header(line: int, message: str) -> ImplementationError:
    return ImplementationError(f"invalid VCF header at line {line}: {message}")


def _malformed(line: int, message: str) -> ImplementationError:
    return ImplementationError(f"malformed VCF record at line {line}: {message}")


def _validate_file_format(line: str) -> None:
    if not line.startswith("##fileformat=VCFv"):
        raise _invalid_header(
            1, "the first line must be a ##fileformat=VCFv... declaration"
        )
    version = line[len("##fileformat=VCFv") :]
    components = version.split(".")
    if (
        len(components) != 2
        or not components[0]
        or not components[1]
        or not components[0].isascii()
        or not components[1].isascii()
        or not components[0].isdigit()
        or not components[1].isdigit()
    ):
        raise _invalid_header(1, f"invalid VCF version '{version}'")


def _validate_meta_line(line: str, line_number: int) -> None:
    body = line[2:]
    key, sep, _value = body.partition("=")
    if not sep:
        raise _invalid_header(
            line_number, "meta-information lines must use ##key=value syntax"
        )
    if not key or not all(
        character.isascii() and (character.isalnum() or character in "_-.")
        for character in key
    ):
        raise _invalid_header(line_number, f"invalid meta-information key '{key}'")


def _parse_column_header(line: str, line_number: int) -> tuple[int, int]:
    required = ["#CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER", "INFO"]
    columns = line.split("\t")
    if len(columns) < len(required):
        raise _invalid_header(
            line_number,
            f"expected at least 8 tab-separated columns, found {len(columns)}",
        )
    for index, expected in enumerate(required):
        if columns[index] != expected:
            raise _invalid_header(
                line_number,
                f"column {index + 1} must be {expected}, found '{columns[index]}'",
            )
    if len(columns) > len(required) and columns[8] != "FORMAT":
        raise _invalid_header(
            line_number, f"column 9 must be FORMAT, found '{columns[8]}'"
        )
    samples = columns[9:]
    seen = set()
    for sample in samples:
        if not sample:
            raise _invalid_header(line_number, "sample names must not be empty")
        if sample in seen:
            raise _invalid_header(line_number, f"duplicate sample name '{sample}'")
        seen.add(sample)
    return len(columns), len(samples)


def _parse_alternate_alleles(field: str, line_number: int) -> list[str]:
    if field == ".":
        return []
    alleles = field.split(",")
    if any(allele == "" or allele == "." for allele in alleles):
        raise _malformed(
            line_number,
            "ALT contains an empty or missing allele in a non-missing allele list",
        )
    return alleles


def _classify_allele(reference: str, alternate: str) -> str:
    if (
        alternate == "*"
        or (alternate.startswith("<") and alternate.endswith(">"))
        or "[" in alternate
        or "]" in alternate
    ):
        return "symbolic"
    if len(reference) != len(alternate):
        return "indel"
    if len(reference) == 1:
        return "snp"
    return "mnv"


def _substitution_kind(reference: str, alternate: str) -> str | None:
    if not reference or not alternate:
        return None
    ref = reference[0].upper()
    alt = alternate[0].upper()
    if ref not in "ACGT" or alt not in "ACGT" or ref == alt:
        return None
    if (ref, alt) in (("A", "G"), ("G", "A"), ("C", "T"), ("T", "C")):
        return "transition"
    return "transversion"


def _genotype_summary(
    genotype: str, alternate_count: int, line_number: int
) -> tuple[bool, int]:
    if genotype == "":
        raise _malformed(line_number, "GT value is empty")
    missing = False
    alternate_allele_count = 0
    for allele in genotype.replace("/", "|").split("|"):
        if allele == "":
            raise _malformed(line_number, f"invalid GT value '{genotype}'")
        if allele == ".":
            missing = True
            continue
        if not allele.isascii() or not allele.isdigit():
            raise _malformed(
                line_number, f"invalid GT allele '{allele}' in '{genotype}'"
            )
        index = int(allele)
        if index > alternate_count:
            raise _malformed(
                line_number,
                f"GT allele index {index} exceeds the {alternate_count} alternate alleles",
            )
        if index != 0:
            alternate_allele_count += 1
    return missing, alternate_allele_count


def _genotype_counts(
    columns: list[str],
    alternate_alleles: list[str],
    sample_count: int,
    line_number: int,
) -> tuple[int, int, int, int]:
    if sample_count == 0:
        return 0, 0, 0, 0
    format_fields = columns[8].split(":")
    if any(field == "" for field in format_fields):
        raise _malformed(line_number, "FORMAT contains an empty field name")
    if len(set(format_fields)) != len(format_fields):
        seen = set()
        for field in format_fields:
            if field in seen:
                raise _malformed(
                    line_number, f"FORMAT contains duplicate field name '{field}'"
                )
            seen.add(field)
    if "GT" not in format_fields:
        return 0, 0, 0, 0
    if format_fields.index("GT") != 0:
        raise _malformed(line_number, "GT must be the first FORMAT field when present")
    missing = called = carriers = alternate_called = 0
    for sample_index, sample in enumerate(columns[9:]):
        if sample == "":
            raise _malformed(line_number, f"sample column {sample_index + 1} is empty")
        values = sample.split(":")
        if len(values) > len(format_fields):
            raise _malformed(
                line_number,
                f"sample column {sample_index + 1} has more values than FORMAT declares",
            )
        genotype = values[0] if values else "."
        is_missing, alternate_allele_count = _genotype_summary(
            genotype, len(alternate_alleles), line_number
        )
        if is_missing:
            missing += 1
        else:
            called += 1
            alternate_called += alternate_allele_count
            if alternate_allele_count != 0:
                carriers += 1
    return missing, called, carriers, alternate_called


def run(inputs: dict[str, Path], parameters: dict[str, Any]) -> dict[str, Any]:
    del parameters  # no tunables: the contract of variant.stats.v1 is fixed
    path = inputs["vcf"]

    record_count = 0
    sample_count = 0
    pass_record_count = 0
    filtered_record_count = 0
    snp_count = indel_count = mnv_count = symbolic_count = 0
    multiallelic_record_count = 0
    transition_count = transversion_count = 0
    missing_genotype_count = called_genotype_count = 0
    carrier_genotype_count = alternate_allele_count = 0
    contig_counts: dict[str, int] = {}

    header = None
    saw_file_format = False

    with _open_text(path) as handle:
        for line_number, raw in enumerate(handle, start=1):
            line = raw.rstrip("\r\n")
            if header is None:
                if line_number == 1:
                    _validate_file_format(line)
                    saw_file_format = True
                    continue
                if line.startswith("##"):
                    if line.startswith("##fileformat="):
                        raise _invalid_header(
                            line_number,
                            "the fileformat declaration must appear exactly once "
                            "as the first line",
                        )
                    _validate_meta_line(line, line_number)
                    continue
                if line.startswith("#"):
                    if not saw_file_format:
                        raise _invalid_header(
                            line_number, "missing fileformat declaration"
                        )
                    header = _parse_column_header(line, line_number)
                    continue
                raise _invalid_header(
                    line_number,
                    "record data appears before the #CHROM column header",
                )

            if line.startswith("#") or line == "":
                raise _malformed(
                    line_number,
                    "blank lines are not permitted after the column header"
                    if line == ""
                    else "header or comment line appears after the #CHROM column header",
                )

            columns = line.split("\t")
            if len(columns) < 8:
                raise _malformed(
                    line_number,
                    f"expected at least 8 tab-separated columns, found {len(columns)}",
                )
            if len(columns) != header[0]:
                raise _malformed(
                    line_number,
                    f"expected {header[0]} columns to match the header, "
                    f"found {len(columns)}",
                )
            if columns[0] == "" or columns[0] == ".":
                raise _malformed(line_number, "CHROM must identify a contig")
            if (
                not columns[1].isascii()
                or not columns[1].isdigit()
                or int(columns[1]) <= 0
                or int(columns[1]) > 2**64 - 1
            ):
                raise _malformed(line_number, f"invalid POS value '{columns[1]}'")
            if columns[3] == "" or columns[3] == ".":
                raise _malformed(line_number, "REF must contain an allele")
            if columns[4] == "":
                raise _malformed(line_number, "ALT must contain an allele or '.'")
            if columns[6] == "":
                raise _malformed(
                    line_number, "FILTER must contain PASS, '.', or a filter name"
                )

            alleles = _parse_alternate_alleles(columns[4], line_number)
            missing, called, carriers, alt_called = _genotype_counts(
                columns, alleles, header[1], line_number
            )

            record_count += 1
            if columns[6] == "PASS":
                pass_record_count += 1
            elif columns[6] != ".":
                filtered_record_count += 1
            if len(alleles) > 1:
                multiallelic_record_count += 1

            for allele in alleles:
                allele_class = _classify_allele(columns[3], allele)
                if allele_class == "snp":
                    snp_count += 1
                elif allele_class == "indel":
                    indel_count += 1
                elif allele_class == "mnv":
                    mnv_count += 1
                else:
                    symbolic_count += 1

            if len(alleles) == 1 and _classify_allele(columns[3], alleles[0]) == "snp":
                kind = _substitution_kind(columns[3], alleles[0])
                if kind == "transition":
                    transition_count += 1
                elif kind == "transversion":
                    transversion_count += 1

            missing_genotype_count += missing
            called_genotype_count += called
            carrier_genotype_count += carriers
            alternate_allele_count += alt_called
            contig_counts[columns[0]] = contig_counts.get(columns[0], 0) + 1
            sample_count = header[1]

    if header is None:
        raise ImplementationError("VCF column header is missing")

    ti_tv_ratio = (
        transition_count / transversion_count if transversion_count != 0 else None
    )
    genotype_total = missing_genotype_count + called_genotype_count
    missing_genotype_rate = (
        missing_genotype_count / genotype_total if genotype_total != 0 else None
    )

    warnings: list[str] = []
    if sample_count == 0:
        warnings.append(NO_SAMPLES_WARNING)
    if record_count == 0:
        warnings.append(NO_RECORDS_WARNING)

    return {
        "record_count": record_count,
        "sample_count": sample_count,
        "pass_record_count": pass_record_count,
        "filtered_record_count": filtered_record_count,
        "snp_count": snp_count,
        "indel_count": indel_count,
        "mnv_count": mnv_count,
        "symbolic_count": symbolic_count,
        "multiallelic_record_count": multiallelic_record_count,
        "transition_count": transition_count,
        "transversion_count": transversion_count,
        "ti_tv_ratio": ti_tv_ratio,
        "missing_genotype_count": missing_genotype_count,
        "called_genotype_count": called_genotype_count,
        "carrier_genotype_count": carrier_genotype_count,
        "alternate_allele_count": alternate_allele_count,
        "missing_genotype_rate": missing_genotype_rate,
        "contig_counts": dict(sorted(contig_counts.items())),
        "warnings": warnings,
    }
