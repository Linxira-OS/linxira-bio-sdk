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
   -1.8187   -0.7500    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
   -0.5196    0.0000    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
   -0.5196    1.5000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0
    0.7794   -0.7500    0.0000 C   0  0  0  0  0  0  0  0  0  0  0  0
    2.0785   -0.0000    0.0000 N   0  0  0  0  0  0  0  0  0  0  0  0
  1  2  1  0
  2  3  2  0
  2  4  1  0
  4  5  1  0
M  END
$$$$
"""


class ConformerPackTests(unittest.TestCase):
    def run_pack(self, request: dict) -> dict:
        with tempfile.TemporaryDirectory(prefix="linxira-conformers-") as temporary:
            root = Path(temporary)
            request_path = root / "request.json"
            result_path = root / "result.json"
            request_path.write_text(json.dumps(request), encoding="utf-8")
            process = subprocess.run(
                [
                    sys.executable,
                    str(PACK_ROOT / "src" / "conformers.py"),
                    "--request",
                    str(request_path),
                    "--result",
                    str(result_path),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertTrue(result_path.is_file(), process.stderr)
            envelope = json.loads(result_path.read_text(encoding="utf-8"))
            # Worker contract: an error envelope is reported with a nonzero
            # process exit; only an ok envelope must exit 0.
            if envelope.get("status") == "ok":
                self.assertEqual(process.returncode, 0, process.stderr)
            return envelope, result_path

    def build_request(self, temporary: str, parameters: dict) -> dict:
        input_path = Path(temporary) / "molecules.sdf"
        input_path.write_text(SDF, encoding="utf-8")
        return {
            "schema_version": "2",
            "job_id": "conformers-test",
            "capability": "chemistry.conformers.v1",
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
            "parameters": parameters,
        }

    def test_generates_reproducible_conformers_for_sdf_input(self):
        with tempfile.TemporaryDirectory(prefix="linxira-conformers-in-") as temporary:
            output_path = Path(temporary) / "out" / "conformers.sdf"
            parameters = {
                "output_directory": str(output_path.parent),
                "output_filename": output_path.name,
                "num_conformers": 3,
                "seed": 42,
            }
            envelope, _ = self.run_pack(self.build_request(temporary, parameters))
            self.assertEqual(envelope["status"], "ok")
            self.assertEqual(envelope["capability"], "chemistry.conformers.v1")
            self.assertEqual(envelope["result"]["molecule_count"], 1)
            self.assertEqual(envelope["result"]["conformer_count"], 3)
            per_molecule = envelope["result"]["per_molecule"][0]
            self.assertEqual(per_molecule["conformer_count"], 3)
            self.assertLess(per_molecule["energy_min_kcal_mol"], per_molecule["energy_max_kcal_mol"] + 1e-9)
            self.assertTrue(output_path.is_file())
            records = output_path.read_text(encoding="utf-8").split("$$$$")
            records = [record for record in records if record.strip()]
            self.assertEqual(len(records), 3)
            self.assertIn("mmff_energy_kcal_mol", records[0])
            artifact = envelope["artifacts"][0]
            self.assertEqual(artifact["role"], "conformers")
            self.assertEqual(artifact["format"], "sdf")

            # A second run with the same seed must reproduce the same SDF
            # bytes; ETKDGv3 with a fixed seed is deterministic.
            with tempfile.TemporaryDirectory(prefix="linxira-conformers-repeat-") as repeat:
                repeat_path = Path(repeat) / "out" / "conformers.sdf"
                parameters_repeat = dict(parameters)
                parameters_repeat["output_directory"] = str(repeat_path.parent)
                parameters_repeat["output_filename"] = repeat_path.name
                envelope_repeat, _ = self.run_pack(
                    self.build_request(temporary, parameters_repeat)
                )
                self.assertEqual(envelope_repeat["status"], "ok")
                self.assertEqual(
                    envelope_repeat["artifacts"][0]["sha256"],
                    artifact["sha256"],
                )

    def test_rms_prune_can_reduce_conformer_count(self):
        with tempfile.TemporaryDirectory(prefix="linxira-conformers-prune-") as temporary:
            output_path = Path(temporary) / "out" / "conformers.sdf"
            parameters = {
                "output_directory": str(output_path.parent),
                "output_filename": output_path.name,
                "num_conformers": 5,
                "seed": 7,
                "rms_prune_threshold": 1.5,
            }
            envelope, _ = self.run_pack(self.build_request(temporary, parameters))
            self.assertEqual(envelope["status"], "ok")
            self.assertLessEqual(envelope["result"]["conformer_count"], 5)

    def test_rejects_out_of_range_num_conformers(self):
        with tempfile.TemporaryDirectory(prefix="linxira-conformers-bad-") as temporary:
            parameters = {
                "output_directory": temporary,
                "output_filename": "conformers.sdf",
                "num_conformers": 0,
            }
            envelope, _ = self.run_pack(self.build_request(temporary, parameters))
            self.assertEqual(envelope["status"], "error")
            self.assertEqual(envelope["diagnostics"][0]["code"], "workflow_failed")
            self.assertIn("num_conformers", envelope["diagnostics"][0]["message"])

    def test_rejects_missing_output_parameter(self):
        with tempfile.TemporaryDirectory(prefix="linxira-conformers-missing-") as temporary:
            request = self.build_request(temporary, {})
            del request["parameters"]["output_directory"]
            envelope, _ = self.run_pack(request)
            self.assertEqual(envelope["status"], "error")
            self.assertIn("output_directory", envelope["diagnostics"][0]["message"])


if __name__ == "__main__":
    unittest.main()
