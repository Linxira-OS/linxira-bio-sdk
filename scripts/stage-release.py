#!/usr/bin/env python3
"""Stage one release bundle from the canonical repository documentation."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "packaging" / "bundle-manifest.json"


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--platform", choices=("windows", "debian", "arch"))
    parser.add_argument("--binary-dir", type=Path)
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    if not arguments.check and not all(
        (arguments.platform, arguments.binary_dir, arguments.output)
    ):
        parser.error("staging requires --platform, --binary-dir, and --output")
    return arguments


def load_manifest() -> dict[str, object]:
    with MANIFEST_PATH.open(encoding="utf-8") as handle:
        manifest = json.load(handle)
    if not isinstance(manifest, dict) or manifest.get("schema_version") != "1":
        raise ValueError("invalid release bundle manifest")
    return manifest


def repository_path(relative_path: str) -> Path:
    candidate = (ROOT / relative_path).resolve()
    if candidate != ROOT and ROOT not in candidate.parents:
        raise ValueError(f"bundle path leaves repository: {relative_path}")
    return candidate


def validate_sources(manifest: dict[str, object]) -> None:
    include_files = manifest.get("include_files")
    include_trees = manifest.get("include_trees")
    binaries = manifest.get("binaries")
    if not isinstance(include_files, list) or not isinstance(include_trees, list):
        raise ValueError("bundle manifest requires include_files and include_trees")
    if not isinstance(binaries, dict) or set(binaries) != {"windows", "debian", "arch"}:
        raise ValueError("bundle manifest requires Windows, Debian, and Arch binaries")

    required_files = {
        "Cargo.lock",
        "deny.toml",
        "licenses/NotoSansCJK-OFL.txt",
        "tools/catalog.json",
        "profiles/local-core.json",
    }
    if not required_files <= set(include_files):
        raise ValueError("release bundle lacks a required catalog, policy, or license file")
    required_trees = {"docs", "schemas", "skills", "workflows"}
    if not required_trees <= set(include_trees):
        raise ValueError("release bundle requires docs, schemas, skills, and workflows")

    for relative_path in include_files:
        if not isinstance(relative_path, str) or not repository_path(relative_path).is_file():
            raise ValueError(f"bundle file is missing: {relative_path}")
    for relative_path in include_trees:
        if not isinstance(relative_path, str) or not repository_path(relative_path).is_dir():
            raise ValueError(f"bundle tree is missing: {relative_path}")


def copy_sources(manifest: dict[str, object], staging_root: Path) -> None:
    for relative_path in manifest["include_files"]:
        source = repository_path(relative_path)
        destination = staging_root / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    for relative_path in manifest["include_trees"]:
        shutil.copytree(repository_path(relative_path), staging_root / relative_path)


def copy_binaries(
    manifest: dict[str, object], platform: str, binary_dir: Path, staging_root: Path
) -> None:
    for binary_name in manifest["binaries"][platform]:
        source = binary_dir / binary_name
        if not source.is_file():
            raise ValueError(f"release binary is missing: {source}")
        shutil.copy2(source, staging_root / binary_name)


def write_lock(staging_root: Path, platform: str) -> None:
    files = []
    for path in sorted(path for path in staging_root.rglob("*") if path.is_file()):
        files.append(
            {
                "path": path.relative_to(staging_root).as_posix(),
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
        )
    lock = {"schema_version": "1", "platform": platform, "files": files}
    with (staging_root / "bundle-manifest.lock.json").open("w", encoding="utf-8") as handle:
        json.dump(lock, handle, ensure_ascii=False, indent=2)
        handle.write("\n")


def main() -> None:
    arguments = parse_arguments()
    manifest = load_manifest()
    validate_sources(manifest)
    if arguments.check:
        print("release bundle sources are valid and include canonical documentation")
        return

    output = arguments.output.resolve()
    staging_root = output / manifest["artifact_root"]
    if staging_root.exists():
        raise ValueError(f"release staging root already exists: {staging_root}")
    staging_root.mkdir(parents=True)
    copy_sources(manifest, staging_root)
    copy_binaries(manifest, arguments.platform, arguments.binary_dir.resolve(), staging_root)
    write_lock(staging_root, arguments.platform)
    print(staging_root)


if __name__ == "__main__":
    main()
