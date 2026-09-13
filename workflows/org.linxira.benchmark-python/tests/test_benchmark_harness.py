"""Smoke and parity tests for the Python benchmark harness (M2-T3).

Run with ``python -m unittest discover -s workflows/org.linxira.benchmark-python/tests``.
Biopython-dependent cases are skipped when the library is not installed so the
suite stays runnable on a bare interpreter; CI installs the pack lock.
"""

from __future__ import annotations

import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path

try:
    import Bio  # type: ignore  # noqa: F401

    HAVE_BIOPYTHON = True
except ImportError:
    HAVE_BIOPYTHON = False

try:
    from jsonschema import Draft202012Validator  # type: ignore

    HAVE_JSONSCHEMA = True
except ImportError:
    HAVE_JSONSCHEMA = False

PACK_ROOT = Path(__file__).resolve().parents[1]
SCRIPT = PACK_ROOT / "src" / "benchmark_harness.py"
INPUT_SCHEMA = PACK_ROOT / "schemas" / "input.schema.json"
OUTPUT_SCHEMA = PACK_ROOT / "schemas" / "output.schema.json"
REPOSITORY_ROOT = PACK_ROOT.parents[1]
TINY_FASTA = REPOSITORY_ROOT / "tests" / "fixtures" / "sequences" / "tiny.fa"

SPEC = importlib.util.spec_from_file_location("benchmark_harness", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

# `linxira-bio sequence stats tests/fixtures/sequences/tiny.fa --json` (Rust
# engine 1.0.1); the Python implementation must agree within 1e-6 relative.
RUST_TINY_RESULT = {
    "sequence_count": 3,
    "total_bases": 12,
    "min_length": 2,
    "max_length": 6,
    "mean_length": 4.0,
    "n50": 6,
    "l50": 1,
    "au_n": 4.666666666666667,
    "gc_percent": 60.0,
    "n_count": 2,
    "n_percent": 16.666666666666664,
}


def sha256_of(path: Path) -> str:
    return MODULE.sha256_file(path)


def make_request(fasta: Path, output_directory: Path, **overrides) -> dict:
    request = {
        "schema_version": "2",
        "job_id": "benchmark-sequence-stats-v1",
        "capability": "sequence.stats.v1",
        "inputs": [
            {
                "artifact_id": "input-fasta",
                "role": "fasta",
                "cardinality": "single",
                "files": [
                    {
                        "file_id": "input-fasta-1",
                        "path": str(fasta),
                        "format": "fasta",
                        "compression": "none",
                        "size_bytes": fasta.stat().st_size,
                        "sha256": sha256_of(fasta),
                    }
                ],
            }
        ],
        "execution": {"mode": "local-cpu", "backend": "python"},
        "parameters": {"output_directory": str(output_directory)},
    }
    request.update(overrides)
    return request


def run_harness(request: dict, workspace: Path) -> tuple[int, dict, str]:
    request_path = workspace / "request.json"
    request_path.write_text(json.dumps(request), encoding="utf-8")
    output_directory = Path(request["parameters"]["output_directory"])
    result_path = output_directory / "result.json"
    stderr = io.StringIO()
    with redirect_stderr(stderr):
        code = MODULE.main(["--request", str(request_path), "--result", str(result_path)])
    envelope = json.loads(result_path.read_text(encoding="utf-8"))
    return code, envelope, stderr.getvalue()


def assert_close(test: unittest.TestCase, expected: float, actual: float, field: str) -> None:
    scale = max(abs(expected), abs(actual))
    if scale == 0.0:
        test.assertEqual(expected, actual, field)
        return
    test.assertLessEqual(abs(expected - actual) / scale, 1e-6, f"{field}: {expected} vs {actual}")


class RegistryTests(unittest.TestCase):
    def test_registry_exposes_sequence_stats_with_the_rust_contract(self):
        implementation = MODULE.IMPLEMENTATIONS["sequence.stats.v1"]
        self.assertEqual(implementation.INPUT_ROLES, ("fasta",))
        self.assertEqual(implementation.PARAMETERS, ())

    def test_input_schema_accepts_a_worker_style_request(self):
        if not HAVE_JSONSCHEMA:
            self.skipTest("jsonschema is not installed")
        validator = Draft202012Validator(json.loads(INPUT_SCHEMA.read_text(encoding="utf-8")))
        request = make_request(TINY_FASTA, Path(tempfile.gettempdir()) / "unused")
        self.assertEqual(sorted(validator.iter_errors(request), key=str), [])


class ValidationTests(unittest.TestCase):
    def test_rejects_unknown_capability_with_an_error_envelope(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            request = make_request(TINY_FASTA, workspace / "out", capability="sequence.orf.v1")
            code, envelope, stderr = run_harness(request, workspace)
        self.assertEqual(code, 2)
        self.assertEqual(envelope["status"], "error")
        self.assertEqual(envelope["capability"], "sequence.orf.v1")
        self.assertEqual(envelope["job_id"], "benchmark-sequence-stats-v1")
        self.assertIn("no python implementation", stderr)
        self.assertEqual(envelope["diagnostics"][0]["severity"], "error")

    def test_rejects_a_request_addressed_to_another_backend(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            request = make_request(TINY_FASTA, workspace / "out")
            request["execution"]["backend"] = "r"
            code, envelope, _ = run_harness(request, workspace)
        self.assertEqual(code, 2)
        self.assertIn("execution.backend", envelope["diagnostics"][0]["message"])

    def test_rejects_parameters_the_native_contract_does_not_accept(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            request = make_request(TINY_FASTA, workspace / "out")
            request["parameters"]["min_length"] = 10
            code, envelope, _ = run_harness(request, workspace)
        self.assertEqual(code, 2)
        self.assertIn("does not accept parameter min_length", envelope["diagnostics"][0]["message"])

    def test_rejects_a_result_path_outside_the_output_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            request = make_request(TINY_FASTA, workspace / "out")
            request_path = workspace / "request.json"
            request_path.write_text(json.dumps(request), encoding="utf-8")
            stray = workspace / "elsewhere" / "result.json"
            stray.parent.mkdir()
            with redirect_stderr(io.StringIO()):
                code = MODULE.main(["--request", str(request_path), "--result", str(stray)])
            envelope = json.loads(stray.read_text(encoding="utf-8"))
        self.assertEqual(code, 2)
        self.assertIn("output_directory", envelope["diagnostics"][0]["message"])


@unittest.skipUnless(HAVE_BIOPYTHON, "Biopython is not installed")
class ParityTests(unittest.TestCase):
    def test_tiny_fixture_matches_the_rust_engine_within_tolerance(self):
        implementation = MODULE.IMPLEMENTATIONS["sequence.stats.v1"]
        result = implementation.run({"fasta": TINY_FASTA}, {})
        self.assertEqual(set(result), set(RUST_TINY_RESULT))
        for field, expected in RUST_TINY_RESULT.items():
            if isinstance(expected, int):
                self.assertEqual(result[field], expected, field)
            else:
                assert_close(self, expected, result[field], field)

    def test_gzip_input_is_read_by_magic_bytes(self):
        import gzip

        implementation = MODULE.IMPLEMENTATIONS["sequence.stats.v1"]
        with tempfile.TemporaryDirectory() as directory:
            compressed = Path(directory) / "reads.data"
            with gzip.open(compressed, "wb") as handle:
                handle.write(b">one\nACGT\n>two\nNN\n")
            result = implementation.run({"fasta": compressed}, {})
        self.assertEqual(result["sequence_count"], 2)
        self.assertEqual(result["total_bases"], 6)
        self.assertEqual(result["n_count"], 2)
        assert_close(self, 50.0, result["gc_percent"], "gc_percent")

    def test_engine_error_cases_are_reproduced(self):
        implementation = MODULE.IMPLEMENTATIONS["sequence.stats.v1"]
        cases = {
            "sequence before header": b"ACGT\n",
            "empty identifier": b">\nACGT\n",
            "no records": b"\n",
        }
        with tempfile.TemporaryDirectory() as directory:
            for name, content in cases.items():
                path = Path(directory) / "case.fa"
                path.write_bytes(content)
                with self.assertRaises(ValueError, msg=name):
                    implementation.run({"fasta": path}, {})

    def test_full_run_writes_the_envelope_artifact_and_self_reported_timing(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            output_directory = workspace / "analysis" / "sequence.stats.v1_run"
            output_directory.parent.mkdir(parents=True)
            request = make_request(TINY_FASTA, output_directory)
            code, envelope, stderr = run_harness(request, workspace)
            self.assertEqual(code, 0, stderr)
            self.assertEqual(envelope["status"], "ok")
            self.assertEqual(envelope["capability"], "sequence.stats.v1")
            self.assertEqual(envelope["schema_version"], "2")
            for field, expected in RUST_TINY_RESULT.items():
                if isinstance(expected, int):
                    self.assertEqual(envelope["result"][field], expected, field)
                else:
                    assert_close(self, expected, envelope["result"][field], field)

            [artifact] = envelope["artifacts"]
            artifact_path = Path(artifact["path"])
            self.assertEqual(artifact_path.parent, output_directory)
            self.assertTrue(artifact_path.is_file())
            self.assertEqual(artifact["sha256"], sha256_of(artifact_path))
            self.assertEqual(artifact["size_bytes"], artifact_path.stat().st_size)
            self.assertEqual(json.loads(artifact_path.read_text(encoding="utf-8")), envelope["result"])

            provenance = envelope["provenance"]
            self.assertEqual(provenance["input_sha256"], {"fasta": sha256_of(TINY_FASTA)})
            self.assertEqual(
                provenance["dependency_lock_sha256"], sha256_of(PACK_ROOT / "requirements.lock")
            )
            self.assertTrue(any(entry["name"] == "Biopython" for entry in provenance["software"]))

            [self_reported] = [
                diagnostic
                for diagnostic in envelope["diagnostics"]
                if diagnostic["code"] == MODULE.SELF_REPORTED_CODE
            ]
            self.assertEqual(self_reported["severity"], "info")
            metrics = json.loads(self_reported["message"])
            self.assertGreaterEqual(metrics["wall_ms"], 0.0)
            self.assertEqual(metrics["backend"], "python")
            if sys.platform != "win32":
                self.assertIsNotNone(metrics["peak_rss_mb"])
                self.assertGreater(metrics["peak_rss_mb"], 0.0)

            if HAVE_JSONSCHEMA:
                validator = Draft202012Validator(
                    json.loads(OUTPUT_SCHEMA.read_text(encoding="utf-8"))
                )
                self.assertEqual(sorted(validator.iter_errors(envelope), key=str), [])

    def test_refuses_to_overwrite_an_existing_output_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            output_directory = workspace / "out"
            output_directory.mkdir()
            request = make_request(TINY_FASTA, output_directory)
            code, envelope, _ = run_harness(request, workspace)
        self.assertEqual(code, 2)
        self.assertEqual(envelope["status"], "error")
        self.assertIn("output directory appeared", envelope["diagnostics"][0]["message"])


class ParityCompareMixin:
    """Recursive 1e-6 relative comparison against a recorded Rust result."""

    REFERENCE_DIR = PACK_ROOT / "tests" / "reference"

    def compare_with_reference(self, reference, actual, path="$"):
        if isinstance(reference, dict):
            self.assertEqual(set(reference), set(actual), path)
            for key, expected in reference.items():
                self.assertIn(key, actual, f"{path}.{key}")
                self.compare_with_reference(expected, actual[key], f"{path}.{key}")
        elif isinstance(reference, list):
            self.assertEqual(len(reference), len(actual), path)
            for index, (expected, value) in enumerate(zip(reference, actual)):
                self.compare_with_reference(expected, value, f"{path}[{index}]")
        elif isinstance(reference, bool):
            self.assertEqual(reference, actual, path)
        elif isinstance(reference, (int, float)):
            scale = max(abs(reference), abs(actual))
            if scale == 0:
                self.assertEqual(reference, actual, path)
            else:
                self.assertLessEqual(
                    abs(reference - actual) / scale,
                    1e-6,
                    f"{path}: {reference} vs {actual}",
                )
        else:
            self.assertEqual(reference, actual, path)

    def load_reference(self, name):
        return json.loads((self.REFERENCE_DIR / name).read_text(encoding="utf-8"))


@unittest.skipUnless(HAVE_BIOPYTHON, "Biopython is not installed")
class ExpressionPcaParityTests(ParityCompareMixin, unittest.TestCase):
    """M3 #5: the NumPy port must match the Rust engine's PCA field by field."""

    def setUp(self):
        try:
            import numpy  # noqa: F401

            self.implementation = MODULE.IMPLEMENTATIONS["expression.pca.v1"]
        except ImportError:
            self.skipTest("NumPy is not installed")

    def test_matches_the_rust_engine_on_the_fixture(self):
        reference = self.load_reference("expression.pca.v1.k3.json")
        actual = self.implementation.run(
            {"matrix": REPOSITORY_ROOT / "tests" / "fixtures" / "expression-matrix" / "deseq2-counts.csv"},
            {"components": 3, "scale_features": False},
        )
        self.compare_with_reference(reference, actual)

    def test_rejects_missing_and_non_numeric_values_like_the_engine(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "matrix.csv"
            path.write_text("gene,s1,s2\ng1,1,NA\ng2,2,3\n", encoding="utf-8")
            with self.assertRaises(ValueError, msg="missing values are rejected"):
                self.implementation.run({"matrix": path}, {})
            path.write_text("gene,s1,s2\ng1,1,x\ng2,2,3\n", encoding="utf-8")
            with self.assertRaises(ValueError, msg="non-numeric values are rejected"):
                self.implementation.run({"matrix": path}, {})

    def test_constant_features_warn_and_rank_limits_components(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "matrix.csv"
            # Two samples only: the rank permits exactly one component.
            path.write_text("gene,s1,s2\ng1,1,2\ng2,5,6\ng3,7,8\n", encoding="utf-8")
            result = self.implementation.run({"matrix": path}, {"components": 5})
            self.assertEqual(len(result["components"]), 1)
            self.assertTrue(
                any("matrix rank permits at most 1" in warning for warning in result["warnings"])
            )


class SetVennParityTests(ParityCompareMixin, unittest.TestCase):
    """M3 #7: exact set arithmetic, no third-party library involved."""

    def setUp(self):
        self.implementation = MODULE.IMPLEMENTATIONS["set.venn.v1"]
        self.sets_table = REPOSITORY_ROOT / "tests" / "fixtures" / "set-analysis" / "sets.tsv"

    def test_matches_the_rust_engine_on_the_fixture(self):
        self.compare_with_reference(
            self.load_reference("set.venn.v1.default.json"),
            self.implementation.run({"table": self.sets_table}, {}),
        )

    def test_items_mode_matches_the_rust_engine(self):
        self.compare_with_reference(
            self.load_reference("set.venn.v1.items.json"),
            self.implementation.run({"table": self.sets_table}, {"include_items": True}),
        )

    def test_rejects_more_than_six_columns_like_the_engine(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "wide.tsv"
            header = "\t".join(f"set{i}" for i in range(7))
            path.write_text(f"{header}\n" + "\t".join("a" for _ in range(7)) + "\n", encoding="utf-8")
            with self.assertRaises(ValueError, msg="six-column limit"):
                self.implementation.run({"table": path}, {})


if __name__ == "__main__":
    unittest.main()
