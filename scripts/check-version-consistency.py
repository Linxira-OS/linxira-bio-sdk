#!/usr/bin/env python3
"""Check that release versions agree across native, SDK, conda, and tag.

The product version in ``Cargo.toml`` is canonical. The Python SDK and the
Bioconda recipe must use it, and a release tag must be ``v<product>``.

PyPI release ``0.1.0`` is immutable and predates the lockstep policy, so the
exact pair ``(1.1.5, 0.1.0)`` is allowed temporarily. Remove that exception at
the next release.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
LEGACY_SDK_VERSION = ("1.1.5", "0.1.0")


def _section_value(text: str, section: str, key: str, source: Path) -> str:
    section_match = re.search(rf"^\[{re.escape(section)}\]\s*$", text, re.MULTILINE)
    if section_match is None:
        raise ValueError(f"{source}: missing [{section}] section")
    start = section_match.end()
    next_section = re.search(r"^\[", text[start:], re.MULTILINE)
    block = text[start : start + next_section.start()] if next_section else text[start:]
    value_match = re.search(
        rf"^{re.escape(key)}\s*=\s*\"([^\"]+)\"", block, re.MULTILINE
    )
    if value_match is None:
        raise ValueError(f"{source}: missing {key} in [{section}]")
    return value_match.group(1)


def _read_versions(root: Path) -> dict[str, str]:
    cargo = root / "Cargo.toml"
    pyproject = root / "sdk" / "python" / "pyproject.toml"
    sdk_init = root / "sdk" / "python" / "linxira_bio" / "__init__.py"
    recipe = root / "packaging" / "bioconda" / "meta.yaml"

    cargo_text = cargo.read_text(encoding="utf-8")
    pyproject_text = pyproject.read_text(encoding="utf-8")
    sdk_init_text = sdk_init.read_text(encoding="utf-8")
    recipe_text = recipe.read_text(encoding="utf-8")

    recipe_match = re.search(
        r'\{%\s*set\s+version\s*=\s*"([^"]+)"\s*%\}', recipe_text
    )
    if recipe_match is None:
        raise ValueError(f"{recipe}: missing version assignment")

    init_match = re.search(
        r'^__version__\s*=\s*"([^"]+)"', sdk_init_text, re.MULTILINE
    )
    if init_match is None:
        raise ValueError(f"{sdk_init}: missing __version__")

    return {
        "product": _section_value(cargo_text, "workspace.package", "version", cargo),
        "sdk": _section_value(pyproject_text, "project", "version", pyproject),
        "sdk_init": init_match.group(1),
        "conda": recipe_match.group(1),
    }


def _release_tag() -> str | None:
    ref_type = os.environ.get("GITHUB_REF_TYPE")
    ref_name = os.environ.get("GITHUB_REF_NAME")
    if ref_type == "tag" and ref_name:
        return ref_name
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tag",
        help="release tag to check; defaults to GITHUB_REF_NAME on tag builds",
    )
    args = parser.parse_args()

    try:
        versions = _read_versions(ROOT)
    except (OSError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    product = versions["product"]
    sdk = versions["sdk"]
    failures: list[str] = []

    print(
        "versions: "
        f"product={product} sdk={sdk} sdk_init={versions['sdk_init']} "
        f"conda={versions['conda']}"
    )

    if versions["conda"] != product:
        failures.append(
            f"Bioconda recipe version {versions['conda']} != product {product}"
        )
    if versions["sdk_init"] != sdk:
        failures.append(
            f"SDK __version__ {versions['sdk_init']} != pyproject version {sdk}"
        )

    if sdk == product:
        pass
    elif (product, sdk) == LEGACY_SDK_VERSION:
        print(
            f"WARNING: legacy SDK version exception: product {product} ships "
            f"SDK {sdk}; remove the exception at the next release",
            file=sys.stderr,
        )
    else:
        failures.append(
            f"SDK version {sdk} != product {product}; expected lockstep"
        )

    tag = args.tag or _release_tag()
    if tag is not None:
        expected_tag = f"v{product}"
        if tag != expected_tag:
            failures.append(f"release tag {tag} != {expected_tag}")

    if failures:
        for failure in failures:
            print(f"ERROR: {failure}", file=sys.stderr)
        return 1

    print("PASS: release versions are consistent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
