from __future__ import annotations

import copy
import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "verify_coverage_v1.py"
SPEC = importlib.util.spec_from_file_location("verify_coverage_v1", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
coverage_v1 = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = coverage_v1
SPEC.loader.exec_module(coverage_v1)


class CoverageV1ValidationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.coverage = coverage_v1.load_json(coverage_v1.DEFAULT_COVERAGE)
        cls.catalog = coverage_v1.load_json(
            coverage_v1.DEFAULT_CAPABILITY_CATALOG
        )
        cls.inventory = coverage_v1.load_json(
            coverage_v1.DEFAULT_REFERENCE_INVENTORY
        )

    @staticmethod
    def item(document: dict[str, object], item_id: str) -> dict[str, object]:
        items = document["items"]
        assert isinstance(items, list)
        return next(item for item in items if item["id"] == item_id)

    @staticmethod
    def feature(document: dict[str, object], feature_id: str) -> dict[str, object]:
        features = document["observed_features"]
        assert isinstance(features, list)
        return next(feature for feature in features if feature["id"] == feature_id)

    def validate(
        self,
        coverage: dict[str, object] | None = None,
        catalog: dict[str, object] | None = None,
        inventory: dict[str, object] | None = None,
        *,
        release: bool = False,
    ) -> object:
        return coverage_v1.validate_coverage_documents(
            coverage if coverage is not None else self.coverage,
            catalog if catalog is not None else self.catalog,
            inventory if inventory is not None else self.inventory,
            release=release,
        )

    def test_repository_coverage_contract_is_structurally_valid(self) -> None:
        summary = self.validate()
        self.assertEqual(summary.total_weight, 100)
        self.assertEqual(summary.target_weight, 80)
        self.assertEqual(summary.target_by_domain, coverage_v1.DOMAIN_TARGETS)
        self.assertEqual(summary.offline_reference_count, 57)

    def test_release_gate_rejects_an_incomplete_target(self) -> None:
        document = copy.deepcopy(self.coverage)
        item = self.item(document, "table.manipulate")
        item["target_v1"] = True
        item["disposition"] = "planned-with-owner"
        item["owner"] = "data-io"
        item.pop("capability_ids", None)
        item.pop("reason", None)
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "release target is incomplete"
        ):
            self.validate(document, release=True)

    def test_release_gate_accepts_only_available_catalog_mappings(self) -> None:
        document = copy.deepcopy(self.coverage)
        for item in document["items"]:
            if not item["target_v1"]:
                continue
            item["disposition"] = "implemented"
            item["capability_ids"] = ["dataset.inspect.v1"]
            item["evidence"] = [
                "catalog:dataset.inspect.v1",
                *[
                    evidence
                    for evidence in item["evidence"]
                    if evidence.startswith("offline-reference:")
                ],
            ]
            item.pop("owner", None)
            item.pop("reason", None)
        summary = self.validate(document, release=True)
        self.assertEqual(summary.achieved_weight, 80)

    def test_declared_target_below_eighty_is_rejected(self) -> None:
        document = copy.deepcopy(self.coverage)
        document["release_target"]["minimum_weight"] = 79
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "release target must be"
        ):
            self.validate(document)

    def test_actual_weight_drift_is_rejected(self) -> None:
        document = copy.deepcopy(self.coverage)
        self.item(document, "sequence.stats")["weight"] = 2
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "actual domain weights"
        ):
            self.validate(document)

    def test_invalid_disposition_is_rejected(self) -> None:
        document = copy.deepcopy(self.coverage)
        self.item(document, "table.manipulate")["disposition"] = "almost-done"
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "invalid disposition"
        ):
            self.validate(document)

    def test_planned_item_requires_owner(self) -> None:
        document = copy.deepcopy(self.coverage)
        self.item(document, "alignment.coverage").pop("owner")
        with self.assertRaisesRegex(coverage_v1.CoverageValidationError, "owner"):
            self.validate(document)

    def test_excluded_item_cannot_be_a_target(self) -> None:
        document = copy.deepcopy(self.coverage)
        item = self.item(document, "alignment.coverage")
        item["disposition"] = "excluded-with-reason"
        item["reason"] = "outside the supported research scope"
        item.pop("owner")
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "excluded item cannot"
        ):
            self.validate(document)

    def test_implemented_item_cannot_map_to_planned_capability(self) -> None:
        document = copy.deepcopy(self.coverage)
        item = self.item(document, "sequence.stats")
        item["capability_ids"] = ["protein.af2.predict.v1"]
        item["evidence"] = ["catalog:protein.af2.predict.v1"]
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "non-available capability"
        ):
            self.validate(document)

    def test_implemented_item_requires_catalog_evidence(self) -> None:
        document = copy.deepcopy(self.coverage)
        self.item(document, "sequence.stats")["evidence"] = [
            "scope:v1-biological-coverage-plan"
        ]
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "lacks catalog evidence"
        ):
            self.validate(document)

    def test_unknown_capability_mapping_is_rejected(self) -> None:
        document = copy.deepcopy(self.coverage)
        item = self.item(document, "sequence.stats")
        item["capability_ids"] = ["missing.capability.v1"]
        item["evidence"] = ["catalog:missing.capability.v1"]
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "unknown capability"
        ):
            self.validate(document)

    def test_every_offline_reference_fact_requires_a_known_mapping(self) -> None:
        inventory = copy.deepcopy(self.inventory)
        self.feature(inventory, "offline-reference-2.475.hf.009")["coverage_ids"] = [
            "missing.coverage"
        ]
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "unknown coverage item"
        ):
            self.validate(inventory=inventory)

    def test_reference_inventory_cannot_be_silently_shrunk(self) -> None:
        inventory = copy.deepcopy(self.inventory)
        inventory["observed_features"].pop()
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "feature count"
        ):
            self.validate(inventory=inventory)

    def test_reference_mapping_requires_bidirectional_evidence(self) -> None:
        document = copy.deepcopy(self.coverage)
        item = self.item(document, "sequence.stats")
        item["evidence"] = ["catalog:sequence.stats.v1"]
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "lacks evidence for reference"
        ):
            self.validate(document)

    def test_offline_reference_fact_cannot_map_as_cloud_only(self) -> None:
        document = copy.deepcopy(self.coverage)
        self.item(document, "sequence.extract")["execution"] = "cloud"
        with self.assertRaisesRegex(
            coverage_v1.CoverageValidationError, "maps only as non-local"
        ):
            self.validate(document)

    def test_duplicate_json_keys_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.json"
            path.write_text('{"schema_version":"1","schema_version":"2"}', encoding="utf-8")
            with self.assertRaisesRegex(
                coverage_v1.CoverageValidationError, "duplicate JSON key"
            ):
                coverage_v1.load_json(path)


if __name__ == "__main__":
    unittest.main()
