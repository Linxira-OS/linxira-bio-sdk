#!/usr/bin/env python3
"""Recompute the file hashes of a workflow pack manifest.

Usage:
    python scripts/update-pack-manifest.py workflows/org.linxira.benchmark-python
    python scripts/update-pack-manifest.py --check workflows/org.linxira.benchmark-python

Every regular file under the pack directory except the manifest itself, byte-
code caches, build output (`target/`), and dot-files is listed in `files[]`
with its SHA-256; `runtime.dependency_lock.sha256` is synced to the lock file.
`--check` exits non-zero when the manifest on disk is stale, so CI can enforce
that a pack edit always ships with refreshed hashes.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

IGNORED_DIRECTORIES = {"__pycache__", "target", ".pytest_cache"}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tracked_files(pack_root: Path) -> list[Path]:
    files = []
    for path in sorted(pack_root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(pack_root)
        if relative.name == "manifest.json" and len(relative.parts) == 1:
            continue
        if any(part in IGNORED_DIRECTORIES or part.startswith(".") for part in relative.parts):
            continue
        files.append(relative)
    return files


def refreshed_manifest(pack_root: Path) -> dict:
    manifest_path = pack_root / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    hashes = {
        relative.as_posix(): sha256_file(pack_root / relative)
        for relative in tracked_files(pack_root)
    }
    # Keep the existing entry order so a refresh only touches what changed;
    # newly tracked files are appended in path order.
    previous_order = [
        entry["path"]
        for entry in manifest.get("files", [])
        if isinstance(entry, dict) and entry.get("path") in hashes
    ]
    ordered = previous_order + sorted(path for path in hashes if path not in previous_order)
    manifest["files"] = [{"path": path, "sha256": hashes[path]} for path in ordered]
    lock = manifest.get("runtime", {}).get("dependency_lock")
    if isinstance(lock, dict) and isinstance(lock.get("path"), str):
        lock_path = pack_root / lock["path"]
        if not lock_path.is_file():
            raise SystemExit(f"dependency lock is missing: {lock_path}")
        lock["sha256"] = sha256_file(lock_path)
    return manifest


def file_hashes(manifest: dict) -> dict[str, str]:
    return {
        entry["path"]: str(entry.get("sha256", "")).lower()
        for entry in manifest.get("files", [])
        if isinstance(entry, dict) and isinstance(entry.get("path"), str)
    }


def lock_hash(manifest: dict) -> str:
    lock = manifest.get("runtime", {}).get("dependency_lock", {})
    return str(lock.get("sha256", "")).lower() if isinstance(lock, dict) else ""


def is_stale(current: dict, desired: dict) -> bool:
    """Semantic comparison: formatting and entry order never count as stale."""
    return file_hashes(current) != file_hashes(desired) or lock_hash(current) != lock_hash(desired)


def render(manifest: dict) -> str:
    return json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("pack", type=Path, help="pack directory containing manifest.json")
    parser.add_argument(
        "--check", action="store_true", help="fail instead of writing when hashes are stale"
    )
    options = parser.parse_args(argv)
    pack_root = options.pack.resolve()
    manifest_path = pack_root / "manifest.json"
    if not manifest_path.is_file():
        print(f"no manifest.json in {pack_root}", file=sys.stderr)
        return 2
    current = json.loads(manifest_path.read_text(encoding="utf-8"))
    desired = refreshed_manifest(pack_root)
    if not is_stale(current, desired):
        print(f"{manifest_path}: up to date ({len(desired['files'])} files)")
        return 0
    if options.check:
        changed = sorted(
            path
            for path in set(file_hashes(current)) | set(file_hashes(desired))
            if file_hashes(current).get(path) != file_hashes(desired).get(path)
        )
        print(
            f"{manifest_path}: file hashes are stale ({', '.join(changed) or 'dependency lock'}); "
            "run scripts/update-pack-manifest.py without --check",
            file=sys.stderr,
        )
        return 1
    manifest_path.write_text(render(desired), encoding="utf-8")
    print(f"{manifest_path}: refreshed ({len(desired['files'])} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
