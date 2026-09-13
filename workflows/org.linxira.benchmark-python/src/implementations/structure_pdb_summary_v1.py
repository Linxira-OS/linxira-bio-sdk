"""structure.pdb.summary.v1 — independent implementation.

A field-for-field port of the Rust engine's PDB summary parser
(engine/crates/linxira-bio-core/src/structure.rs — prose reference, see the
pack README for why a hand-rolled port beats an object-model library here):
same fixed-width column extraction, the same MODEL/ENDMDL state machine, the
same residue/chain grouping (first-appearance order, chains sorted by id),
the same element inference table, the same AlphaFold pLDDT interpretation
(bands 90/70/50), and the same warnings. Biopython's ``Bio.PDB`` deliberately
takes a different object-model view (altloc collapsing, hetflag residue ids),
which would break the field-level comparison this backend exists for; the
engine's parser IS the specification being cross-validated.
"""

from __future__ import annotations

import gzip
import math
from pathlib import Path
from typing import Any

CAPABILITY = "structure.pdb.summary.v1"
INPUT_ROLES = ("pdb",)
PARAMETERS: tuple[str, ...] = ("interpret_b_factors_as_plddt",)
GZIP_MAGIC = b"\x1f\x8b"

MAX_PLAIN_INPUT_BYTES = 128 * 1024 * 1024
MAX_COMPRESSED_INPUT_BYTES = 64 * 1024 * 1024
MAX_DECOMPRESSED_BYTES = 128 * 1024 * 1024
MAX_ATOMS = 100_000
MAX_RESULT_RECORDS = 300_000
TWO_LETTER_ELEMENTS = {
    "BR", "CA", "CD", "CL", "CO", "CU", "FE", "HG", "LI", "MG", "MN",
    "NA", "NI", "PB", "ZN",
}

F64_EPSILON = 2.220446049250313e-16


class ImplementationError(ValueError):
    """An input the native engine would also reject."""


def software() -> list[dict[str, str]]:
    return [{"name": "CPython", "version": _python_version()}]


def _python_version() -> str:
    import sys

    return sys.version.split()[0]


def _field(line: str, start: int, end: int) -> str:
    # Python slicing truncates past the end exactly like the engine's
    # `end.min(line.len())`.
    return line[start:end]


def _optional(line: str, start: int, end: int) -> str | None:
    value = _field(line, start, end).strip()
    return value if value else None


def _required(line: str, start: int, end: int, name: str, line_number: int) -> str:
    value = _optional(line, start, end)
    if value is None:
        raise ImplementationError(f"malformed PDB record at line {line_number}: {name} is empty")
    return value


def _number(value: str, name: str, line_number: int) -> float:
    try:
        parsed = float(value)
    except ValueError:
        raise ImplementationError(
            f"malformed PDB record at line {line_number}: invalid {name} {value!r}: not a number"
        ) from None
    if not math.isfinite(parsed):
        raise ImplementationError(
            f"malformed PDB record at line {line_number}: invalid {name} {value!r}: must be finite"
        )
    return parsed


def _optional_number(line: str, start: int, end: int, name: str, line_number: int) -> float | None:
    value = _optional(line, start, end)
    return None if value is None else _number(value, name, line_number)


def _normalize_element(value: str) -> str:
    if len(value) > 2 or not value.isascii() or not value.isalpha():
        raise ImplementationError(f"invalid element field {value!r}")
    return value[0].upper() + value[1:].lower()


def _infer_element(raw_atom_name: str) -> str | None:
    for first_index, character in enumerate(raw_atom_name):
        if character.isascii() and character.isalpha():
            break
    else:
        return None
    first = raw_atom_name[first_index].upper()
    if (
        first_index == 0
        and first_index + 1 < len(raw_atom_name)
        and raw_atom_name[first_index + 1].isascii()
        and raw_atom_name[first_index + 1].isalpha()
    ):
        candidate = first + raw_atom_name[first_index + 1].upper()
        if candidate in TWO_LETTER_ELEMENTS:
            return candidate[0] + candidate[1].lower()
    return first


def _parse_atom(line: str, line_number: int, model_id: str, record: str, atom_index: int) -> dict[str, Any]:
    if len(line) < 54:
        raise ImplementationError(
            f"malformed PDB record at line {line_number}: {record} record is shorter than "
            "the coordinate columns"
        )
    return {
        "index": atom_index,
        "serial": _required(line, 6, 11, "atom serial", line_number),
        "record": "atom" if record == "ATOM" else "hetatm",
        "model_id": model_id,
        "residue_index": 0,
        "name": _required(line, 12, 16, "atom name", line_number),
        "alternate_location": _optional(line, 16, 17),
        "residue_name": _required(line, 17, 20, "residue name", line_number),
        "chain_id": _field(line, 21, 22).strip(),
        "residue_sequence_number": _required(line, 22, 26, "residue sequence number", line_number),
        "insertion_code": _optional(line, 26, 27),
        "position": {
            "x": _number(_required(line, 30, 38, "x coordinate", line_number), "x coordinate", line_number),
            "y": _number(_required(line, 38, 46, "y coordinate", line_number), "y coordinate", line_number),
            "z": _number(_required(line, 46, 54, "z coordinate", line_number), "z coordinate", line_number),
        },
        "occupancy": _optional_number(line, 54, 60, "occupancy", line_number),
        "b_factor": _optional_number(line, 60, 66, "B-factor", line_number),
        "element": (
            None
            if (raw_element := _optional(line, 76, 78)) is None
            else _normalize_element(raw_element)
        ),
        "formal_charge": _optional(line, 78, 80),
    }


def _numeric_summary(values: list[float], quantity: str) -> dict[str, Any] | None:
    if not values:
        return None
    total = 0.0
    for value in values:
        total += value
        if not math.isfinite(total):
            raise ImplementationError(f"PDB {quantity} exceeds the supported finite numeric range")
    mean = total / len(values)
    if not math.isfinite(mean):
        raise ImplementationError(f"PDB {quantity} exceeds the supported finite numeric range")
    return {
        "count": len(values),
        "min": min(values),
        "max": max(values),
        "mean": mean,
    }


def run(inputs: dict[str, Path], parameters: dict[str, Any]) -> dict[str, Any]:
    interpret_plddt = bool(parameters.get("interpret_b_factors_as_plddt", False))
    path = inputs["pdb"]
    with path.open("rb") as probe:
        magic = probe.read(2)
        source_bytes = path.stat().st_size
    compressed = magic == GZIP_MAGIC
    limit = MAX_COMPRESSED_INPUT_BYTES if compressed else MAX_PLAIN_INPUT_BYTES
    if source_bytes > limit:
        raise ImplementationError(
            f"PDB source byte count exceeds the limit of {limit}"
        )
    opener = gzip.open if compressed else open

    atoms: list[dict[str, Any]] = []
    residue_order: list[dict[str, Any]] = []
    residue_positions: dict[tuple, int] = {}
    chain_keys: set[tuple[str, str]] = set()
    model_order: list[str] = []
    seen_models: set[str] = set()
    current_model: str | None = None
    explicit_models = False
    inferred_count = 0
    missing_count = 0
    warnings: list[str] = []
    result_record_count = 0
    decompressed_bytes = 0

    def reserve(additional: int) -> None:
        nonlocal result_record_count
        result_record_count += additional
        if result_record_count > MAX_RESULT_RECORDS:
            raise ImplementationError(
                f"PDB result record count exceeds the limit of {MAX_RESULT_RECORDS}"
            )

    with opener(path, "rt", encoding="utf-8", errors="strict", newline="") as handle:
        for line_number, raw_line in enumerate(handle, start=1):
            decompressed_bytes += len(raw_line.encode("utf-8", errors="strict"))
            if decompressed_bytes > MAX_DECOMPRESSED_BYTES:
                raise ImplementationError(
                    f"PDB decompressed byte count exceeds the limit of {MAX_DECOMPRESSED_BYTES}"
                )
            line = raw_line.rstrip("\r\n")
            if not line.isascii():
                raise ImplementationError(
                    f"malformed PDB record at line {line_number}: records must use ASCII "
                    "fixed-width fields"
                )
            record = _field(line, 0, 6).strip()
            if record == "END":
                break
            if record == "MODEL":
                if current_model is not None:
                    raise ImplementationError(
                        f"malformed PDB record at line {line_number}: nested MODEL records "
                        "are not permitted"
                    )
                if not explicit_models and atoms:
                    raise ImplementationError(
                        f"malformed PDB record at line {line_number}: MODEL appears after "
                        "coordinates that were not enclosed by a model"
                    )
                explicit_models = True
                model_id = _field(line, 10, 14).strip() or str(len(model_order) + 1)
                if model_id in seen_models:
                    raise ImplementationError(
                        f"malformed PDB record at line {line_number}: duplicate MODEL "
                        f"identifier {model_id!r}"
                    )
                seen_models.add(model_id)
                reserve(1)
                model_order.append(model_id)
                current_model = model_id
            elif record == "ENDMDL":
                if not explicit_models or current_model is None:
                    raise ImplementationError(
                        f"malformed PDB record at line {line_number}: ENDMDL does not close "
                        "an active MODEL"
                    )
                current_model = None
            elif record in ("ATOM", "HETATM"):
                if explicit_models:
                    if current_model is None:
                        raise ImplementationError(
                            f"malformed PDB record at line {line_number}: coordinate appears "
                            "outside an explicit MODEL"
                        )
                    model_id = current_model
                else:
                    if not model_order:
                        reserve(1)
                        model_order.append("1")
                        seen_models.add("1")
                    model_id = "1"
                if len(atoms) >= MAX_ATOMS:
                    raise ImplementationError(
                        f"PDB atom count exceeds the limit of {MAX_ATOMS}"
                    )
                atom = _parse_atom(line, line_number, model_id, record, len(atoms))
                key = (
                    atom["model_id"],
                    atom["chain_id"],
                    atom["residue_sequence_number"],
                    atom["insertion_code"],
                    atom["residue_name"],
                    atom["record"] == "hetatm",
                )
                is_new_residue = key not in residue_positions
                is_new_chain = (atom["model_id"], atom["chain_id"]) not in chain_keys
                reserve(1 + int(is_new_residue) + int(is_new_chain))
                if is_new_chain:
                    chain_keys.add((atom["model_id"], atom["chain_id"]))
                if is_new_residue:
                    residue_positions[key] = len(residue_order)
                    residue_order.append(
                        {
                            "key": key,
                            "atom_count": 0,
                            "b_factor_sum": 0.0,
                            "b_factor_count": 0,
                        }
                    )
                residue = residue_order[residue_positions[key]]
                residue["atom_count"] += 1
                if atom["b_factor"] is not None:
                    total = residue["b_factor_sum"] + atom["b_factor"]
                    if not math.isfinite(total):
                        raise ImplementationError(
                            f"malformed PDB record at line {line_number}: residue B-factor "
                            "sum exceeds the supported finite numeric range"
                        )
                    residue["b_factor_sum"] = total
                    residue["b_factor_count"] += 1
                atom["residue_index"] = residue_positions[key]
                if atom["element"] is None:
                    inferred = _infer_element(_field(line, 12, 16))
                    if inferred is not None:
                        atom["element"] = inferred
                        inferred_count += 1
                    else:
                        missing_count += 1
                atoms.append(atom)

    if not atoms:
        raise ImplementationError("PDB contains no ATOM or HETATM records")
    if explicit_models and current_model is not None:
        warnings.append("the final MODEL has no ENDMDL record")
    if inferred_count > 0:
        warnings.append(
            f"inferred elements from atom-name alignment for {inferred_count} atoms"
        )
    if missing_count > 0:
        warnings.append(f"could not determine an element for {missing_count} atoms")

    # Bounds: min/max over every atom coordinate, then span/center with the
    # engine's finite checks.
    minimum = dict(atoms[0]["position"])
    maximum = dict(atoms[0]["position"])
    for atom in atoms[1:]:
        for axis in ("x", "y", "z"):
            minimum[axis] = min(minimum[axis], atom["position"][axis])
            maximum[axis] = max(maximum[axis], atom["position"][axis])
    span: dict[str, float] = {}
    center: dict[str, float] = {}
    for axis in ("x", "y", "z"):
        difference = maximum[axis] - minimum[axis]
        if not math.isfinite(difference):
            raise ImplementationError(
                f"PDB {axis}-coordinate span exceeds the supported finite numeric range"
            )
        span[axis] = difference
        total = minimum[axis] + difference / 2.0
        if not math.isfinite(total):
            raise ImplementationError(
                f"PDB {axis}-coordinate center exceeds the supported finite numeric range"
            )
        center[axis] = total
    bounds = {"min": minimum, "max": maximum, "center": center, "span": span}

    b_factor_summary = _numeric_summary(
        [atom["b_factor"] for atom in atoms if atom["b_factor"] is not None],
        "B-factor summary",
    )

    output_residues: list[dict[str, Any]] = []
    for index, residue in enumerate(residue_order):
        key = residue["key"]
        output_residues.append(
            {
                "index": index,
                "model_id": key[0],
                "chain_id": key[1],
                "sequence_number": key[2],
                "insertion_code": key[3],
                "name": key[4],
                "is_hetero": key[5],
                "atom_count": residue["atom_count"],
                "plddt": None,
            }
        )

    alphafold_confidence = None
    if interpret_plddt:
        for atom in atoms:
            if atom["record"] != "atom":
                continue
            value = atom["b_factor"]
            if value is None:
                raise ImplementationError(
                    "malformed PDB record: polymer atom "
                    f"{atom['serial']} lacks the B-factor required for pLDDT interpretation"
                )
            if not 0.0 <= value <= 100.0:
                raise ImplementationError(
                    f"malformed PDB record: polymer atom {atom['serial']} has B-factor "
                    f"{value}, outside the pLDDT range 0..100"
                )
        values: list[float] = []
        bands = {"very_high_count": 0, "confident_count": 0, "low_count": 0, "very_low_count": 0}
        for index, residue in enumerate(residue_order):
            if residue["key"][5]:
                continue
            if residue["b_factor_count"] != residue["atom_count"]:
                raise ImplementationError(
                    f"malformed PDB record: residue {residue['key'][4]} "
                    f"{residue['key'][2]} lacks complete B-factor values"
                )
            value = residue["b_factor_sum"] / residue["b_factor_count"]
            output_residues[index]["plddt"] = value
            values.append(value)
            if value >= 90.0:
                bands["very_high_count"] += 1
            elif value >= 70.0:
                bands["confident_count"] += 1
            elif value >= 50.0:
                bands["low_count"] += 1
            else:
                bands["very_low_count"] += 1
        summary = _numeric_summary(values, "AlphaFold pLDDT summary")
        if summary is None:
            raise ImplementationError(
                "malformed PDB record: AlphaFold pLDDT interpretation requires at least "
                "one ATOM residue"
            )
        alphafold_confidence = {
            "source": "pdb-b-factor-explicit",
            "residue_count": summary["count"],
            "min_plddt": summary["min"],
            "max_plddt": summary["max"],
            "mean_plddt": summary["mean"],
            "bands": bands,
        }
        warnings.append(
            "B-factor values were interpreted as AlphaFold pLDDT because the caller "
            "explicitly requested it; PDB content alone does not establish AlphaFold provenance"
        )

    # Chains are aggregated per model in BTreeMap order (chain id ascending).
    models: list[dict[str, Any]] = []
    for model_id in model_order:
        model_atoms = [atom for atom in atoms if atom["model_id"] == model_id]
        model_residues = [residue for residue in output_residues if residue["model_id"] == model_id]
        chain_ids = sorted({residue["chain_id"] for residue in model_residues})
        chains = []
        for chain_id in chain_ids:
            chain_residues = [r for r in model_residues if r["chain_id"] == chain_id]
            chains.append(
                {
                    "chain_id": chain_id,
                    "atom_count": sum(
                        1
                        for atom in model_atoms
                        if atom["chain_id"] == chain_id
                    ),
                    "residue_count": len(chain_residues),
                    "polymer_residue_count": sum(
                        1 for r in chain_residues if not r["is_hetero"]
                    ),
                    "hetero_residue_count": sum(1 for r in chain_residues if r["is_hetero"]),
                }
            )
        models.append(
            {
                "model_id": model_id,
                "atom_count": len(model_atoms),
                "residue_count": len(model_residues),
                "chains": chains,
            }
        )

    element_counts: dict[str, int] = {}
    for atom in atoms:
        element = atom["element"] or "unknown"
        element_counts[element] = element_counts.get(element, 0) + 1
    polymer_atom_count = sum(1 for atom in atoms if atom["record"] == "atom")

    return {
        "format": "pdb",
        "coordinate_units": "angstrom",
        "model_count": len(models),
        "chain_count": sum(len(model["chains"]) for model in models),
        "residue_count": len(output_residues),
        "atom_count": len(atoms),
        "polymer_atom_count": polymer_atom_count,
        "hetero_atom_count": len(atoms) - polymer_atom_count,
        "element_counts": {key: element_counts[key] for key in sorted(element_counts)},
        "bounds": bounds,
        "b_factor_summary": b_factor_summary,
        "alphafold_confidence": alphafold_confidence,
        "models": models,
        "residues": output_residues,
        "atoms": atoms,
        "warnings": warnings,
    }
