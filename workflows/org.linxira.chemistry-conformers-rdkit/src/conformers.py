#!/usr/bin/env python3
"""Strict, local-only conformer generation backed by RDKit ETKDGv3."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

PACK_ID = "org.linxira.chemistry-conformers-rdkit"
PACK_VERSION = "0.1.0"
CAPABILITY = "chemistry.conformers.v1"
EXPECTED_PYTHON = (3, 12)
EXPECTED_RDKIT = "2026.3.5"
EXPECTED_NUMPY = "2.5.2"

DEFAULT_NUM_CONFORMERS = 10
DEFAULT_SEED = 42


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
        raise RequestError("chemistry.conformers.v1 requires exactly one input artifact")
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
    # The worker contract resolves parameters.output_directory (and optional
    # output_filename) into the request before invoking the pack.
    output_directory = parameters.get("output_directory")
    if not isinstance(output_directory, str) or not output_directory:
        raise RequestError("parameters.output_directory is required")
    output_filename = parameters.get("output_filename")
    if not isinstance(output_filename, str) or not output_filename:
        output_filename = "conformers.sdf"
    return Path(output_directory) / output_filename


def options_from(request: dict[str, Any]) -> tuple[int, int, float | None]:
    parameters = request.get("parameters")
    if not isinstance(parameters, dict):
        raise RequestError("request parameters must be an object")
    num_conformers = parameters.get("num_conformers", DEFAULT_NUM_CONFORMERS)
    if not isinstance(num_conformers, int) or isinstance(num_conformers, bool):
        raise RequestError("num_conformers must be an integer")
    if not 1 <= num_conformers <= 1000:
        raise RequestError("num_conformers must be between 1 and 1000")
    seed = parameters.get("seed", DEFAULT_SEED)
    if not isinstance(seed, int) or isinstance(seed, bool) or seed < 0:
        raise RequestError("seed must be a non-negative integer")
    rms_prune = parameters.get("rms_prune_threshold")
    if rms_prune is not None:
        if not isinstance(rms_prune, (int, float)) or isinstance(rms_prune, bool):
            raise RequestError("rms_prune_threshold must be a number")
        rms_prune = float(rms_prune)
        if rms_prune <= 0.0:
            raise RequestError("rms_prune_threshold must be positive")
    return num_conformers, seed, rms_prune


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_sdf(mol_text: str) -> list[str]:
    """Parse minimal SDF records ($$$$ separated) into molecule texts."""
    records = []
    for record in mol_text.split("$$$$"):
        record = record.rstrip("\r\n")
        if record.strip():
            records.append(record)
    return records


def generate_conformers(
    molecule_text: str,
    num_conformers: int,
    seed: int,
    rms_prune: float | None,
) -> dict[str, Any]:
    """Embed one molecule with ETKDGv3 and minimise every conformer with MMFF94.

    Returns the hydrogenated molecule, the surviving conformer ids with their
    MMFF energies in kcal/mol, and the RDKit properties for provenance.
    """
    from rdkit import Chem
    from rdkit.Chem import AllChem

    mol = Chem.MolFromMolBlock(molecule_text)
    if mol is None:
        raise RequestError("RDKit could not parse the SDF molecule block")
    mol = Chem.AddHs(mol)
    params = AllChem.ETKDGv3()
    params.randomSeed = seed
    if rms_prune is not None:
        params.pruneRmsThresh = rms_prune
    conf_ids = list(AllChem.EmbedMultipleConfs(mol, numConfs=num_conformers, params=params))
    if not conf_ids:
        raise RequestError("ETKDGv3 produced no conformers for this molecule")
    if not AllChem.MMFFHasAllMoleculeParams(mol):
        raise RequestError("MMFF94 has no parameters for this molecule's atom types")
    optimised = AllChem.MMFFOptimizeMoleculeConfs(mol)
    energies = [float(energy) for _, energy in optimised]
    return {"molecule": mol, "conf_ids": conf_ids, "energies": energies}


def success_result(
    config: dict[str, Any],
    started_at: str,
    input_sha256: str,
    per_molecule: list[dict[str, Any]],
    total_conformers: int,
) -> dict[str, Any]:
    lock_path = Path(__file__).resolve().parents[1] / "requirements.lock"
    return {
        "schema_version": "2",
        "job_id": config["job_id"],
        "capability": CAPABILITY,
        "status": "ok",
        "result": {
            "molecule_count": len(per_molecule),
            "conformer_count": total_conformers,
            "per_molecule": per_molecule,
        },
        "artifacts": [
            {
                "artifact_id": "conformer-structures",
                "role": "conformers",
                "kind": "domain-file",
                "path": str(config["output_path"]),
                "format": "sdf",
                "media_type": "chemical/x-mdl-sdfile",
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
            "command": ["python", "src/conformers.py", "--request", "<request>", "--result", "<result>"],
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
    parser = argparse.ArgumentParser(description="Generate RDKit ETKDGv3 conformers")
    parser.add_argument("--request", required=True, type=Path, help="artifact-aware request JSON")
    parser.add_argument("--result", required=True, type=Path, help="machine-readable result JSON")
    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    options = parse_arguments(arguments)
    started_at = utc_now()
    request: dict[str, Any] = {}
    try:
        request = load_request(options.request)
        job_id = request.get("job_id")
        if not isinstance(job_id, str) or not job_id:
            raise RequestError("job_id is required")
        input_path = resolve_input(request)
        if not input_path.is_file():
            raise RequestError(f"input file does not exist: {input_path}")
        output_path = output_path_from(request)
        num_conformers, seed, rms_prune = options_from(request)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        mol_text = input_path.read_text(encoding="utf-8")
        records = parse_sdf(mol_text)
        if not records:
            raise RequestError("input SDF contains no molecule records")

        from rdkit import Chem

        per_molecule = []
        total_conformers = 0
        writer = Chem.SDWriter(str(output_path))
        try:
            for index, record in enumerate(records, start=1):
                embedded = generate_conformers(record, num_conformers, seed, rms_prune)
                mol = embedded["molecule"]
                conf_ids = embedded["conf_ids"]
                energies = embedded["energies"]
                for position, conf_id in enumerate(conf_ids, start=1):
                    mol.SetProp("_Name", f"molecule{index}_conf{position}")
                    mol.SetProp("_ConformerIndex", str(position))
                    mol.SetProp("mmff_energy_kcal_mol", f"{energies[position - 1]:.4f}")
                    writer.write(mol, confId=conf_id)
                total_conformers += len(conf_ids)
                per_molecule.append(
                    {
                        "molecule_index": index,
                        "conformer_count": len(conf_ids),
                        "energy_min_kcal_mol": round(min(energies), 4),
                        "energy_max_kcal_mol": round(max(energies), 4),
                    }
                )
        finally:
            writer.close()
        config = {"job_id": job_id, "output_path": output_path}
        payload = success_result(config, started_at, sha256_file(input_path), per_molecule, total_conformers)
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
