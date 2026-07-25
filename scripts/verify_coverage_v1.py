#!/usr/bin/env python3
"""Validate the weighted v1 scientific coverage and clean-room reference map."""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_COVERAGE = ROOT / "capabilities" / "coverage-v1.json"
DEFAULT_CAPABILITY_CATALOG = ROOT / "capabilities" / "catalog.json"
DEFAULT_REFERENCE_INVENTORY = (
    ROOT / "capabilities" / "reference-inventories" / "offline-reference-2.475.json"
)

DOMAINS = {
    "general-biology": 75,
    "biochemistry-structure": 15,
    "medical-omics-ruo": 10,
}
DOMAIN_TARGETS = {
    "general-biology": 63,
    "biochemistry-structure": 12,
    "medical-omics-ruo": 5,
}
DISPOSITIONS = {
    "implemented",
    "wrapped",
    "planned-with-owner",
    "excluded-with-reason",
}
EXECUTIONS = {"local", "cloud"}
ID_PATTERN = re.compile(r"^[a-z0-9]+(?:[.-][a-z0-9]+)*$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")


class CoverageValidationError(ValueError):
    """Raised when the v1 coverage contract is inconsistent."""


@dataclass(frozen=True)
class CoverageSummary:
    total_weight: int
    target_weight: int
    achieved_weight: int
    achieved_by_domain: dict[str, int]
    target_by_domain: dict[str, int]
    offline_reference_count: int


def _object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise CoverageValidationError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path: Path) -> dict[str, Any]:
    try:
        document = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=_object_without_duplicate_keys,
        )
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CoverageValidationError(f"cannot read JSON document {path}: {error}") from error
    if not isinstance(document, dict):
        raise CoverageValidationError(f"JSON document must be an object: {path}")
    return document


def _require_nonempty_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise CoverageValidationError(f"{label} must be a non-empty string")
    return value


def _require_unique_strings(value: object, label: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise CoverageValidationError(f"{label} must be a non-empty list")
    strings = [_require_nonempty_string(item, label) for item in value]
    if len(strings) != len(set(strings)):
        raise CoverageValidationError(f"{label} contains duplicates")
    return strings


def _validate_header(document: dict[str, Any], name: str) -> None:
    if document.get("schema_version") != "1":
        raise CoverageValidationError(f"invalid {name} schema_version")


def validate_coverage_documents(
    coverage: dict[str, Any],
    capability_catalog: dict[str, Any],
    reference_inventory: dict[str, Any],
    *,
    release: bool = False,
) -> CoverageSummary:
    """Validate in-memory documents and optionally enforce the v1 release target."""

    _validate_header(coverage, "coverage catalog")
    _validate_header(capability_catalog, "capability catalog")
    _validate_header(reference_inventory, "offline reference inventory")

    if coverage.get("measurement") != "weighted-functional-slices":
        raise CoverageValidationError("coverage measurement must be weighted-functional-slices")
    if coverage.get("total_weight") != 100:
        raise CoverageValidationError("declared total_weight must be 100")

    domain_budgets = coverage.get("domain_budgets")
    if domain_budgets != DOMAINS:
        raise CoverageValidationError(f"domain_budgets must equal {DOMAINS}")

    release_target = coverage.get("release_target")
    if not isinstance(release_target, dict):
        raise CoverageValidationError("release_target must be an object")
    minimum_weight = release_target.get("minimum_weight")
    if not isinstance(minimum_weight, int) or isinstance(minimum_weight, bool):
        raise CoverageValidationError("release_target.minimum_weight must be an integer")
    if minimum_weight < 80 or minimum_weight > 100:
        raise CoverageValidationError("release target must be between 80 and 100")
    if release_target.get("by_domain") != DOMAIN_TARGETS:
        raise CoverageValidationError(f"release target domains must equal {DOMAIN_TARGETS}")
    if sum(DOMAIN_TARGETS.values()) != minimum_weight:
        raise CoverageValidationError("release target domain weights do not match minimum_weight")

    capabilities = capability_catalog.get("capabilities")
    if not isinstance(capabilities, list):
        raise CoverageValidationError("capability catalog requires capabilities")
    capability_statuses: dict[str, str] = {}
    capability_commands: dict[str, str] = {}
    for capability in capabilities:
        if not isinstance(capability, dict):
            raise CoverageValidationError("capability entry must be an object")
        capability_id = _require_nonempty_string(capability.get("id"), "capability id")
        if capability_id in capability_statuses:
            raise CoverageValidationError(f"duplicate capability id: {capability_id}")
        status = _require_nonempty_string(
            capability.get("status"), f"capability status for {capability_id}"
        )
        capability_statuses[capability_id] = status
        command = capability.get("command")
        if isinstance(command, str) and command.strip():
            capability_commands[capability_id] = command

    items = coverage.get("items")
    if not isinstance(items, list) or not items:
        raise CoverageValidationError("coverage catalog requires items")

    item_by_id: dict[str, dict[str, Any]] = {}
    weights_by_domain: Counter[str] = Counter()
    targets_by_domain: Counter[str] = Counter()
    achieved_by_domain: Counter[str] = Counter()
    reference_refs: dict[str, set[str]] = {}

    for item in items:
        if not isinstance(item, dict):
            raise CoverageValidationError("coverage item must be an object")
        item_id = _require_nonempty_string(item.get("id"), "coverage item id")
        if not ID_PATTERN.fullmatch(item_id):
            raise CoverageValidationError(f"invalid coverage item id: {item_id}")
        if item_id in item_by_id:
            raise CoverageValidationError(f"duplicate coverage item id: {item_id}")
        item_by_id[item_id] = item

        _require_nonempty_string(item.get("name"), f"name for {item_id}")
        domain = item.get("domain")
        if domain not in DOMAINS:
            raise CoverageValidationError(f"invalid domain for {item_id}: {domain}")
        weight = item.get("weight")
        if not isinstance(weight, int) or isinstance(weight, bool) or weight <= 0:
            raise CoverageValidationError(f"weight for {item_id} must be a positive integer")
        weights_by_domain[domain] += weight

        execution = item.get("execution")
        if execution not in EXECUTIONS:
            raise CoverageValidationError(f"invalid execution for {item_id}: {execution}")
        target_v1 = item.get("target_v1")
        if not isinstance(target_v1, bool):
            raise CoverageValidationError(f"target_v1 for {item_id} must be boolean")
        if target_v1:
            targets_by_domain[domain] += weight

        disposition = item.get("disposition")
        if disposition not in DISPOSITIONS:
            raise CoverageValidationError(
                f"invalid disposition for {item_id}: {disposition}"
            )
        evidence = _require_unique_strings(item.get("evidence"), f"evidence for {item_id}")
        item_reference_refs = {
            reference.removeprefix("offline-reference:")
            for reference in evidence
            if reference.startswith("offline-reference:")
        }
        reference_refs[item_id] = item_reference_refs

        if disposition in {"implemented", "wrapped"}:
            capability_ids = _require_unique_strings(
                item.get("capability_ids"), f"capability_ids for {item_id}"
            )
            for capability_id in capability_ids:
                if f"catalog:{capability_id}" not in evidence:
                    raise CoverageValidationError(
                        f"{item_id} lacks catalog evidence for {capability_id}"
                    )
                if capability_id not in capability_statuses:
                    raise CoverageValidationError(
                        f"{item_id} references unknown capability {capability_id}"
                    )
                if capability_statuses[capability_id] != "available":
                    raise CoverageValidationError(
                        f"{item_id} maps to non-available capability {capability_id} "
                        f"({capability_statuses[capability_id]})"
                    )
                if capability_id not in capability_commands:
                    raise CoverageValidationError(
                        f"{item_id} maps to capability without command {capability_id}"
                    )
            achieved_by_domain[domain] += weight
            if "owner" in item or "reason" in item:
                raise CoverageValidationError(
                    f"{item_id} has planning/exclusion fields after implementation"
                )
        elif disposition == "planned-with-owner":
            _require_nonempty_string(item.get("owner"), f"owner for {item_id}")
            if "capability_ids" in item or "reason" in item:
                raise CoverageValidationError(
                    f"{item_id} has fields incompatible with planned-with-owner"
                )
        else:
            _require_nonempty_string(item.get("reason"), f"reason for {item_id}")
            if target_v1:
                raise CoverageValidationError(
                    f"excluded item cannot be a v1 release target: {item_id}"
                )
            if "capability_ids" in item or "owner" in item:
                raise CoverageValidationError(
                    f"{item_id} has fields incompatible with excluded-with-reason"
                )

    if dict(weights_by_domain) != DOMAINS:
        raise CoverageValidationError(
            f"actual domain weights {dict(weights_by_domain)} do not equal {DOMAINS}"
        )
    if sum(weights_by_domain.values()) != 100:
        raise CoverageValidationError("actual coverage item weights must total 100")
    if dict(targets_by_domain) != DOMAIN_TARGETS:
        raise CoverageValidationError(
            f"target item weights {dict(targets_by_domain)} do not equal {DOMAIN_TARGETS}"
        )

    if reference_inventory.get("product") != "external-offline-reference" or reference_inventory.get(
        "version"
    ) != "2.475":
        raise CoverageValidationError("unexpected offline reference product/version")
    origin = reference_inventory.get("origin")
    if not isinstance(origin, dict):
        raise CoverageValidationError("reference inventory requires source origin")
    _require_nonempty_string(origin.get("acquisition"), "reference acquisition")
    _require_nonempty_string(origin.get("recorded_on"), "reference record date")
    _require_nonempty_string(
        origin.get("redistribution_status"), "reference redistribution status"
    )
    clean_room = reference_inventory.get("clean_room")
    if not isinstance(clean_room, dict):
        raise CoverageValidationError("reference inventory requires clean_room policy")
    if clean_room.get("class_decompilation") is not False:
        raise CoverageValidationError("clean-room inventory must forbid class decompilation")
    if clean_room.get("implementation_copying") is not False:
        raise CoverageValidationError("clean-room inventory must forbid implementation copying")
    if clean_room.get("resource_redistribution") is not False:
        raise CoverageValidationError("clean-room inventory must forbid resource redistribution")

    artifacts = reference_inventory.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise CoverageValidationError("reference inventory requires artifact hashes")
    artifact_ids: set[str] = set()
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            raise CoverageValidationError("reference artifact must be an object")
        artifact_id = _require_nonempty_string(artifact.get("id"), "reference artifact id")
        if artifact_id in artifact_ids:
            raise CoverageValidationError(f"duplicate reference artifact id: {artifact_id}")
        artifact_ids.add(artifact_id)
        _require_nonempty_string(artifact.get("filename"), f"filename for {artifact_id}")
        sha256 = _require_nonempty_string(artifact.get("sha256"), f"sha256 for {artifact_id}")
        if not SHA256_PATTERN.fullmatch(sha256):
            raise CoverageValidationError(f"invalid sha256 for {artifact_id}")
        size = artifact.get("size_bytes")
        if not isinstance(size, int) or isinstance(size, bool) or size <= 0:
            raise CoverageValidationError(f"invalid size_bytes for {artifact_id}")

    observed_features = reference_inventory.get("observed_features")
    if not isinstance(observed_features, list) or not observed_features:
        raise CoverageValidationError("reference inventory requires observed_features")
    inventory_scope = reference_inventory.get("inventory_scope")
    if not isinstance(inventory_scope, dict):
        raise CoverageValidationError("reference inventory requires inventory_scope")
    expected_feature_count = inventory_scope.get("observed_feature_count")
    expected_offline_count = inventory_scope.get("offline_feature_count")
    expected_cloud_count = inventory_scope.get("cloud_feature_count")
    if (expected_feature_count, expected_offline_count, expected_cloud_count) != (
        60,
        57,
        3,
    ):
        raise CoverageValidationError("reference inventory scope must remain 60/57/3")
    if len(observed_features) != expected_feature_count:
        raise CoverageValidationError(
            "reference observed feature count does not match inventory_scope"
        )
    feature_ids: set[str] = set()
    offline_count = 0
    for feature in observed_features:
        if not isinstance(feature, dict):
            raise CoverageValidationError("reference observed feature must be an object")
        feature_id = _require_nonempty_string(feature.get("id"), "reference feature id")
        if feature_id in feature_ids:
            raise CoverageValidationError(f"duplicate reference feature id: {feature_id}")
        feature_ids.add(feature_id)
        _require_nonempty_string(feature.get("name"), f"name for {feature_id}")
        execution = feature.get("execution")
        if execution not in EXECUTIONS:
            raise CoverageValidationError(
                f"invalid execution for reference feature {feature_id}: {execution}"
            )
        coverage_ids = _require_unique_strings(
            feature.get("coverage_ids"), f"coverage_ids for {feature_id}"
        )
        if execution == "local":
            offline_count += 1
        for coverage_id in coverage_ids:
            if coverage_id not in item_by_id:
                raise CoverageValidationError(
                    f"reference feature {feature_id} maps to unknown coverage item {coverage_id}"
                )
            if feature_id not in reference_refs[coverage_id]:
                raise CoverageValidationError(
                    f"coverage item {coverage_id} lacks evidence for reference feature {feature_id}"
                )
            if execution == "local" and item_by_id[coverage_id].get("execution") != "local":
                raise CoverageValidationError(
                    f"offline reference feature {feature_id} maps only as non-local coverage"
                )

    unknown_feature_refs = {
        reference
        for references in reference_refs.values()
        for reference in references
        if reference not in feature_ids
    }
    if unknown_feature_refs:
        raise CoverageValidationError(
            f"coverage evidence references unknown reference features: {sorted(unknown_feature_refs)}"
        )
    if offline_count != expected_offline_count:
        raise CoverageValidationError(
            "reference offline feature count does not match inventory_scope"
        )
    if len(observed_features) - offline_count != expected_cloud_count:
        raise CoverageValidationError(
            "reference cloud feature count does not match inventory_scope"
        )

    achieved_weight = sum(achieved_by_domain.values())
    if release:
        incomplete_targets = [
            item_id
            for item_id, item in item_by_id.items()
            if item["target_v1"]
            and item["disposition"] not in {"implemented", "wrapped"}
        ]
        if incomplete_targets:
            preview = ", ".join(incomplete_targets[:8])
            remaining = len(incomplete_targets) - min(8, len(incomplete_targets))
            suffix = f" (+{remaining} more)" if remaining else ""
            raise CoverageValidationError(
                f"v1 release target is incomplete: {preview}{suffix}"
            )
        if achieved_weight < minimum_weight:
            raise CoverageValidationError(
                f"v1 achieved weight {achieved_weight} is below target {minimum_weight}"
            )
        for domain, target in DOMAIN_TARGETS.items():
            achieved = achieved_by_domain[domain]
            if achieved < target:
                raise CoverageValidationError(
                    f"v1 achieved weight for {domain} is {achieved}, below {target}"
                )

    return CoverageSummary(
        total_weight=sum(weights_by_domain.values()),
        target_weight=minimum_weight,
        achieved_weight=achieved_weight,
        achieved_by_domain={domain: achieved_by_domain[domain] for domain in DOMAINS},
        target_by_domain=dict(DOMAIN_TARGETS),
        offline_reference_count=offline_count,
    )


def validate_coverage_files(
    coverage_path: Path = DEFAULT_COVERAGE,
    capability_catalog_path: Path = DEFAULT_CAPABILITY_CATALOG,
    reference_inventory_path: Path = DEFAULT_REFERENCE_INVENTORY,
    *,
    release: bool = False,
) -> CoverageSummary:
    return validate_coverage_documents(
        load_json(coverage_path),
        load_json(capability_catalog_path),
        load_json(reference_inventory_path),
        release=release,
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--coverage", type=Path, default=DEFAULT_COVERAGE)
    parser.add_argument(
        "--capability-catalog", type=Path, default=DEFAULT_CAPABILITY_CATALOG
    )
    parser.add_argument(
        "--reference-inventory", type=Path, default=DEFAULT_REFERENCE_INVENTORY
    )
    parser.add_argument(
        "--release",
        action="store_true",
        help="require every weighted v1 target to be implemented or wrapped",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        summary = validate_coverage_files(
            args.coverage,
            args.capability_catalog,
            args.reference_inventory,
            release=args.release,
        )
    except CoverageValidationError as error:
        print(f"coverage-v1 validation failed: {error}")
        return 1
    mode = "release" if args.release else "structure"
    print(
        f"validated coverage-v1 ({mode}): {summary.total_weight} total, "
        f"{summary.target_weight} target, {summary.achieved_weight} achieved, "
        f"{summary.offline_reference_count} offline reference facts"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
