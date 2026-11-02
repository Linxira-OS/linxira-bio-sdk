"""Capability catalog access."""

import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Optional


def _default_catalog_path() -> Path:
    """Find the capabilities catalog.json.

    Search order:
    1. ``LINXIRA_BIO_CATALOG``
    2. ``linxira_bio/data/catalog.json`` packaged with the SDK
    3. The repository layout used during development
    4. ``linxira-bio capabilities --json`` when the native CLI is installed
    """
    env_path = os.environ.get("LINXIRA_BIO_CATALOG")
    if env_path:
        env_candidate = Path(env_path)
        if env_candidate.is_file():
            return env_candidate

    package_dir = Path(__file__).resolve().parent
    packaged = package_dir / "data" / "catalog.json"
    if packaged.is_file():
        return packaged

    # sdk/python/linxira_bio/catalog.py -> ../../.. -> capabilities/catalog.json
    return package_dir.parent.parent.parent / "capabilities" / "catalog.json"


def _catalog_from_cli() -> Optional[list]:
    """Load the catalog from an installed ``linxira-bio`` executable."""
    candidates = []
    configured = os.environ.get("LINXIRA_BIO_CLI")
    if configured:
        candidates.append(configured)
    found = shutil.which("linxira-bio")
    if found:
        candidates.append(found)
    worker = os.environ.get("LINXIRA_BIO_WORKER")
    if worker:
        worker_path = Path(worker)
        cli_name = "linxira-bio.exe" if worker_path.suffix.lower() == ".exe" else "linxira-bio"
        candidates.append(str(worker_path.with_name(cli_name)))

    for cli in candidates:
        try:
            process = subprocess.run(
                [cli, "capabilities", "--json"],
                capture_output=True,
                text=True,
                timeout=30,
            )
        except (OSError, subprocess.TimeoutExpired):
            continue
        if process.returncode != 0 or not process.stdout.strip():
            continue
        try:
            payload = json.loads(process.stdout)
        except json.JSONDecodeError:
            continue
        capabilities = payload.get("capabilities")
        if isinstance(capabilities, list):
            return capabilities
    return None


def get_catalog(catalog_path: Optional[str] = None) -> list:
    """Load the capability catalog.

    Returns the list of capability entries from the catalog.
    Each entry is a dict with keys: id, status, category, command, etc.

    Args:
        catalog_path: Optional path to catalog.json. If not provided,
                      auto-discovers the default catalog.

    Returns:
        List of capability dicts.
    """
    if catalog_path:
        path = Path(catalog_path)
        if not path.is_file():
            raise FileNotFoundError(f"Capability catalog not found at {path}.")
        with open(path, "r", encoding="utf-8") as fh:
            catalog = json.load(fh)
        return catalog.get("capabilities", [])

    path = _default_catalog_path()
    if path.is_file():
        with open(path, "r", encoding="utf-8") as fh:
            catalog = json.load(fh)
        return catalog.get("capabilities", [])

    runtime_catalog = _catalog_from_cli()
    if runtime_catalog is not None:
        return runtime_catalog

    raise FileNotFoundError(
        f"Capability catalog not found at {path}. "
        f"Set LINXIRA_BIO_CATALOG, put linxira-bio on PATH, or provide catalog_path."
    )


def list_capabilities(
    category: Optional[str] = None,
    status: Optional[str] = None,
    catalog_path: Optional[str] = None,
) -> list:
    """List available capabilities, optionally filtered by category and/or status.

    Args:
        category: Optional category filter (e.g. "sequence-io", "read-qc").
        status: Optional status filter (e.g. "available", "planned").
                Defaults to "available" if not specified.
        catalog_path: Optional path to catalog.json.

    Returns:
        List of capability dicts matching the filters.
    """
    capabilities = get_catalog(catalog_path)
    filtered = []
    for cap in capabilities:
        if status is not None and cap.get("status") != status:
            continue
        if status is None and cap.get("status") != "available":
            continue
        if category is not None and cap.get("category") != category:
            continue
        filtered.append(cap)
    return filtered


def get_capability(capability_id: str, catalog_path: Optional[str] = None) -> Optional[dict]:
    """Look up a single capability by ID.

    Args:
        capability_id: The capability ID (e.g. "sequence.stats.v1").
        catalog_path: Optional path to catalog.json.

    Returns:
        The capability dict, or None if not found.
    """
    for cap in get_catalog(catalog_path):
        if cap.get("id") == capability_id:
            return cap
    return None