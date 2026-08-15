#!/usr/bin/env python3
"""Strict, local-only molecular descriptor computation backed by RDKit."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


PACK_ID = "org.linxira.chemistry-descriptors-rdkit"
PACK_VERSION = "0.1.0"
CAPABILITY = "chemistry.descriptors.v1"
EXPECTED_PYTHON = (3, 12)
EXPECTED_RDKIT = "2026.3.5"
EXPECTED_NUMPY = "2.5.2"

DESCRIPTOR_NAMES = [
    "molecular_weight",
    "logp",
    "tpsa",
    "hbd",
    "hba",
    "rotatable_bonds",
    "rings",
    "aromatic_rings",
    "formal_charge",
    "formula",
]


class RequestError(ValueError):
    """A stable, user-correctable request validation failure."""


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def core_version() -> str:
    return os.environ.get("LINXIRA_BIO_CORE_VERSION", "unknown")


def load_request(request_path: Path) -> dict[str, Any]:
    try:
        with request_path.open("r", encoding="utf-8") as handle:
            request = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise RequestError(f"cannot read request: {error}") from error
    if not isinstance(request, dict):
        raise RequestError("request must be an object")
    return request


def resolve_input(request: dict[str, Any]) -> Path:
    inputs = request.get("inputs")
    if not isinstance(inputs, list) or len(inputs) != 1:
        raise RequestError("chemistry.descriptors.v1 requires exactly one input artifact")
    files = inputs[0].get("files")
    if not isinstance(files, list) or len(files) != 1:
        raise RequestError("input artifact must contain exactly one file")
    path = files[0].get("path")
    if not isinstance(path, str) or not path:
        raise RequestError("input file path is missing")
    return Path(path)


def output_path_from(request: dict[str, Any]) -> Path:
    parameters = request.get("parameters")
    if not isinstance(parameters, dict):
        raise RequestError("request parameters must be an object")
    output = parameters.get("output")
    if not isinstance(output, str) or not output:
        raise RequestError("parameters.output is required")
    return Path(output)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_sdf(mol_text: str) -> list[dict[str, Any]]:
    """Parse minimal SDF records ($$$$ separated) into molecule texts."""
    return [
        record.strip()
        for record in mol_text.split("$$$$")
        if record.strip()
    ]


def compute_descriptors(molecule_text: str) -> dict[str, Any]:
    from rdkit import Chem
    from rdkit.Chem import Crippen, Descriptors, Lipinski, rdMolDescriptors

    mol = Chem.MolFromMolBlock(molecule_text)
    if mol is None:
        raise RequestError("RDKit could not parse the SDF molecule block")
    return {
        "molecular_weight": Descriptors.MolWt(mol),
        "logp": Crippen.MolLogP(mol),
        "tpsa": rdMolDescriptors.CalcTPSA(mol),
        "hbd": Lipinski.NumHDonors(mol),
        "hba": Lipinski.NumHAcceptors(mol),
        "rotatable_bonds": Lipinski.NumRotatableBonds(mol),
        "rings": rdMolDescriptors.CalcNumRings(mol),
        "aromatic_rings": rdMolDescriptors.CalcNumAromaticRings(mol),
        "formal_charge": Chem.GetFormalCharge(mol),
        "formula": rdMolDescriptors.CalcMolFormula(mol),
    }


def success_result(
    config: dict[str, Any],
    started_at: str,
    input_sha256: str,
    rows: list[dict[str, Any]],
) -> dict[str, Any]:
    lock_path = Path(__file__).resolve().parents[1] / "requirements.lock"
    return {
        "schema_version": "2",
        "job_id": config["job_id"],
        "capability": CAPABILITY,
        "status": "ok",
        "result": {
            "molecule_count": len(rows),
            "descriptor_names": DESCRIPTOR_NAMES,
            "rows": rows,
        },
        "artifacts": [
            {
                "artifact_id": "descriptor-table",
                "role": "descriptors",
                "kind": "table",
                "path": str(config["output_path"]),
                "format": "tsv",
                "media_type": "text/tab-separated-values",
                "size_bytes": config["output_path"].stat().st_size,
                "sha256": sha256_file(config["output_path"]),
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
                {"name": "RDKit", "version": EXPECTED_RDKIT, "package_id": "rdkit"},
                {"name": "NumPy", "version": EXPECTED_NUMPY, "package_id": "numpy"},
            ],
            "input_sha256": {"molecules": input_sha256},
            "command": ["python", "src/descriptors.py", "--request", "<request>", "--result", "<result>"],
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
    parser = argparse.ArgumentParser(description="Compute RDKit molecular descriptors")
    parser.add_argument("--request", required=True, type=Path, help="artifact-aware request JSON")
    parser.add_argument("--result", required=True, type=Path, help="machine-readable result JSON")
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    options = parse_arguments(arguments)
    started_at = utc_now()
    try:
        request = load_request(options.request)
        job_id = request.get("job_id")
        if not isinstance(job_id, str) or not job_id:
            raise RequestError("job_id is required")
        input_path = resolve_input(request)
        if not input_path.is_file():
            raise RequestError(f"input file does not exist: {input_path}")
        output_path = output_path_from(request)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        mol_text = input_path.read_text(encoding="utf-8")
        rows = [
            {"molecule_index": index, **compute_descriptors(record)}
            for index, record in enumerate(parse_sdf(mol_text), start=1)
        ]
        header = "molecule_index\t" + "\t".join(DESCRIPTOR_NAMES)
        lines = [header]
        for row in rows:
            lines.append(
                "\t".join(
                    str(row.get(name, ""))
                    for name in ["molecule_index", *DESCRIPTOR_NAMES]
                )
            )
        output_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        config = {
            "job_id": job_id,
            "output_path": output_path,
        }
        payload = success_result(
            config, started_at, sha256_file(input_path), rows
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
