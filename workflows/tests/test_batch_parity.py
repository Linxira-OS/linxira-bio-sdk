"""Cross-backend parity: org.linxira.batch-combat-py vs org.linxira.batch-combat-r.

Runs both pack entrypoints on identical synthetic inputs and asserts the
corrected matrices are byte-identical. Skips when Rscript or NumPy is
unavailable so the generic workflows-tests discovery (which runs in the
dependency-free CI venv) stays green; the dedicated batch-combat CI steps
run it with both runtimes present.

Standalone: python3 workflows/tests/test_batch_parity.py
"""

from __future__ import annotations

import json

import os
import shutil
import sys
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PY_PACK = REPO_ROOT / "workflows" / "org.linxira.batch-combat-py"
R_PACK = REPO_ROOT / "workflows" / "org.linxira.batch-combat-r"

try:
    import numpy as np

    HAS_NUMPY = True
except ImportError:
    HAS_NUMPY = False

HAS_R = shutil.which("Rscript") is not None


def build_inputs(root: Path) -> tuple[Path, Path]:
    """Deterministic expression matrix with a planted batch shift and group effect."""
    rng = np.random.default_rng(20261002)
    n_features, per_batch = 150, 5
    base = rng.lognormal(mean=5.0, sigma=1.0, size=n_features)
    group_effect = rng.normal(0.0, 0.6, n_features)
    samples = [f"S{i}" for i in range(per_batch * 2)]
    batch = ["B0"] * per_batch + ["B1"] * per_batch
    condition = ["control", "case"] * per_batch
    libs = rng.uniform(0.9, 1.1, len(samples))
    counts = np.empty((n_features, len(samples)))
    for j, sample in enumerate(samples):
        shift = 0.8 if batch[j] == "B0" else -0.5
        lam = base * libs[j] * np.exp(group_effect * (condition[j] == "case") + shift)
        counts[:, j] = rng.poisson(lam)
    matrix = root / "matrix.csv"
    lines = ["feature," + ",".join(samples)]
    for index in range(n_features):
        lines.append(
            f"G{index:04d}," + ",".join(f"{v:.6f}" for v in np.log2(counts[index] + 1))
        )
    matrix.write_text("\n".join(lines) + "\n", encoding="utf-8")
    table = root / "samples.csv"
    rows = ["sample,batch,condition"]
    for j, sample in enumerate(samples):
        rows.append(f"{sample},{batch[j]},{condition[j]}")
    table.write_text("\n".join(rows) + "\n", encoding="utf-8")
    return matrix, table


def run_backend(
    pack_root: Path,
    interpreter: str,
    script: str,
    matrix: Path,
    table: Path,
    out_dir: Path,
) -> bytes:
    request = {
        "schema_version": "2",
        "job_id": "parity",
        "capability": "expression.batch-correct.v1",
        "inputs": [
            {
                "artifact_id": "matrix",
                "role": "expression-matrix",
                "cardinality": "single",
                "files": [{"file_id": "m1", "path": str(matrix), "format": "csv", "compression": "none", "size_bytes": matrix.stat().st_size}],
            },
            {
                "artifact_id": "samples",
                "role": "sample-metadata",
                "cardinality": "single",
                "files": [{"file_id": "s1", "path": str(table), "format": "csv", "compression": "none", "size_bytes": table.stat().st_size}],
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
    request_path = out_dir / "request.json"
    result_path = out_dir / "result.json"
    out_dir.mkdir(parents=True, exist_ok=True)
    request_path.write_text(json.dumps(request), encoding="utf-8")
    subprocess.run(
        [interpreter, str(pack_root / "src" / script), "--request", str(request_path), "--result", str(result_path)],
        check=True,
        capture_output=True,
    )
    document = json.loads(result_path.read_text(encoding="utf-8"))
    assert document["status"] == "ok", document.get("diagnostics")
    return (out_dir / "corrected.tsv").read_bytes()


@unittest.skipUnless(HAS_NUMPY, "NumPy unavailable")
@unittest.skipUnless(HAS_R, "Rscript unavailable")
class TestCrossBackendParity(unittest.TestCase):
    def test_corrected_matrices_are_byte_identical(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="linxira-combat-parity-")
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        matrix, table = build_inputs(root)
        python_interpreter = os.environ.get("LINXIRA_BIO_WORKFLOW_PYTHON")
        if python_interpreter:
            candidates = [
                Path(python_interpreter),
                Path(python_interpreter) / "python.exe",
                Path(python_interpreter) / "Scripts" / "python.exe",
                Path(python_interpreter) / "bin" / "python",
            ]
            python_interpreter = next((str(c) for c in candidates if c.is_file()), None)
        python_interpreter = python_interpreter or sys.executable
        assert python_interpreter is not None
        py_bytes = run_backend(PY_PACK, python_interpreter, "run_combat.py", matrix, table, root / "py")
        r_bytes = run_backend(R_PACK, "Rscript", "run_combat.R", matrix, table, root / "r")
        self.assertEqual(py_bytes, r_bytes, "backends must emit byte-identical corrected matrices")
        self.assertTrue(py_bytes.splitlines()[0].startswith(b"feature\tS0\t"), "unexpected matrix header")


if __name__ == "__main__":
    unittest.main()
