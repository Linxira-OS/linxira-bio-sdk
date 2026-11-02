from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sdk" / "python"))

import linxira_bio
from linxira_bio.dynamic import DynamicAPI


class FakeClient:
    def execute(self, capability, inputs, parameters=None):
        return {
            "capability": capability,
            "inputs": inputs,
            "parameters": parameters or {},
        }


class DynamicApiBindingTests(unittest.TestCase):
    def test_generated_method_is_bound_to_the_instance(self) -> None:
        api = DynamicAPI(FakeClient())

        result = api.sequence_stats(fasta="input.fa")

        self.assertEqual(result["capability"], "sequence.stats.v1")
        self.assertEqual(result["inputs"], {"fasta": "input.fa"})

    def test_packaged_catalog_matches_repository_catalog(self) -> None:
        canonical = ROOT / "capabilities" / "catalog.json"
        packaged = Path(linxira_bio.__file__).resolve().parent / "data" / "catalog.json"

        self.assertTrue(canonical.is_file())
        self.assertTrue(packaged.is_file())
        self.assertEqual(
            json.loads(canonical.read_text(encoding="utf-8")),
            json.loads(packaged.read_text(encoding="utf-8")),
        )


if __name__ == "__main__":
    unittest.main()
