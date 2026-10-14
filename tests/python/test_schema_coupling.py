"""The report and the summary must accept the same environment block.

The summary embeds the first report's environment verbatim
(`scripts/benchmark/analysis/summarize-reports.py`), and both schemas set
`additionalProperties: false`. So a key added to one schema and not the other
does not fail at measurement time - it fails when the window's archive is
validated, i.e. after the machine time has been spent. That happened on
2026-10-13: `code_branch` / `code_describe` reached every report and were absent
from the summary schema, which would have rejected the archive the 8-hour window
produced. These tests make the two schemas move together.
"""

import json
import pathlib
import unittest

REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
REPORT_SCHEMA = REPO_ROOT / "schemas" / "benchmark-report.schema.json"
SUMMARY_SCHEMA = REPO_ROOT / "schemas" / "benchmark-summary.schema.json"


def environment_properties(path: pathlib.Path) -> dict:
    schema = json.loads(path.read_text(encoding="utf-8"))
    return schema["properties"]["environment"]["properties"]


def sample_for(property_schema: dict):
    """A type-correct placeholder, so the schema's own types are exercised."""
    declared = property_schema.get("type")
    types = declared if isinstance(declared, list) else [declared]
    if "null" in types:
        return None
    if "string" in types:
        return "x"
    if "integer" in types:
        return 1
    if "number" in types:
        return 1.0
    if "boolean" in types:
        return True
    raise AssertionError(f"no sample value for type {declared!r}")


class SchemaCouplingTests(unittest.TestCase):
    def test_summary_accepts_every_report_environment_key(self) -> None:
        report_keys = environment_properties(REPORT_SCHEMA)
        summary_keys = environment_properties(SUMMARY_SCHEMA)
        missing = sorted(set(report_keys) - set(summary_keys))
        self.assertEqual(
            missing,
            [],
            "the summary copies a report's environment verbatim; these keys would "
            f"make every archived window summary invalid: {missing}",
        )

    def test_environment_blocks_validate_across_both_schemas(self) -> None:
        try:
            import jsonschema  # type: ignore
        except ImportError:  # pragma: no cover - jsonschema ships with the CI image
            self.skipTest("jsonschema is not installed")
        report_env = environment_properties(REPORT_SCHEMA)
        summary_env_schema = {"type": "object", "additionalProperties": False,
                             "properties": environment_properties(SUMMARY_SCHEMA)}
        # A block carrying every report key must satisfy the summary too, and each
        # value must satisfy the summary's declared type for that key.
        block = {key: sample_for(schema) for key, schema in report_env.items()}
        jsonschema.validate(instance=block, schema=summary_env_schema)


if __name__ == "__main__":
    unittest.main()


def methodology_properties() -> dict:
    schema = json.loads(SUMMARY_SCHEMA.read_text(encoding="utf-8"))
    return schema["properties"]["methodology"]["properties"]


class DescriptorMethodologyTest(unittest.TestCase):
    """A window descriptor's methodology block must fit the summary schema.

    `summarize-reports.py` copies `descriptor["methodology"]` verbatim into the
    summary, and the summary schema sets `additionalProperties: false`. So a key
    added to a descriptor and not to the schema is discovered by the repository
    validator *after* the archive has been built - which is what happened on
    2026-10-13: the descriptor carried `code_label` and the schema did not, and the
    archive the measuring host produced was rejected on the way in. Same failure
    mode as `code_branch` / `code_describe` above, one stage further down the chain.

    This walks the committed descriptors rather than a sample, so the guard covers
    every window that has been run.
    """

    def descriptors(self) -> list[pathlib.Path]:
        found = sorted((REPO_ROOT / "scripts" / "benchmark" / "windows").glob("*/descriptor.json"))
        self.assertTrue(found, "no window descriptors found")
        return found

    def test_descriptor_methodology_keys_are_allowed(self):
        allowed = set(methodology_properties())
        for path in self.descriptors():
            methodology = json.loads(path.read_text(encoding="utf-8")).get("methodology") or {}
            unexpected = sorted(set(methodology) - allowed)
            self.assertEqual(
                [],
                unexpected,
                f"{path.relative_to(REPO_ROOT)} writes methodology keys the summary schema "
                f"rejects: {unexpected}",
            )

    def test_descriptor_methodology_carries_every_required_key(self):
        required = set(
            json.loads(SUMMARY_SCHEMA.read_text(encoding="utf-8"))["properties"]["methodology"]["required"]
        )
        for path in self.descriptors():
            methodology = json.loads(path.read_text(encoding="utf-8")).get("methodology") or {}
            missing = sorted(required - set(methodology))
            self.assertEqual(
                [],
                missing,
                f"{path.relative_to(REPO_ROOT)} is missing required methodology keys: {missing}",
            )
