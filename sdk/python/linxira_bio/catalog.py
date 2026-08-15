"""Capability catalog access."""

import json
import os
from pathlib import Path
from typing import Optional


def _default_catalog_path() -> Path:
    """Find the capabilities catalog.json relative to this package."""
    # sdk/python/linxira_bio/catalog.py -> ../../.. -> capabilities/catalog.json
    package_dir = Path(__file__).resolve().parent
    # Try relative to the package directory first
    candidate = package_dir.parent.parent.parent / "capabilities" / "catalog.json"
    if candidate.is_file():
        return candidate
    # Try the environment variable
    env_path = os.environ.get("LINXIRA_BIO_CATALOG")
    if env_path:
        env_candidate = Path(env_path)
        if env_candidate.is_file():
            return env_candidate
    # Fall back to the relative path (let it fail if missing)
    return candidate


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
    path = Path(catalog_path) if catalog_path else _default_catalog_path()
    if not path.is_file():
        raise FileNotFoundError(
            f"Capability catalog not found at {path}. "
            f"Set LINXIRA_BIO_CATALOG or provide catalog_path."
        )
    with open(path, "r", encoding="utf-8") as fh:
        catalog = json.load(fh)
    return catalog.get("capabilities", [])


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