#!/usr/bin/env python3
"""Generate strict, target-specific Cargo dependency notices for releases."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import re
import subprocess
import tarfile
import tomllib
from collections import defaultdict
from pathlib import Path
from pathlib import PurePosixPath
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
BUNDLE_MANIFEST_PATH = ROOT / "packaging" / "bundle-manifest.json"
OVERRIDES_PATH = ROOT / "licenses" / "cargo-overrides.json"
REPORT_SCHEMA_PATH = ROOT / "schemas" / "third-party-dependencies.schema.json"
GENERATOR_VERSION = "1"
JSON_OUTPUT_NAME = "THIRD_PARTY_DEPENDENCIES.json"
TEXT_OUTPUT_NAME = "THIRD_PARTY_DEPENDENCIES.txt"
SUPPORTED_PLATFORMS = ("windows", "debian", "arch")
TEXT_FILE_SUFFIXES = {"", ".html", ".md", ".markdown", ".rst", ".txt"}
NON_NOTICE_SUFFIXES = {
    ".c",
    ".cc",
    ".cpp",
    ".h",
    ".hpp",
    ".json",
    ".lock",
    ".rs",
    ".toml",
    ".xml",
    ".yaml",
    ".yml",
}
NOTICE_NAME = re.compile(
    r"^(?:licen[cs]e|copying|copyright|notice|ofl|ufl)(?:[-_.].*)?$",
    re.IGNORECASE,
)
SPDX_TOKEN = re.compile(r"[A-Za-z0-9][A-Za-z0-9.+-]*")
SPDX_OPERATORS = {"AND", "OR", "WITH"}
SPDX_TEXT_MARKERS = {
    "0BSD": (
        "permission to use, copy, modify, and/or distribute this software for "
        "any purpose with or without fee is hereby granted",
    ),
    "Apache-2.0": ("apache license", "version 2.0, january 2004"),
    "BSD-2-Clause": ("redistribution and use in source and binary forms",),
    "BSD-3-Clause": ("redistribution and use in source and binary forms",),
    "BSL-1.0": ("boost software license - version 1.0",),
    "CC0-1.0": ("cc0 1.0 universal",),
    "GPL-2.0-only": ("gnu general public license", "version 2, june 1991"),
    "ISC": ("permission to use, copy, modify, and/or distribute this software",),
    "LLVM-exception": ("llvm exceptions to the apache 2.0 license",),
    "MIT": ("permission is hereby granted, free of charge",),
    "OFL-1.1": ("sil open font license version 1.1",),
    "Ubuntu-font-1.0": ("ubuntu font licence",),
    "Unicode-3.0": ("unicode license v3", "unicode, inc. license agreement"),
    "Unlicense": ("this is free and unencumbered software released into the public domain",),
    "Zlib": ("this software is provided 'as-is', without any express or implied warranty",),
}
MAX_NOTICE_BYTES = 2 * 1024 * 1024


class NoticeError(ValueError):
    """A release notice cannot be generated without manual review."""


class UnresolvedNoticePointer(NoticeError):
    """A crate packaged a path placeholder instead of the referenced text."""


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--platform", choices=SUPPORTED_PLATFORMS)
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--offline",
        action="store_true",
        help="pass --offline to cargo metadata after dependencies have been fetched",
    )
    parser.add_argument(
        "--check-config",
        action="store_true",
        help="validate pinned override documents without resolving Cargo metadata",
    )
    arguments = parser.parse_args()
    if not arguments.check_config and not (arguments.platform and arguments.output):
        parser.error("generation requires --platform and --output")
    return arguments


def load_json(path: Path) -> Any:
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise NoticeError(f"cannot read JSON file {path}: {error}") from error


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def cargo_lock_registry_checksums(
    content: bytes,
) -> dict[tuple[str, str, str], str]:
    try:
        payload = tomllib.loads(content.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise NoticeError(f"cannot parse Cargo.lock: {error}") from error
    packages = payload.get("package")
    if not isinstance(packages, list):
        raise NoticeError("Cargo.lock lacks package entries")

    checksums: dict[tuple[str, str, str], str] = {}
    for index, package in enumerate(packages):
        if not isinstance(package, dict):
            raise NoticeError(f"Cargo.lock package {index} is invalid")
        source = package.get("source")
        if not isinstance(source, str) or not source.startswith("registry+"):
            continue
        name = package.get("name")
        version = package.get("version")
        checksum = package.get("checksum")
        if not isinstance(name, str) or not name:
            raise NoticeError(f"Cargo.lock registry package {index} has invalid name")
        if not isinstance(version, str) or not version:
            raise NoticeError(
                f"Cargo.lock registry package {index} has invalid version"
            )
        if not isinstance(checksum, str) or not re.fullmatch(
            r"[0-9a-f]{64}", checksum
        ):
            raise NoticeError(
                f"Cargo.lock registry package {name} {version} has invalid checksum"
            )
        key = (name, version, source)
        if key in checksums:
            raise NoticeError(
                f"Cargo.lock repeats registry package {name} {version} from {source}"
            )
        checksums[key] = checksum
    if not checksums:
        raise NoticeError("Cargo.lock contains no registry package checksums")
    return checksums


def repository_path(relative_path: str) -> Path:
    if not isinstance(relative_path, str) or not relative_path:
        raise NoticeError("notice document path must be a non-empty string")
    candidate = (ROOT / relative_path).resolve()
    if not path_is_at_or_inside(candidate, ROOT):
        raise NoticeError(f"notice document leaves repository: {relative_path}")
    return candidate


def paths_refer_to_same_existing_file(left: Path, right: Path) -> bool:
    try:
        return left.samefile(right)
    except OSError:
        return False


def path_is_at_or_inside(candidate: Path, root: Path) -> bool:
    try:
        candidate.relative_to(root)
        return True
    except ValueError:
        pass
    if paths_refer_to_same_existing_file(candidate, root):
        return True
    return any(paths_refer_to_same_existing_file(parent, root) for parent in candidate.parents)


def relative_to_existing_root(candidate: Path, root: Path) -> Path:
    try:
        return candidate.relative_to(root)
    except ValueError:
        pass
    if paths_refer_to_same_existing_file(candidate, root):
        return Path()
    for parent in candidate.parents:
        if paths_refer_to_same_existing_file(parent, root):
            return candidate.relative_to(parent)
    raise ValueError(f"{candidate!s} is not in the subpath of {root!s}")


def decode_notice(content: bytes, origin: object) -> tuple[bytes, str]:
    if not content or len(content) > MAX_NOTICE_BYTES or b"\0" in content:
        raise NoticeError(f"invalid license or notice document: {origin}")
    try:
        text = content.decode("utf-8")
    except UnicodeDecodeError as error:
        raise NoticeError(
            f"license or notice document is not UTF-8: {origin}"
        ) from error
    if not text.strip():
        raise NoticeError(f"empty license or notice document: {origin}")
    stripped = text.strip()
    if re.fullmatch(r"(?:\.\.?[/\\])+[A-Za-z0-9_. /\\-]+", stripped):
        raise UnresolvedNoticePointer(
            f"license or notice document is only an unresolved path pointer: {origin}: "
            f"{stripped!r}"
        )
    return content, text


def read_notice(path: Path) -> tuple[bytes, str]:
    if not path.is_file():
        raise NoticeError(f"license or notice document is missing: {path}")
    return decode_notice(path.read_bytes(), path)


def load_bundle_notice_configuration() -> dict[str, object]:
    manifest = load_json(BUNDLE_MANIFEST_PATH)
    if not isinstance(manifest, dict):
        raise NoticeError("release bundle manifest must contain an object")
    configuration = manifest.get("dependency_notices")
    if not isinstance(configuration, dict):
        raise NoticeError("release bundle manifest lacks dependency_notices")
    if set(configuration) != {"roots", "targets", "overrides", "outputs"}:
        raise NoticeError("dependency_notices has unexpected or missing keys")

    roots = configuration.get("roots")
    if (
        not isinstance(roots, list)
        or not roots
        or any(not isinstance(root, str) or not root for root in roots)
        or len(roots) != len(set(roots))
    ):
        raise NoticeError("dependency_notices.roots must be unique package names")

    targets = configuration.get("targets")
    if (
        not isinstance(targets, dict)
        or set(targets) != set(SUPPORTED_PLATFORMS)
        or any(not isinstance(value, str) or not value for value in targets.values())
    ):
        raise NoticeError("dependency_notices.targets must map every release platform")

    if configuration.get("overrides") != OVERRIDES_PATH.relative_to(ROOT).as_posix():
        raise NoticeError("dependency_notices.overrides must use the canonical path")
    outputs = configuration.get("outputs")
    if outputs != {"json": JSON_OUTPUT_NAME, "text": TEXT_OUTPUT_NAME}:
        raise NoticeError("dependency_notices.outputs must use the canonical filenames")
    return configuration


def validate_override_configuration() -> list[dict[str, object]]:
    payload = load_json(OVERRIDES_PATH)
    if not isinstance(payload, dict) or set(payload) != {"schema_version", "entries"}:
        raise NoticeError("cargo override manifest has unexpected or missing keys")
    if payload.get("schema_version") != "1":
        raise NoticeError("unsupported cargo override manifest schema")
    entries = payload.get("entries")
    if not isinstance(entries, list):
        raise NoticeError("cargo override manifest entries must be an array")

    coverage: set[tuple[str, str, str]] = set()
    referenced_paths: set[Path] = set()
    validated: list[dict[str, object]] = []
    override_root = (ROOT / "licenses" / "cargo-overrides").resolve()
    required_entry_keys = {
        "package",
        "version",
        "source",
        "license_expression",
        "vcs_revision",
        "platforms",
        "reason",
        "documents",
    }
    required_document_keys = {"path", "sha256", "source_url"}

    for index, entry in enumerate(entries):
        if not isinstance(entry, dict) or set(entry) != required_entry_keys:
            raise NoticeError(f"override entry {index} has unexpected or missing keys")
        for field in (
            "package",
            "version",
            "source",
            "license_expression",
            "vcs_revision",
            "reason",
        ):
            if not isinstance(entry.get(field), str) or not entry[field].strip():
                raise NoticeError(f"override entry {index} has invalid {field}")
        if not re.fullmatch(r"[0-9a-f]{40}", str(entry["vcs_revision"])):
            raise NoticeError(f"override entry {index} has invalid vcs_revision")

        platforms = entry.get("platforms")
        if (
            not isinstance(platforms, list)
            or not platforms
            or len(platforms) != len(set(platforms))
            or any(platform not in SUPPORTED_PLATFORMS for platform in platforms)
        ):
            raise NoticeError(f"override entry {index} has invalid platforms")
        for platform in platforms:
            key = (str(entry["package"]), str(entry["version"]), platform)
            if key in coverage:
                raise NoticeError(f"duplicate override coverage for {key}")
            coverage.add(key)

        documents = entry.get("documents")
        if not isinstance(documents, list) or not documents:
            raise NoticeError(f"override entry {index} requires documents")
        document_urls: list[str] = []
        for document_index, document in enumerate(documents):
            if not isinstance(document, dict) or set(document) != required_document_keys:
                raise NoticeError(
                    f"override entry {index} document {document_index} is invalid"
                )
            relative_path = document.get("path")
            if not isinstance(relative_path, str) or not relative_path.startswith(
                "licenses/cargo-overrides/"
            ):
                raise NoticeError(f"override document has invalid path: {relative_path}")
            path = repository_path(relative_path)
            if not path_is_at_or_inside(path, override_root):
                raise NoticeError(
                    f"override document leaves licenses/cargo-overrides: {relative_path}"
                )
            content, _ = read_notice(path)
            expected_hash = document.get("sha256")
            if not isinstance(expected_hash, str) or not re.fullmatch(
                r"[0-9a-f]{64}", expected_hash
            ):
                raise NoticeError(f"override document has invalid SHA-256: {relative_path}")
            actual_hash = sha256_bytes(content)
            if actual_hash != expected_hash:
                raise NoticeError(
                    f"override document SHA-256 mismatch: {relative_path}: {actual_hash}"
                )
            source_url = document.get("source_url")
            if not isinstance(source_url, str) or not source_url.startswith("https://"):
                raise NoticeError(f"override document has invalid source URL: {relative_path}")
            document_urls.append(source_url)
            referenced_paths.add(path)
        revision = str(entry["vcs_revision"])
        if any(revision not in source_url for source_url in document_urls):
            reason = str(entry["reason"]).lower()
            pinned_fallback = all(
                re.search(r"/[0-9a-f]{40}/", source_url) for source_url in document_urls
            )
            if "canonical" not in reason or not pinned_fallback:
                raise NoticeError(
                    f"override entry {index} is not pinned to its Cargo VCS revision "
                    "and lacks an explicit canonical fixed-revision fallback"
                )
        validated.append(entry)

    tracked_documents = {path.resolve() for path in override_root.rglob("*") if path.is_file()}
    unreferenced = sorted(tracked_documents - referenced_paths)
    if unreferenced:
        paths = ", ".join(path.relative_to(ROOT).as_posix() for path in unreferenced)
        raise NoticeError(f"unreferenced cargo override documents: {paths}")
    return validated


def cargo_metadata(target: str, *, offline: bool) -> dict[str, object]:
    command = [
        "cargo",
        "metadata",
        "--locked",
        "--format-version",
        "1",
        "--filter-platform",
        target,
    ]
    if offline:
        command.append("--offline")
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        details = result.stderr.strip() or result.stdout.strip()
        raise NoticeError(f"cargo metadata failed for {target}: {details}")
    try:
        metadata = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise NoticeError("cargo metadata returned invalid JSON") from error
    if not isinstance(metadata, dict) or metadata.get("version") != 1:
        raise NoticeError("cargo metadata returned an unsupported document")
    return metadata


def cargo_version() -> str:
    result = subprocess.run(
        ["cargo", "--version"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    version = result.stdout.strip()
    if result.returncode != 0 or not version.startswith("cargo "):
        details = result.stderr.strip() or version
        raise NoticeError(f"cannot identify Cargo version: {details}")
    return version


def parse_cargo_tree(output: str) -> dict[tuple[str, str], list[str]]:
    selected: dict[tuple[str, str], set[str]] = defaultdict(set)
    for line_number, raw_line in enumerate(output.splitlines(), start=1):
        line = re.sub(r" \(\*\)$", "", raw_line.strip())
        if not line:
            continue
        package_display, separator, feature_display = line.partition("|")
        if not separator:
            raise NoticeError(f"cargo tree line {line_number} lacks a feature separator")
        match = re.match(r"^([A-Za-z0-9_-]+) v([^ ]+)(?: .*)?$", package_display)
        if match is None:
            raise NoticeError(f"cannot parse cargo tree line {line_number}: {raw_line!r}")
        identity = (match.group(1), match.group(2))
        selected[identity].update(
            feature for feature in feature_display.split(",") if feature
        )
    if not selected:
        raise NoticeError("cargo tree returned an empty release dependency graph")
    return {identity: sorted(features) for identity, features in selected.items()}


def cargo_release_tree(
    target: str, root_names: list[str], *, offline: bool
) -> dict[tuple[str, str], list[str]]:
    command = [
        "cargo",
        "tree",
        "--locked",
        "--target",
        target,
        "--edges",
        "normal,build",
        "--prefix",
        "none",
        "--format",
        "{p}|{f}",
    ]
    if offline:
        command.append("--offline")
    for root_name in root_names:
        command.extend(("--package", root_name))
    result = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        details = result.stderr.strip() or result.stdout.strip()
        raise NoticeError(f"cargo tree failed for {target}: {details}")
    return parse_cargo_tree(result.stdout)


def release_closure(
    metadata: dict[str, object],
    root_names: list[str],
    selected_packages: dict[tuple[str, str], list[str]],
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
    packages_value = metadata.get("packages")
    workspace_members = metadata.get("workspace_members")
    if not isinstance(packages_value, list) or not isinstance(workspace_members, list):
        raise NoticeError("cargo metadata lacks packages or workspace_members")
    packages = [
        package
        for package in packages_value
        if isinstance(package, dict) and isinstance(package.get("id"), str)
    ]
    workspace_ids = set(workspace_members)
    packages_by_identity: dict[tuple[str, str], list[dict[str, object]]] = defaultdict(list)
    for package in packages:
        packages_by_identity[(str(package.get("name")), str(package.get("version")))].append(
            package
        )

    selected: list[dict[str, object]] = []
    for identity, features in selected_packages.items():
        matches = packages_by_identity.get(identity, [])
        if len(matches) != 1:
            raise NoticeError(
                f"cargo tree identity {identity[0]} {identity[1]} maps to "
                f"{len(matches)} metadata packages; source disambiguation is required"
            )
        package = dict(matches[0])
        package["active_features"] = features
        selected.append(package)

    roots: list[dict[str, object]] = []
    for root_name in root_names:
        matches = [
            package
            for package in selected
            if package["id"] in workspace_ids and package.get("name") == root_name
        ]
        if len(matches) != 1:
            raise NoticeError(f"release root {root_name!r} did not resolve exactly once")
        root = matches[0]
        roots.append({"name": root["name"], "version": root["version"]})

    external: list[dict[str, object]] = []
    for package in selected:
        package_id = package["id"]
        if package_id in workspace_ids:
            continue
        if package.get("source") is None:
            raise NoticeError(
                f"non-workspace path dependency requires an explicit release policy: {package_id}"
            )
        external.append(package)
    external.sort(key=lambda package: (package["name"], package["version"], package["source"]))
    roots.sort(key=lambda root: str(root["name"]))
    return roots, external


def is_notice_relative_path(relative: PurePosixPath) -> bool:
    suffix = relative.suffix.lower()
    if suffix in NON_NOTICE_SUFFIXES:
        return False
    if NOTICE_NAME.fullmatch(relative.name):
        return True
    if suffix not in TEXT_FILE_SUFFIXES:
        return False
    if "license" in relative.name.lower() or "licence" in relative.name.lower():
        return True
    return any(part.lower() in {"license", "licenses"} for part in relative.parts[:-1])


def is_notice_path(path: Path, package_root: Path) -> bool:
    relative = PurePosixPath(relative_to_existing_root(path, package_root).as_posix())
    return is_notice_relative_path(relative)


def parse_package_vcs(content: bytes, origin: object) -> dict[str, str]:
    try:
        payload = json.loads(content.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise NoticeError(f"invalid Cargo VCS metadata: {origin}") from error
    if not isinstance(payload, dict) or not isinstance(payload.get("git"), dict):
        raise NoticeError(f"invalid Cargo VCS metadata: {origin}")
    revision = payload["git"].get("sha1")
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise NoticeError(f"invalid Cargo VCS revision: {origin}")
    result = {"revision": revision}
    vcs_path = payload.get("path_in_vcs")
    if isinstance(vcs_path, str):
        result["path"] = vcs_path
    return result


def package_vcs(package_root: Path) -> dict[str, str] | None:
    path = package_root / ".cargo_vcs_info.json"
    if not path.is_file():
        return None
    return parse_package_vcs(path.read_bytes(), path)


def registry_archive_path(package: dict[str, object]) -> Path:
    name = str(package["name"])
    version = str(package["version"])
    archive_stem = f"{name}-{version}"
    if (
        not re.fullmatch(r"[A-Za-z0-9_-]+", name)
        or not re.fullmatch(r"[A-Za-z0-9.+_-]+", version)
    ):
        raise NoticeError(f"invalid registry package identity: {name} {version}")

    package_root = Path(str(package["manifest_path"])).resolve().parent
    registry_partition = package_root.parent
    registry_src = registry_partition.parent
    registry_root = registry_src.parent
    if (
        package_root.name != archive_stem
        or registry_src.name != "src"
        or registry_root.name != "registry"
        or not registry_partition.name
    ):
        raise NoticeError(
            f"registry package has an unexpected Cargo source path: "
            f"{name} {version}: {package_root}"
        )
    return registry_root / "cache" / registry_partition.name / f"{archive_stem}.crate"


def archive_member_relative_path(
    member_name: str, expected_root: str, archive_path: Path
) -> PurePosixPath | None:
    if (
        not member_name
        or member_name.startswith("/")
        or "\\" in member_name
        or any(part in {"", ".", ".."} for part in member_name.split("/"))
    ):
        raise NoticeError(f"unsafe member path in verified crate archive {archive_path}")
    parts = member_name.split("/")
    if parts[0] != expected_root:
        raise NoticeError(
            f"member leaves package root in verified crate archive {archive_path}: "
            f"{member_name}"
        )
    if len(parts) == 1:
        return None
    return PurePosixPath(*parts[1:])


def archive_license_file_path(
    package: dict[str, object], package_root: Path
) -> PurePosixPath | None:
    license_file = package.get("license_file")
    if not isinstance(license_file, str) or not license_file:
        return None
    candidate = Path(license_file)
    if candidate.is_absolute():
        try:
            candidate = relative_to_existing_root(candidate.resolve(), package_root.resolve())
        except ValueError as error:
            raise NoticeError(
                f"package license_file leaves package root: {package['id']}"
            ) from error
    relative = PurePosixPath(candidate.as_posix())
    if relative.is_absolute() or not relative.parts or any(
        part in {"", ".", ".."} for part in relative.parts
    ):
        raise NoticeError(f"package license_file leaves package root: {package['id']}")
    return relative


def registry_package_evidence(
    package: dict[str, object], expected_checksum: str
) -> tuple[list[dict[str, str]], list[str], dict[str, str] | None]:
    archive_path = registry_archive_path(package)
    try:
        archive_content = archive_path.read_bytes()
    except OSError as error:
        raise NoticeError(
            f"cannot read locked crate archive for {package['name']} "
            f"{package['version']}: {archive_path}: {error}"
        ) from error
    actual_checksum = sha256_bytes(archive_content)
    if actual_checksum != expected_checksum:
        raise NoticeError(
            f"locked crate archive SHA-256 mismatch for {package['name']} "
            f"{package['version']}: expected {expected_checksum}, got {actual_checksum}"
        )

    expected_root = f"{package['name']}-{package['version']}"
    try:
        with tarfile.open(fileobj=io.BytesIO(archive_content), mode="r:*") as archive:
            members: dict[str, tarfile.TarInfo] = {}
            for member in archive.getmembers():
                relative = archive_member_relative_path(
                    member.name, expected_root, archive_path
                )
                if relative is None:
                    continue
                relative_path = relative.as_posix()
                if relative_path in members:
                    raise NoticeError(
                        f"duplicate member in verified crate archive {archive_path}: "
                        f"{relative_path}"
                    )
                members[relative_path] = member

            package_root = Path(str(package["manifest_path"])).resolve().parent
            candidates = {
                PurePosixPath(path)
                for path, member in members.items()
                if member.isfile() and is_notice_relative_path(PurePosixPath(path))
            }
            license_file = archive_license_file_path(package, package_root)
            if license_file is not None:
                candidates.add(license_file)

            documents: list[dict[str, str]] = []
            unresolved_pointers: list[str] = []
            for relative in sorted(candidates, key=lambda item: item.as_posix()):
                relative_path = relative.as_posix()
                member = members.get(relative_path)
                if member is None or not member.isfile():
                    raise NoticeError(
                        f"package license_file is missing from verified crate archive: "
                        f"{package['name']} {package['version']}: {relative_path}"
                    )
                if member.size > MAX_NOTICE_BYTES:
                    raise NoticeError(
                        f"invalid license or notice document in {archive_path}: "
                        f"{relative_path}"
                    )
                extracted = archive.extractfile(member)
                if extracted is None:
                    raise NoticeError(
                        f"cannot read verified crate archive member: {archive_path}: "
                        f"{relative_path}"
                    )
                content = extracted.read(MAX_NOTICE_BYTES + 1)
                try:
                    content, text = decode_notice(
                        content, f"{archive_path}!/{relative_path}"
                    )
                except UnresolvedNoticePointer:
                    unresolved_pointers.append(relative_path)
                    continue
                documents.append(
                    {
                        "origin": "crate-package",
                        "path": relative_path,
                        "sha256": sha256_bytes(content),
                        "source_url": (
                            f"https://crates.io/crates/{package['name']}/"
                            f"{package['version']}"
                        ),
                        "text": text,
                    }
                )

            vcs: dict[str, str] | None = None
            vcs_member = members.get(".cargo_vcs_info.json")
            if vcs_member is not None:
                if not vcs_member.isfile() or vcs_member.size > MAX_NOTICE_BYTES:
                    raise NoticeError(
                        f"invalid Cargo VCS metadata in verified crate archive: "
                        f"{archive_path}"
                    )
                extracted = archive.extractfile(vcs_member)
                if extracted is None:
                    raise NoticeError(
                        f"cannot read Cargo VCS metadata in verified crate archive: "
                        f"{archive_path}"
                    )
                vcs = parse_package_vcs(
                    extracted.read(MAX_NOTICE_BYTES + 1),
                    f"{archive_path}!/.cargo_vcs_info.json",
                )
    except (tarfile.TarError, OSError, EOFError) as error:
        raise NoticeError(f"cannot read verified crate archive {archive_path}: {error}") from error
    return documents, unresolved_pointers, vcs


def discover_package_documents(
    package: dict[str, object],
) -> tuple[list[dict[str, str]], list[str]]:
    manifest_path = Path(str(package["manifest_path"])).resolve()
    package_root = manifest_path.parent
    candidates: set[Path] = set()
    license_file = package.get("license_file")
    if isinstance(license_file, str) and license_file:
        candidate = Path(license_file)
        if not candidate.is_absolute():
            candidate = package_root / candidate
        candidate = candidate.resolve()
        if candidate != package_root and package_root not in candidate.parents:
            raise NoticeError(f"package license_file leaves package root: {package['id']}")
        candidates.add(candidate)
    for candidate in package_root.rglob("*"):
        if candidate.is_file() and is_notice_path(candidate, package_root):
            candidates.add(candidate.resolve())

    documents: list[dict[str, str]] = []
    unresolved_pointers: list[str] = []
    for path in sorted(
        candidates, key=lambda item: relative_to_existing_root(item, package_root).as_posix()
    ):
        relative_path = relative_to_existing_root(path, package_root).as_posix()
        try:
            content, text = read_notice(path)
        except UnresolvedNoticePointer:
            unresolved_pointers.append(relative_path)
            continue
        documents.append(
            {
                "origin": "crate-package",
                "path": relative_path,
                "sha256": sha256_bytes(content),
                "source_url": f"https://crates.io/crates/{package['name']}/{package['version']}",
                "text": text,
            }
        )
    return documents, unresolved_pointers


def collect_package_evidence(
    package: dict[str, object],
    registry_checksums: dict[tuple[str, str, str], str],
) -> tuple[list[dict[str, str]], list[str], dict[str, str] | None]:
    source = str(package["source"])
    if source.startswith("registry+"):
        key = (str(package["name"]), str(package["version"]), source)
        checksum = registry_checksums.get(key)
        if checksum is None:
            raise NoticeError(
                f"release registry package is absent from Cargo.lock: "
                f"{package['name']} {package['version']} from {source}"
            )
        return registry_package_evidence(package, checksum)
    package_root = Path(str(package["manifest_path"])).resolve().parent
    documents, unresolved_pointers = discover_package_documents(package)
    return documents, unresolved_pointers, package_vcs(package_root)


def expression_identifiers(expression: str) -> set[str]:
    return {
        token
        for token in SPDX_TOKEN.findall(expression)
        if token.upper() not in SPDX_OPERATORS
    }


def document_text_matches(identifier: str, text: str) -> bool:
    normalized = re.sub(r"\s+", " ", text.casefold()).strip()
    isc_condition = (
        "provided that the above copyright notice and this permission notice appear"
    )
    if identifier == "0BSD":
        return SPDX_TEXT_MARKERS[identifier][0] in normalized and isc_condition not in normalized
    if identifier == "Apache-2.0":
        return all(marker in normalized for marker in SPDX_TEXT_MARKERS[identifier])
    if identifier == "BSD-2-Clause":
        return (
            "redistribution and use in source and binary forms" in normalized
            and "neither the name" not in normalized
        )
    if identifier == "BSD-3-Clause":
        return (
            "redistribution and use in source and binary forms" in normalized
            and "neither the name" in normalized
        )
    if identifier == "GPL-2.0-only":
        return all(marker in normalized for marker in SPDX_TEXT_MARKERS[identifier])
    if identifier == "ISC":
        return SPDX_TEXT_MARKERS[identifier][0] in normalized and isc_condition in normalized
    markers = SPDX_TEXT_MARKERS.get(identifier)
    if markers is None:
        return False
    return any(marker in normalized for marker in markers)


def assert_document_coverage(
    package: dict[str, object], documents: list[dict[str, str]]
) -> None:
    expression = package.get("license")
    license_file = package.get("license_file")
    compound = False
    identifiers: set[str] = set()
    if not isinstance(expression, str) or not expression.strip():
        if not isinstance(license_file, str) or not license_file:
            raise NoticeError(
                f"dependency has neither license expression nor license_file: "
                f"{package['name']} {package['version']}"
            )
        required_documents = 1
    else:
        identifiers = expression_identifiers(expression)
        if not identifiers:
            raise NoticeError(
                f"dependency has an invalid license expression: "
                f"{package['name']} {package['version']}: {expression}"
            )
        compound = "/" in expression or any(
            operator in expression.upper() for operator in (" AND ", " OR ", " WITH ")
        )
        required_documents = len(identifiers) if compound else 1
    unique_texts = {document["sha256"] for document in documents}
    if len(unique_texts) < required_documents:
        raise NoticeError(
            f"missing or ambiguous license text for {package['name']} "
            f"{package['version']}: expression {expression!r} needs at least "
            f"{required_documents} distinct document(s), found {len(unique_texts)}; "
            "add a pinned verified override"
        )
    if identifiers:
        for identifier in sorted(identifiers):
            markers = SPDX_TEXT_MARKERS.get(identifier)
            if markers is None:
                raise NoticeError(
                    f"no strict document mapping exists for SPDX identifier "
                    f"{identifier!r} in {package['name']} {package['version']}"
                )
            matched = any(
                document_text_matches(identifier, document["text"])
                for document in documents
            )
            if not matched:
                raise NoticeError(
                    f"license text for SPDX identifier {identifier!r} is not "
                    f"unambiguously identified for {package['name']} {package['version']}; "
                    "add a pinned verified override"
                )


def override_index(
    entries: list[dict[str, object]],
    platform: str,
    packages: list[dict[str, object]],
    evidence_by_key: dict[
        tuple[str, str, str],
        tuple[list[dict[str, str]], list[str], dict[str, str] | None],
    ],
) -> dict[tuple[str, str, str], dict[str, object]]:
    packages_by_key = {
        (str(package["name"]), str(package["version"]), str(package["source"])): package
        for package in packages
    }
    active: dict[tuple[str, str, str], dict[str, object]] = {}
    for entry in entries:
        if platform not in entry["platforms"]:
            continue
        key = (str(entry["package"]), str(entry["version"]), str(entry["source"]))
        package = packages_by_key.get(key)
        if package is None:
            raise NoticeError(
                f"stale or mistargeted override for {entry['package']} "
                f"{entry['version']} on {platform}"
            )
        if package.get("license") != entry["license_expression"]:
            raise NoticeError(f"override license expression drift for {entry['package']}")
        vcs = evidence_by_key[key][2]
        if vcs is None or vcs["revision"] != entry["vcs_revision"]:
            raise NoticeError(f"override VCS revision drift for {entry['package']}")
        active[key] = entry
    return active


def override_documents(entry: dict[str, object]) -> list[dict[str, str]]:
    documents: list[dict[str, str]] = []
    for document in entry["documents"]:
        path = repository_path(document["path"])
        content, text = read_notice(path)
        documents.append(
            {
                "origin": "verified-override",
                "path": str(document["path"]),
                "sha256": sha256_bytes(content),
                "source_url": str(document["source_url"]),
                "text": text,
            }
        )
    return documents


def dependency_record(
    package: dict[str, object],
    entry: dict[str, object] | None,
    evidence: tuple[
        list[dict[str, str]], list[str], dict[str, str] | None
    ],
) -> tuple[dict[str, object], dict[str, str]]:
    source_documents, source_pointers, vcs = evidence
    documents = [dict(document) for document in source_documents]
    unresolved_pointers = list(source_pointers)
    if unresolved_pointers and entry is None:
        pointers = ", ".join(unresolved_pointers)
        raise NoticeError(
            f"dependency contains unresolved notice pointer(s) without a verified "
            f"override: {package['name']} {package['version']}: {pointers}"
        )
    if entry is not None:
        documents.extend(override_documents(entry))
    documents.sort(key=lambda document: (document["origin"], document["path"]))
    assert_document_coverage(package, documents)

    unique_documents: list[dict[str, str]] = []
    seen_documents: set[tuple[str, str, str]] = set()
    text_by_hash: dict[str, str] = {}
    for document in documents:
        key = (document["origin"], document["path"], document["sha256"])
        if key in seen_documents:
            continue
        seen_documents.add(key)
        text = document.pop("text")
        existing = text_by_hash.get(document["sha256"])
        if existing is not None and existing != text:
            raise NoticeError(f"SHA-256 collision in notice documents: {document['sha256']}")
        text_by_hash[document["sha256"]] = text
        unique_documents.append(document)

    record: dict[str, object] = {
        "name": package["name"],
        "version": package["version"],
        "source": package["source"],
        "repository": package.get("repository"),
        "license_expression": package.get("license"),
        "active_features": package["active_features"],
        "documents": unique_documents,
    }
    if vcs is not None:
        record["vcs"] = dict(vcs)
    if entry is not None:
        record["override_reason"] = entry["reason"]
    if unresolved_pointers:
        record["replaced_package_pointers"] = unresolved_pointers
    return record, text_by_hash


def render_human_report(report: dict[str, object]) -> str:
    lines = [
        "Linxira Bio SDK Third-Party Cargo Dependency Notices",
        "====================================================",
        "",
        f"Release platform: {report['platform']}",
        f"Rust target: {report['target_triple']}",
        f"Cargo: {report['cargo_version']}",
        f"Cargo.lock SHA-256: {report['cargo_lock_sha256']}",
        f"External dependency count: {report['dependency_count']}",
        "",
        "This file is generated deterministically from the locked, target-filtered",
        "Cargo release dependency graph. Project-owned code remains licensed under",
        "AGPL-3.0-or-later. Third-party terms below are not replaced or relicensed.",
        "",
        "Release roots",
        "-------------",
    ]
    for root in report["release_roots"]:
        lines.append(f"- {root['name']} {root['version']}")
    lines.extend(["", "Dependencies", "------------"])
    for dependency in report["dependencies"]:
        expression = dependency.get("license_expression") or "license_file"
        lines.append(f"- {dependency['name']} {dependency['version']} [{expression}]")
        lines.append(f"  Source: {dependency['source']}")
        if dependency.get("repository"):
            lines.append(f"  Repository: {dependency['repository']}")
        if dependency.get("vcs"):
            lines.append(f"  VCS revision: {dependency['vcs']['revision']}")
        if dependency.get("override_reason"):
            lines.append(f"  Verified override: {dependency['override_reason']}")
        if dependency.get("replaced_package_pointers"):
            lines.append(
                "  Replaced package pointer(s): "
                + ", ".join(dependency["replaced_package_pointers"])
            )
        for document in dependency["documents"]:
            lines.append(
                f"  Notice: {document['origin']}:{document['path']} "
                f"sha256:{document['sha256']}"
            )

    users: dict[str, set[str]] = defaultdict(set)
    sources: dict[str, set[str]] = defaultdict(set)
    for dependency in report["dependencies"]:
        package_name = f"{dependency['name']} {dependency['version']}"
        for document in dependency["documents"]:
            digest = document["sha256"]
            users[digest].add(package_name)
            sources[digest].add(f"{document['origin']}:{document['path']}")

    lines.extend(["", "Retained license and notice texts", "================================="])
    for license_text in report["license_texts"]:
        digest = license_text["sha256"]
        lines.extend(
            [
                "",
                f"SHA-256: {digest}",
                f"Used by: {', '.join(sorted(users[digest]))}",
                f"Documents: {', '.join(sorted(sources[digest]))}",
                "-" * 72,
                license_text["text"].rstrip("\r\n"),
                "-" * 72,
            ]
        )
    return "\n".join(lines) + "\n"


def validate_generated_report(report: dict[str, object]) -> None:
    dependencies = report.get("dependencies")
    license_texts = report.get("license_texts")
    if not isinstance(dependencies, list) or report.get("dependency_count") != len(
        dependencies
    ):
        raise NoticeError("generated report dependency_count is inconsistent")
    if not isinstance(license_texts, list):
        raise NoticeError("generated report lacks license_texts")

    dependency_identities: set[tuple[object, object, object]] = set()
    document_hashes: set[str] = set()
    for dependency in dependencies:
        if not isinstance(dependency, dict):
            raise NoticeError("generated report contains an invalid dependency")
        identity = (
            dependency.get("name"),
            dependency.get("version"),
            dependency.get("source"),
        )
        if identity in dependency_identities:
            raise NoticeError(f"generated report repeats dependency {identity}")
        dependency_identities.add(identity)
        for document in dependency.get("documents", []):
            if not isinstance(document, dict) or not isinstance(
                document.get("sha256"), str
            ):
                raise NoticeError(f"generated report has an invalid document: {identity}")
            document_hashes.add(document["sha256"])

    text_hashes: set[str] = set()
    for license_text in license_texts:
        if not isinstance(license_text, dict) or not isinstance(
            license_text.get("text"), str
        ):
            raise NoticeError("generated report contains an invalid retained text")
        digest = license_text.get("sha256")
        if not isinstance(digest, str) or sha256_bytes(
            license_text["text"].encode("utf-8")
        ) != digest:
            raise NoticeError(f"generated retained text SHA-256 mismatch: {digest}")
        if digest in text_hashes:
            raise NoticeError(f"generated report repeats retained text: {digest}")
        text_hashes.add(digest)
    if document_hashes != text_hashes:
        raise NoticeError("generated document and retained-text hashes are inconsistent")

    try:
        from jsonschema import Draft202012Validator, FormatChecker
    except ImportError as error:
        raise NoticeError(
            "jsonschema is required to validate release notices; install "
            "requirements-ci.txt"
        ) from error
    schema = load_json(REPORT_SCHEMA_PATH)
    if not isinstance(schema, dict):
        raise NoticeError("third-party dependency schema must contain an object")
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(
        validator.iter_errors(report),
        key=lambda error: tuple(str(part) for part in error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{error.json_path}: {error.message}" for error in errors[:5]
        )
        raise NoticeError(f"generated report does not match its schema: {details}")


def build_report(platform: str, *, offline: bool = False) -> dict[str, object]:
    configuration = load_bundle_notice_configuration()
    entries = validate_override_configuration()
    lock_content = (ROOT / "Cargo.lock").read_bytes()
    registry_checksums = cargo_lock_registry_checksums(lock_content)
    target = configuration["targets"][platform]
    metadata = cargo_metadata(str(target), offline=offline)
    selected_packages = cargo_release_tree(
        str(target), list(configuration["roots"]), offline=offline
    )
    roots, packages = release_closure(
        metadata, list(configuration["roots"]), selected_packages
    )
    evidence_by_key = {
        (
            str(package["name"]),
            str(package["version"]),
            str(package["source"]),
        ): collect_package_evidence(package, registry_checksums)
        for package in packages
    }
    active_overrides = override_index(entries, platform, packages, evidence_by_key)

    dependencies: list[dict[str, object]] = []
    text_by_hash: dict[str, str] = {}
    for package in packages:
        key = (str(package["name"]), str(package["version"]), str(package["source"]))
        record, package_texts = dependency_record(
            package, active_overrides.get(key), evidence_by_key[key]
        )
        for digest, text in package_texts.items():
            existing = text_by_hash.get(digest)
            if existing is not None and existing != text:
                raise NoticeError(f"SHA-256 collision in retained texts: {digest}")
            text_by_hash[digest] = text
        dependencies.append(record)

    override_content = OVERRIDES_PATH.read_bytes()
    return {
        "$schema": "https://linxira.org/schemas/bio/third-party-dependencies.v1.json",
        "schema_version": "1",
        "generator_version": GENERATOR_VERSION,
        "cargo_version": cargo_version(),
        "platform": platform,
        "target_triple": target,
        "cargo_lock_sha256": sha256_bytes(lock_content),
        "override_manifest_sha256": sha256_bytes(override_content),
        "release_roots": roots,
        "dependency_count": len(dependencies),
        "dependencies": dependencies,
        "license_texts": [
            {"sha256": digest, "text": text}
            for digest, text in sorted(text_by_hash.items())
        ],
    }


def generate_notice_bundle(
    platform: str, output: Path, *, offline: bool = False
) -> tuple[Path, Path]:
    if platform not in SUPPORTED_PLATFORMS:
        raise NoticeError(f"unsupported release platform: {platform}")
    report = build_report(platform, offline=offline)
    validate_generated_report(report)
    output.mkdir(parents=True, exist_ok=True)
    json_path = output / JSON_OUTPUT_NAME
    text_path = output / TEXT_OUTPUT_NAME
    json_content = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    json_path.write_bytes(json_content.encode("utf-8"))
    text_path.write_bytes(render_human_report(report).encode("utf-8"))
    return json_path, text_path


def main() -> None:
    arguments = parse_arguments()
    load_bundle_notice_configuration()
    entries = validate_override_configuration()
    if arguments.check_config:
        print(f"validated {len(entries)} pinned Cargo license override entries")
        return
    assert arguments.platform is not None and arguments.output is not None
    json_path, text_path = generate_notice_bundle(
        arguments.platform, arguments.output.resolve(), offline=arguments.offline
    )
    print(json_path)
    print(text_path)


if __name__ == "__main__":
    try:
        main()
    except NoticeError as error:
        raise SystemExit(f"third-party notice error: {error}") from error
