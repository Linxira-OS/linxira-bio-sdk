"""Generate a pack's `requirements.lock` from a `requirements.in` pin set.

The lock files in this repository are `--require-hashes` inputs: every direct and
transitive requirement must be pinned with the hashes of the artifacts an
installer may actually select. Hand-maintaining them is how the benchmark lock
ended up with an escaped line-break sequence where a line break belonged and
without `pyfastx`, which `pytrf` declares — a lock that `uv pip install
--require-hashes` rejected outright, so the CI step that installs it never ran
the tests behind it.

This generator resolves the pins with `uv pip compile --generate-hashes` once per
target platform and merges the hash sets, so one lock serves every platform the
pack is installed on. Regeneration is a command, not an edit:

    python scripts/generate-python-lock.py \\
        workflows/org.linxira.benchmark-python/requirements.in \\
        --out workflows/org.linxira.benchmark-python/requirements.lock

Platforms default to the two the repository publishes for: x86_64-manylinux (CI
and every benchmark host) and x86_64 windows (development machines). Adding a
platform means adding a `--platform` value and regenerating; never edit the
hashes.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

DEFAULT_PLATFORMS = ("x86_64-manylinux_2_28", "x86_64-pc-windows-msvc")

PIN_RE = re.compile(r"^(?P<name>[A-Za-z0-9._-]+)==(?P<version>[^\s\\]+)\s*\\?$")
HASH_RE = re.compile(r"^\s*--hash=sha256:(?P<digest>[0-9a-f]{64})\s*\\?$")
VIA_RE = re.compile(r"^\s*#\s*via\s*(?P<via>.*?)\s*$")


class LockError(RuntimeError):
    pass


def compile_platform(
    requirements: Path, platform: str, python_version: str, uv: str
) -> dict[str, dict[str, object]]:
    """Resolve one platform into `{package: {version, hashes, via}}`."""
    with tempfile.TemporaryDirectory() as directory:
        out = Path(directory) / "platform.lock"
        command = [
            uv,
            "pip",
            "compile",
            "--generate-hashes",
            "--no-header",
            "--quiet",
            "--python-version",
            python_version,
            "--python-platform",
            platform,
            "-o",
            str(out),
            str(requirements),
        ]
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
        if completed.returncode != 0:
            raise LockError(
                f"uv pip compile failed for {platform} ({completed.returncode}):\n"
                f"{completed.stderr.strip()}"
            )
        return parse_compile_output(out.read_text(encoding="utf-8"))


def normalise_via(raw: str) -> str:
    """Turn a `# via` entry into the reason to print in the lock."""
    entry = raw.strip()
    if not entry:
        return ""
    if entry.startswith("-r "):
        name = entry[3:].strip().rstrip("/")
        name = name.rsplit("/", 1)[-1]
        return f"the direct pins ({name})"
    return entry


def parse_compile_output(text: str) -> dict[str, dict[str, object]]:
    packages: dict[str, dict[str, object]] = {}
    current: dict[str, object] | None = None
    in_via_block = False
    for raw_line in text.splitlines():
        if not raw_line.strip():
            continue
        pin = PIN_RE.match(raw_line.strip())
        if pin and not raw_line.lstrip().startswith("--"):
            current = {
                "version": pin.group("version"),
                "hashes": [],
                "via": [],
            }
            packages[pin.group("name")] = current
            in_via_block = False
            continue
        if current is None:
            raise LockError(f"lock line outside a package block: {raw_line!r}")
        digest = HASH_RE.match(raw_line)
        if digest:
            hashes = current["hashes"]
            assert isinstance(hashes, list)
            if digest.group("digest") not in hashes:
                hashes.append(digest.group("digest"))
            in_via_block = False
            continue
        via = VIA_RE.match(raw_line)
        if via:
            vias = current["via"]
            entry = normalise_via(via.group("via"))
            if entry and entry not in vias:
                vias.append(entry)
            in_via_block = not entry
            continue
        if in_via_block and raw_line.lstrip().startswith("#"):
            vias = current["via"]
            assert isinstance(vias, list)
            entry = normalise_via(raw_line.strip().lstrip("#").strip())
            if entry and entry not in vias:
                vias.append(entry)
            continue
        raise LockError(f"unrecognised lock line: {raw_line!r}")
    if not packages:
        raise LockError("uv produced no packages")
    return packages


def merge_platforms(
    per_platform: dict[str, dict[str, dict[str, object]]]
) -> dict[str, dict[str, object]]:
    merged: dict[str, dict[str, object]] = {}
    for platform, packages in per_platform.items():
        for name, package in packages.items():
            target = merged.setdefault(
                name, {"version": package["version"], "hashes": [], "via": []}
            )
            if target["version"] != package["version"]:
                raise LockError(
                    f"{name}: platform {platform} resolves "
                    f"{package['version']}, others resolve {target['version']}"
                )
            target_hashes = target["hashes"]
            target_via = target["via"]
            assert isinstance(target_hashes, list) and isinstance(target_via, list)
            for digest in package["hashes"]:  # type: ignore[union-attr]
                if digest not in target_hashes:
                    target_hashes.append(digest)
            for entry in package["via"]:  # type: ignore[union-attr]
                if entry not in target_via:
                    target_via.append(entry)
    return merged


def render(
    merged: dict[str, dict[str, object]],
    requirements: Path,
    platforms: tuple[str, ...],
    python_version: str,
    header: str,
) -> str:
    lines: list[str] = [line for line in header.strip().splitlines()]
    platforms_text = " ".join(f"--platform {platform}" for platform in platforms)
    lines += [
        "",
        f"# Regenerate, never edit:",
        f"#   python scripts/generate-python-lock.py {requirements.as_posix()} \\",
        f"#     --out {requirements.with_suffix('.lock').as_posix()} \\",
        f"#     --python-version {python_version} {platforms_text}",
        "",
    ]
    for name in sorted(merged):
        package = merged[name]
        hashes = package["hashes"]
        vias = package["via"]
        assert isinstance(hashes, list) and isinstance(vias, list)
        lines.append(f"{name}=={package['version']} \\")
        for index, digest in enumerate(hashes):
            suffix = " \\" if index < len(hashes) - 1 else ""
            lines.append(f"    --hash=sha256:{digest}{suffix}")
        for entry in vias:
            lines.append(f"    # via {entry}")
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("requirements", type=Path, help="requirements.in pin set")
    parser.add_argument("--out", type=Path, required=True, help="lock file to write")
    parser.add_argument("--python-version", default="3.12")
    parser.add_argument("--platform", action="append", default=None)
    parser.add_argument(
        "--header-file",
        type=Path,
        default=None,
        help="file holding the lock's leading comment block",
    )
    arguments = parser.parse_args(argv)

    requirements = arguments.requirements
    if not requirements.is_file():
        raise SystemExit(f"requirements file not found: {requirements}")
    uv = shutil.which("uv")
    if uv is None:
        raise SystemExit("uv is required to regenerate a lock (https://docs.astral.sh/uv/)")

    platforms = tuple(arguments.platform or DEFAULT_PLATFORMS)
    per_platform = {
        platform: compile_platform(requirements, platform, arguments.python_version, uv)
        for platform in platforms
    }
    merged = merge_platforms(per_platform)

    if arguments.header_file is not None:
        header = arguments.header_file.read_text(encoding="utf-8")
    elif arguments.out.is_file():
        existing = [
            line
            for line in arguments.out.read_text(encoding="utf-8").splitlines()
            if line.startswith("#") and not line.startswith("# Regenerate")
        ]
        header = "\n".join(line for line in existing if line.strip())
    else:
        header = f"# Lock for {requirements.as_posix()} (CPython {arguments.python_version})."

    rendered = render(merged, requirements, platforms, arguments.python_version, header)
    arguments.out.parent.mkdir(parents=True, exist_ok=True)
    arguments.out.write_text(rendered, encoding="utf-8", newline="\n")

    total_hashes = sum(len(package["hashes"]) for package in merged.values())  # type: ignore[arg-type]
    print(
        f"wrote {arguments.out}: {len(merged)} packages, {total_hashes} hashes, "
        f"platforms={','.join(platforms)}"
    )
    for name in sorted(merged):
        print(f"  {name}=={merged[name]['version']} ({len(merged[name]['hashes'])})")  # type: ignore[arg-type]
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
