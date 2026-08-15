import importlib.util
import io
import json
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path

from jsonschema import Draft202012Validator

try:
    import Bio  # type: ignore
    import numpy  # type: ignore

    HAVE_STACK = True
except ImportError:
    HAVE_STACK = False


SCRIPT = Path(__file__).resolve().parents[1] / "src" / "convert_sequences.py"
INPUT_SCHEMA = SCRIPT.parents[1] / "schemas" / "input.schema.json"
SPEC = importlib.util.spec_from_file_location("convert_sequences", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
INPUT_VALIDATOR = Draft202012Validator(
    json.loads(INPUT_SCHEMA.read_text(encoding="utf-8"))
)


class ValidationTests(unittest.TestCase):
    def request(self, source: Path, output_directory: Path) -> dict:
        return {
            "schema_version": "2",
            "job_id": "validation-test",
            "capability": MODULE.CAPABILITY,
            "inputs": [{
                "artifact_id": "sequences",
                "role": "sequences",
                "cardinality": "single",
                "files": [{
                    "file_id": "input",
                    "path": str(source),
                    "format": "fasta",
                    "compression": "none",
                    "size_bytes": source.stat().st_size,
                }],
            }],
            "execution": {"mode": "local-cpu"},
            "parameters": {
                "output_directory": str(output_directory),
                "output_filename": "converted.gb",
                "output_format": "genbank",
            },
        }

    def test_valid_request_without_importing_biopython(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">id\nACGT\n", encoding="utf-8")
            config = MODULE.validate_request(
                self.request(source, root / "output"), root / "output" / "result.json"
            )
            self.assertEqual(config["input_format"], "fasta")
            self.assertEqual(config["output_format"], "genbank")

    def test_unknown_field_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">id\nACGT\n", encoding="utf-8")
            request = self.request(source, root / "output")
            request["unexpected"] = True
            with self.assertRaisesRegex(MODULE.RequestError, "unsupported fields"):
                MODULE.validate_request(request, root / "output" / "result.json")

    def test_existing_output_directory_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">id\nACGT\n", encoding="utf-8")
            output = root / "output"
            output.mkdir()
            request = self.request(source, output)
            with self.assertRaisesRegex(MODULE.RequestError, "must not already exist"):
                MODULE.validate_request(request, output / "result.json")

    def test_nonportable_windows_output_filenames_are_rejected(self) -> None:
        invalid_filenames = [
            "bad:name.fa",
            "bad<name.fa",
            "bad>name.fa",
            'bad"name.fa',
            "bad/name.fa",
            "bad\\name.fa",
            "bad|name.fa",
            "bad?name.fa",
            "bad*name.fa",
            "control\x00name.fa",
            "control\x1fname.fa",
            "delete\x7fname.fa",
            "trailing.",
            "trailing ",
            ".",
            "..",
            "CON",
            "con.fa",
            "NUL.extra.txt",
            "COM1.fastq",
            "lpt9",
            "COM\u00b9.fa",
            "LPT\u00b3.txt",
            "CLOCK$.fa",
            "CONIN$.fa",
            "conout$.txt",
            "result.json",
        ]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">id\nACGT\n", encoding="utf-8")
            for filename in invalid_filenames:
                with self.subTest(filename=repr(filename)):
                    output = root / "output"
                    request = self.request(source, output)
                    request["parameters"]["output_filename"] = filename
                    with self.assertRaises(MODULE.RequestError):
                        MODULE.validate_request(request, output / "result.json")
                    self.assertTrue(
                        list(INPUT_VALIDATOR.iter_errors(request)),
                        f"input schema accepted nonportable filename {filename!r}",
                    )

    def test_portable_output_filenames_remain_valid(self) -> None:
        valid_filenames = ["converted.fa", "conifer.fa", "com10.fa", "reads.v1.fa"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">id\nACGT\n", encoding="utf-8")
            for filename in valid_filenames:
                with self.subTest(filename=filename):
                    output = root / "output"
                    request = self.request(source, output)
                    request["parameters"]["output_filename"] = filename
                    config = MODULE.validate_request(request, output / "result.json")
                    self.assertEqual(config["output_filename"], filename)
                    self.assertFalse(list(INPUT_VALIDATOR.iter_errors(request)))

    def test_atomic_json_replaces_complete_document(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "result.json"
            target.write_text("old", encoding="utf-8")
            MODULE.write_json_atomic(target, {"status": "ok"})
            self.assertEqual(json.loads(target.read_text(encoding="utf-8")), {"status": "ok"})

    def test_error_result_creates_atomic_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "failed-output" / "result.json"
            self.assertTrue(MODULE.write_error_result_atomic(target, {"status": "error"}))
            self.assertEqual(json.loads(target.read_text(encoding="utf-8")), {"status": "error"})

    def test_runtime_failure_commits_only_error_result(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">id\nACGT\n", encoding="utf-8")
            output = root / "output"
            request_path = root / "request.json"
            request_path.write_text(
                json.dumps(self.request(source, output)), encoding="utf-8"
            )
            expected_python = MODULE.EXPECTED_PYTHON
            MODULE.EXPECTED_PYTHON = (0, 0)
            try:
                with redirect_stderr(io.StringIO()):
                    status = MODULE.main([
                        "--request", str(request_path), "--result", str(output / "result.json")
                    ])
            finally:
                MODULE.EXPECTED_PYTHON = expected_python
            self.assertEqual(status, 2)
            self.assertEqual(
                [path.name for path in output.iterdir()], ["result.json"]
            )
            self.assertEqual(
                json.loads((output / "result.json").read_text(encoding="utf-8"))["status"],
                "error",
            )


@unittest.skipUnless(HAVE_STACK, "biopython 1.85 and numpy 2.2.4 are required")
class ConversionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.original_python = MODULE.EXPECTED_PYTHON
        self.original_numpy = MODULE.EXPECTED_NUMPY
        MODULE.EXPECTED_PYTHON = sys.version_info[:2]
        MODULE.EXPECTED_NUMPY = numpy.__version__

    def tearDown(self) -> None:
        MODULE.EXPECTED_PYTHON = self.original_python
        MODULE.EXPECTED_NUMPY = self.original_numpy

    def run_conversion(self, source: Path, output: Path, output_format: str) -> dict:
        request = ValidationTests().request(source, output)
        request["parameters"]["output_filename"] = f"converted.{output_format}"
        request["parameters"]["output_format"] = output_format
        request_path = output.parent / "request.json"
        request_path.write_text(json.dumps(request), encoding="utf-8")
        with redirect_stderr(io.StringIO()):
            status = MODULE.main(
                ["--request", str(request_path), "--result", str(output / "result.json")]
            )
        self.assertEqual(status, 0)
        return json.loads((output / "result.json").read_text(encoding="utf-8"))

    def test_fasta_to_genbank_defaults_molecule_type(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">one\nACGTNN\n>two\nGGGG\n>three\nAT\n", encoding="utf-8")
            output = root / "output"
            envelope = self.run_conversion(source, output, "genbank")
            self.assertEqual(envelope["status"], "ok")
            self.assertEqual(envelope["result"]["records_written"], 3)
            records = list(Bio.SeqIO.parse(output / "converted.genbank", "genbank"))
            self.assertEqual(len(records), 3)
            self.assertTrue(
                all(record.annotations.get("molecule_type") == "DNA" for record in records)
            )

    def test_genbank_molecule_type_is_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.gb"
            record = Bio.SeqRecord.SeqRecord(
                Bio.Seq.Seq("ACGTNN"), id="id", name="id"
            )
            record.annotations["molecule_type"] = "RNA"
            Bio.SeqIO.write(record, source, "genbank")
            output = root / "output"
            request = ValidationTests().request(source, output)
            request["inputs"][0]["files"][0]["format"] = "genbank"
            request["parameters"]["output_filename"] = "converted.gb"
            request_path = output.parent / "request.json"
            request_path.write_text(json.dumps(request), encoding="utf-8")
            with redirect_stderr(io.StringIO()):
                status = MODULE.main(
                    [
                        "--request",
                        str(request_path),
                        "--result",
                        str(output / "result.json"),
                    ]
                )
            self.assertEqual(status, 0)
            records = list(Bio.SeqIO.parse(output / "converted.gb", "genbank"))
            self.assertEqual(records[0].annotations.get("molecule_type"), "RNA")

    def test_fasta_to_embl_defaults_molecule_type(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "input.fa"
            source.write_text(">one\nACGTNN\n>two\nGGGG\n>three\nAT\n", encoding="utf-8")
            output = root / "output"
            envelope = self.run_conversion(source, output, "embl")
            self.assertEqual(envelope["status"], "ok")
            self.assertEqual(envelope["result"]["records_written"], 3)
            records = list(Bio.SeqIO.parse(output / "converted.embl", "embl"))
            self.assertEqual(len(records), 3)
            self.assertTrue(
                all(record.annotations.get("molecule_type") == "DNA" for record in records)
            )


if __name__ == "__main__":
    unittest.main()
