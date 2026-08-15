import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

PACK_ROOT = Path(__file__).resolve().parents[1]

SDF = """molecule1
  RDKit          2D

  5  4  0  0  0  0  0  0  0  0999 V2000
    0.0000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    1.2000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    2.4000    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    1.2000    1.2000    0.0000 N   0  0  0  0  0  0  0  0  0  0  0  0
    1.2000   -1.2000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0
  1  2  1  0
  2  3  1  0
  2  4  1  0
  2  5  2  0
M  END
$$$$
"""


class DescriptorPackTests(unittest.TestCase):
    def run_pack(self, request: dict) -> dict:
        with tempfile.TemporaryDirectory(prefix="linxira-descriptors-") as temporary:
            root = Path(temporary)
            request_path = root / "request.json"
            result_path = root / "result.json"
            request_path.write_text(json.dumps(request), encoding="utf-8")
            process = subprocess.run(
                [
                    sys.executable,
                    str(PACK_ROOT / "src" / "descriptors.py"),
                    "--request",
                    str(request_path),
                    "--result",
                    str(result_path),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertTrue(result_path.is_file())
            envelope = json.loads(result_path.read_text(encoding="utf-8"))
            return envelope, result_path

    def test_computes_descriptors_for_sdf_input(self):
        with tempfile.TemporaryDirectory(prefix="linxira-descriptors-in-") as temporary:
            input_path = Path(temporary) / "molecules.sdf"
            input_path.write_text(SDF, encoding="utf-8")
            output_path = Path(temporary) / "out" / "descriptors.tsv"
            request = {
                "schema_version": "2",
                "job_id": "descriptors-test",
                "capability": "chemistry.descriptors.v1",
                "inputs": [
                    {
                        "artifact_id": "molecules",
                        "role": "molecules",
                        "cardinality": "single",
                        "files": [
                            {
                                "file_id": "molecules-1",
                                "path": str(input_path),
                                "format": "sdf",
                                "compression": "none",
                                "size_bytes": input_path.stat().st_size,
                            }
                        ],
                    }
                ],
                "execution": {"mode": "local-cpu"},
                "parameters": {
                    "output_directory": str(output_path.parent),
                    "output_filename": output_path.name,
                },
            }
            envelope, _ = self.run_pack(request)
            self.assertEqual(envelope["status"], "ok")
            self.assertEqual(envelope["capability"], "chemistry.descriptors.v1")
            self.assertEqual(envelope["result"]["molecule_count"], 1)
            self.assertTrue(output_path.is_file())
            table = output_path.read_text(encoding="utf-8")
            self.assertTrue(table.startswith("molecule_index\tmolecular_weight\tlogp"))
            self.assertIn("formula", table)
            rows = table.strip().splitlines()
            self.assertEqual(len(rows), 2)

    def test_rejects_missing_output_parameter(self):
        with tempfile.TemporaryDirectory(prefix="linxira-descriptors-in-") as temporary:
            input_path = Path(temporary) / "molecules.sdf"
            input_path.write_text(SDF, encoding="utf-8")
            request = {
                "schema_version": "2",
                "job_id": "descriptors-error",
                "capability": "chemistry.descriptors.v1",
                "inputs": [
                    {
                        "artifact_id": "molecules",
                        "role": "molecules",
                        "cardinality": "single",
                        "files": [
                            {
                                "file_id": "molecules-1",
                                "path": str(input_path),
                                "format": "sdf",
                                "compression": "none",
                                "size_bytes": input_path.stat().st_size,
                            }
                        ],
                    }
                ],
                "execution": {"mode": "local-cpu"},
                "parameters": {},
            }
            envelope, _ = self.run_pack(request)
            self.assertEqual(envelope["status"], "error")
            self.assertEqual(envelope["diagnostics"][0]["code"], "workflow_failed")
            self.assertIn("output", envelope["diagnostics"][0]["message"])


if __name__ == "__main__":
    unittest.main()
