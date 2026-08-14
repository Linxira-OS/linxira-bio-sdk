#!/usr/bin/env python3
"""Materialize the complete transitive R/Bioconductor environment lock for a workflow pack.

Computes the recursive Depends/Imports/LinkingTo closure of the direct requirements
from an installed project-isolated R library, resolves canonical source tarball URLs
(CRAN src/contrib or Bioconductor 3.x src/contrib), downloads each tarball to pin
SHA-256, and writes the resolved_environment_lock entries into the pack lock file.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE_OR_RECOMMENDED = {"base", "recommended"}
DEP_FIELDS = ("Depends", "Imports", "LinkingTo")
PACKAGE_PATTERN = re.compile(r"([A-Za-z][A-Za-z0-9.]*)")


def parse_dcf(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    key: str | None = None
    value: str | None = None
    for raw in text.splitlines():
        if not raw.strip():
            key = None
            value = None
            continue
        if raw.startswith((" ", "\t")) and key is not None:
            value += "\n" + raw.strip()
            fields[key] = value
            continue
        name, _, body = raw.partition(":")
        key = name.strip()
        value = body.strip()
        fields[key] = value
    return fields


def parse_dependencies(raw: str) -> set[str]:
    if not raw:
        return set()
    # Strip version constraints and alternatives: "R (>= 4.6), methods, foo (>= 1)"
    dependencies: set[str] = set()
    for chunk in raw.split(","):
        chunk = chunk.strip()
        if not chunk:
            continue
        match = PACKAGE_PATTERN.match(chunk)
        if match:
            dependencies.add(match.group(1))
    return dependencies


def scan_library(library: Path) -> dict[str, dict[str, object]]:
    packages: dict[str, dict[str, object]] = {}
    if not library.is_dir():
        raise ValueError(f"R library does not exist: {library}")
    for directory in sorted(library.iterdir()):
        description = directory / "DESCRIPTION"
        if not description.is_file():
            continue
        fields = parse_dcf(description.read_text(encoding="utf-8", errors="replace"))
        name = fields.get("Package")
        if not name:
            continue
        depends = set()
        for field in DEP_FIELDS:
            depends |= parse_dependencies(fields.get(field, ""))
        packages[name] = {
            "version": fields.get("Version", ""),
            "license": fields.get("License", ""),
            "priority": fields.get("Priority", ""),
            "depends": depends,
        }
    return packages


def scan_runtime_priorities(runtime_library: Path | None = None) -> dict[str, str]:
    """Map base/recommended package names installed with the R runtime."""
    candidates: list[Path] = []
    if runtime_library is not None:
        candidates.append(runtime_library)
    r_home = os.environ.get("R_HOME")
    if r_home:
        candidates.append(Path(r_home) / "library")
    try:
        result = subprocess.run(
            ["R", "RHOME"], capture_output=True, text=True, timeout=30
        )
        if result.returncode == 0 and result.stdout.strip():
            candidates.append(Path(result.stdout.strip()) / "library")
    except (OSError, subprocess.SubprocessError):
        pass
    priorities: dict[str, str] = {}
    for library in candidates:
        if not library.is_dir():
            continue
        for directory in library.iterdir():
            description = directory / "DESCRIPTION"
            if not description.is_file():
                continue
            fields = parse_dcf(description.read_text(encoding="utf-8", errors="replace"))
            name = fields.get("Package")
            priority = fields.get("Priority", "")
            if name and priority:
                priorities[name] = priority
    return priorities


def compute_closure(
    roots: list[str],
    packages: dict[str, dict[str, object]],
    runtime_priorities: dict[str, str],
) -> list[str]:
    seen: set[str] = set()
    resolved: set[str] = set()
    queue = list(roots)
    while queue:
        package = queue.pop(0)
        if package in seen or package == "R":
            continue
        seen.add(package)
        priority = runtime_priorities.get(package, "")
        if priority.lower() in BASE_OR_RECOMMENDED:
            continue
        info = packages.get(package)
        if info is None:
            raise ValueError(
                f"dependency {package} is not installed in the project library "
                "and is not a base/recommended runtime package"
            )
        resolved.add(package)
        queue.extend(info["depends"] - seen)
    return sorted(resolved)


def fetch_text(url: str) -> str:
    request = urllib.request.Request(
        url, headers={"User-Agent": "linxira-bio-sdk-release/0.1.0"}
    )
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read().decode("utf-8", errors="replace")


def fetch_bytes(url: str) -> bytes:
    request = urllib.request.Request(
        url, headers={"User-Agent": "linxira-bio-sdk-release/0.1.0"}
    )
    with urllib.request.urlopen(request, timeout=120) as response:
        return response.read()


def bioc_package_repositories(release: str) -> dict[str, str]:
    """Map Bioc package name -> repository id for the given release."""
    mapping: dict[str, str] = {}
    repositories = {
        "bioconductor-bioc": "bioc",
        "bioconductor-data-annotation": "data/annotation",
        "bioconductor-data-experiment": "data/experiment",
    }
    for repository_id, repository_path in repositories.items():
        url = f"https://bioconductor.org/packages/{release}/{repository_path}/src/contrib/PACKAGES"
        text = fetch_text(url)
        current: str | None = None
        for line in text.splitlines():
            if line.startswith("Package:"):
                current = line.split(":", 1)[1].strip()
            elif line.startswith("Version:") and current is not None:
                mapping[current] = repository_id
        if current is None:
            raise ValueError(f"could not parse Bioc PACKAGES: {url}")
    return mapping


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def materialize(
    lock_path: Path, library: Path, runtime_library: Path | None
) -> list[dict[str, str]]:
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    direct_requirements = lock["direct_requirements"]
    roots = [requirement["name"] for requirement in direct_requirements]
    release = lock["runtime"]["bioconductor_release"]

    packages = scan_library(library)
    runtime_priorities = scan_runtime_priorities(runtime_library)
    closure = compute_closure(roots, packages, runtime_priorities)
    missing = [name for name in closure if name not in packages]
    if missing:
        raise ValueError(f"closure packages missing from scan: {missing}")

    print(f"closure size: {len(closure)} packages")
    bioc_repositories = bioc_package_repositories(release)
    print(f"Bioc 3.x package map: {len(bioc_repositories)} entries")

    entries: list[dict[str, str]] = []
    downloads: dict[str, bytes] = {}

    def resolve(package: str) -> dict[str, str]:
        info = packages[package]
        version = info["version"]
        repository_id = bioc_repositories.get(package, "cran")
        if repository_id.startswith("bioconductor"):
            repository_path = {
                "bioconductor-bioc": "bioc",
                "bioconductor-data-annotation": "data/annotation",
                "bioconductor-data-experiment": "data/experiment",
            }[repository_id]
            url = (
                f"https://bioconductor.org/packages/{release}/{repository_path}"
                f"/src/contrib/{package}_{version}.tar.gz"
            )
        else:
            url = f"https://cloud.r-project.org/src/contrib/{package}_{version}.tar.gz"
        return {
            "name": package,
            "version": version,
            "repository": repository_id,
            "source_url": url,
            "sha256": "",
            "license": info["license"],
        }

    def download(entry: dict[str, str]) -> dict[str, str]:
        try:
            data = fetch_bytes(entry["source_url"])
        except urllib.error.HTTPError:
            # Bioc-only packages installed from data repos use the soft repo URL space
            raise
        entry["sha256"] = sha256_bytes(data)
        downloads[entry["name"]] = data
        return entry

    resolved = [resolve(package) for package in closure]
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        entries = list(executor.map(download, resolved))

    for requirement in direct_requirements:
        name = requirement["name"]
        entry = next(item for item in entries if item["name"] == name)
        print(f"  {name} {entry['version']} {entry['source_url']} {entry['sha256'][:12]}")
    return entries


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lock", required=True, type=Path)
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--runtime-library", type=Path)
    arguments = parser.parse_args()

    lock_path = arguments.lock.resolve()
    library = arguments.library.resolve()
    runtime_library = (
        arguments.runtime_library.resolve() if arguments.runtime_library else None
    )
    entries = materialize(lock_path, library, runtime_library)
    if not entries:
        raise SystemExit("no lock entries materialized")

    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    lock["resolved_environment_lock"]["entries"] = entries
    lock["installable"] = True
    lock.pop("install_blocker", None)
    lock_path.write_text(
        json.dumps(lock, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote {len(entries)} resolved entries to {lock_path}")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        raise SystemExit(f"lock materialization failed: {error}") from error
