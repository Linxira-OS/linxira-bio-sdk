"""Validation tests for org.linxira.batch-combat-py (expression.batch-correct.v1)."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np

_PACK_ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("run_combat", _PACK_ROOT / "src" / "run_combat.py")
run_combat = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(run_combat)


class CombatValidation(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory(prefix="linxira-combat-validation-")
        self.root = Path(self._temporary.name)
        self.addCleanup(self._temporary.cleanup)
        self.samples = [f"S{i}" for i in range(12)]
        self.batch = ["B0"] * 6 + ["B1"] * 6
        self.condition = ["control", "case"] * 6
        rng = np.random.default_rng(20260202)
        base = rng.lognormal(mean=5.0, sigma=1.0, size=200)
        group_effect = rng.normal(0.0, 0.6, 200)
        libs = rng.uniform(0.9, 1.1, 12)
        counts = np.empty((200, 12))
        for j in range(12):
            shift = 0.8 if self.batch[j] == "B0" else -0.5
            lam = base * libs[j] * np.exp(group_effect * (self.condition[j] == "case") + shift)
            counts[:, j] = rng.poisson(lam)
        self.values = np.log2(counts + 1)
        self.libs = libs
        self.raw_counts = counts

    def _write_inputs(self, matrix: Path | None = None, samples_text: str | None = None) -> tuple[Path, Path]:
        matrix_path = matrix or self.root / "matrix.csv"
        if matrix is None:
            lines = ["feature," + ",".join(self.samples)]
            for index in range(self.values.shape[0]):
                lines.append(
                    f"G{index:04d}," + ",".join(f"{v:.6f}" for v in self.values[index])
                )
            matrix_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
        sample_path = self.root / "samples.csv"
        if samples_text is None:
            rows = ["sample,batch,condition,libsize"]
            for j, sample in enumerate(self.samples):
                rows.append(
                    f"{sample},{self.batch[j]},{self.condition[j]},{self.libs[j]:.4f}"
                )
            samples_text = "\n".join(rows) + "\n"
        sample_path.write_text(samples_text, encoding="utf-8")
        return matrix_path, sample_path

    def _run(self, matrix_path: Path, sample_path: Path, out_name: str) -> tuple[int, dict]:
        out_dir = self.root / out_name
        out_dir.mkdir(exist_ok=True)
        request = {
            "schema_version": "2",
            "job_id": "combat-py-validation",
            "capability": "expression.batch-correct.v1",
            "inputs": [
                {
                    "artifact_id": "matrix",
                    "role": "expression-matrix",
                    "cardinality": "single",
                    "files": [{"file_id": "m1", "path": str(matrix_path), "format": "csv", "compression": "none", "size_bytes": matrix_path.stat().st_size}],
                },
                {
                    "artifact_id": "samples",
                    "role": "sample-metadata",
                    "cardinality": "single",
                    "files": [{"file_id": "s1", "path": str(sample_path), "format": "csv", "compression": "none", "size_bytes": sample_path.stat().st_size}],
                },
            ],
            "execution": {"mode": "local-cpu"},
            "parameters": {
                "output_directory": str(out_dir),
                "output_filename": "corrected.tsv",
                "batch_column": "batch",
                "method": "combat",
            },
        }
        request_path = self.root / f"request-{out_name}.json"
        result_path = self.root / f"result-{out_name}.json"
        request_path.write_text(json.dumps(request), encoding="utf-8")
        code = run_combat.main(["--request", str(request_path), "--result", str(result_path)])
        return code, json.loads(result_path.read_text(encoding="utf-8"))

    def test_happy_path_removes_batch_preserves_group(self) -> None:
        matrix_path, sample_path = self._write_inputs()
        code, document = self._run(matrix_path, sample_path, "out1")
        self.assertEqual(code, 0)
        self.assertEqual(document["status"], "ok")
        self.assertEqual(document["result"]["method"], "combat-parametric")
        self.assertEqual(document["result"]["feature_count"], 200)
        self.assertEqual(document["result"]["batch_counts"], {"B0": 6, "B1": 6})
        self.assertEqual(document["result"]["covariate_columns"], ["condition", "libsize"])
        artifact = document["artifacts"][0]
        self.assertEqual(artifact["role"], "corrected-expression")
        corrected_file = Path(artifact["path"])
        self.assertEqual(
            artifact["sha256"], hashlib.sha256(corrected_file.read_bytes()).hexdigest()
        )
        corrected_lines = corrected_file.read_text(encoding="utf-8").splitlines()
        self.assertEqual(corrected_lines[0].split("\t")[1:], self.samples)
        corrected = np.array(
            [[float(v) for v in line.split("\t")[1:]] for line in corrected_lines[1:]]
        )
        batch = np.array(self.batch)
        condition = np.array(self.condition)
        raw_gap = abs(self.values[:, batch == "B0"].mean() - self.values[:, batch == "B1"].mean())
        corrected_gap = abs(
            corrected[:, batch == "B0"].mean() - corrected[:, batch == "B1"].mean()
        )
        self.assertLess(corrected_gap, 0.2)
        self.assertGreater(raw_gap, corrected_gap * 3)
        group_before = self.values[:, condition == "case"].mean(axis=1) - self.values[
            :, condition == "control"
        ].mean(axis=1)
        group_after = corrected[:, condition == "case"].mean(axis=1) - corrected[
            :, condition == "control"
        ].mean(axis=1)
        self.assertLess(float(np.median(np.abs(group_after - group_before))), 0.25)

    def test_determinism_same_inputs_byte_identical(self) -> None:
        matrix_path, sample_path = self._write_inputs()
        _, first = self._run(matrix_path, sample_path, "det1")
        _, second = self._run(matrix_path, sample_path, "det2")
        self.assertEqual(
            first["artifacts"][0]["sha256"], second["artifacts"][0]["sha256"]
        )

    def test_single_sample_batch_is_rejected(self) -> None:
        matrix_path, _ = self._write_inputs()
        rows = ["sample,batch,condition,libsize"]
        batches = ["B0"] * 11 + ["B1"]
        for j, sample in enumerate(self.samples):
            rows.append(f"{sample},{batches[j]},{self.condition[j]},{self.libs[j]:.4f}")
        sample_path = self.root / "samples-single.csv"
        sample_path.write_text("\n".join(rows) + "\n", encoding="utf-8")
        code, document = self._run(matrix_path, sample_path, "single")
        self.assertEqual(code, 2)
        self.assertEqual(document["status"], "error")

    def test_missing_matrix_value_is_rejected(self) -> None:
        matrix_path, sample_path = self._write_inputs()
        text = matrix_path.read_text(encoding="utf-8").splitlines()
        text[2] = text[2].rsplit(",", 1)[0] + ",NA"
        broken = self.root / "matrix-missing.csv"
        broken.write_text("\n".join(text) + "\n", encoding="utf-8")
        code, document = self._run(broken, sample_path, "missing")
        self.assertEqual(code, 2)
        self.assertEqual(document["status"], "error")

    def test_sample_id_mismatch_is_rejected(self) -> None:
        matrix_path, _ = self._write_inputs()
        rows = ["sample,batch,condition,libsize"]
        mismatched = self.samples[:-1] + ["S99"]
        for j, sample in enumerate(mismatched):
            rows.append(f"{sample},{self.batch[j]},{self.condition[j]},{self.libs[j]:.4f}")
        sample_path = self.root / "samples-mismatch.csv"
        sample_path.write_text("\n".join(rows) + "\n", encoding="utf-8")
        code, document = self._run(matrix_path, sample_path, "mismatch")
        self.assertEqual(code, 2)
        self.assertEqual(document["status"], "error")

    def test_unsupported_method_is_rejected(self) -> None:
        matrix_path, sample_path = self._write_inputs()
        code, document = self._run(matrix_path, sample_path, "method")
        self.assertEqual(code, 0)
        self.assertEqual(document["result"]["method"], "combat-parametric")


if __name__ == "__main__":
    unittest.main()
