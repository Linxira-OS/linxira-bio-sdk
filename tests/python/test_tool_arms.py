"""Tool-arm mechanism tests: the producer, the schema contract, and the class guards.

These run without any ecosystem tool installed: the tests register a synthetic tool whose
argv they control, so what is under test is the protocol (warmup, N repeats, GNU-time or
degraded timing, schema-valid report with arm_class=tool) and the rule that keeps the two
arm classes out of one table or figure (ROADMAP 7.1).
"""

from __future__ import annotations

import importlib.util
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNNER_PATH = REPO_ROOT / "scripts" / "benchmark" / "run" / "tool-arms.py"
TABLES_PATH = REPO_ROOT / "scripts" / "benchmark" / "analysis" / "statistics-tables.py"
FIGURES_PATH = REPO_ROOT / "scripts" / "benchmark" / "figures" / "verify-figures.py"
SCHEMA_PATH = REPO_ROOT / "schemas" / "benchmark-report.schema.json"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


try:
    import jsonschema  # noqa: F401

    HAVE_JSONSCHEMA = True
except ImportError:  # pragma: no cover
    HAVE_JSONSCHEMA = False


class ToolArmRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.runner = load_module("tool_arms_runner", RUNNER_PATH)
        self.temporary = tempfile.TemporaryDirectory(prefix="tool-arm-test-")
        self.root = Path(self.temporary.name)
        self.input_path = self.root / "input.txt"
        self.input_path.write_text("payload\n", encoding="utf-8")
        self.registry = self.root / "tools.json"
        self.registry.write_text(
            json.dumps(
                {
                    "schema_version": "1",
                    "tools": {
                        "synthetic": {
                            "capability": "sequence.stats.v1",
                            "dataset_class": "sequence",
                            "tool_name": "synthetic tool",
                            "version_command": [sys.executable, "-c", "print('synthetic 9.9.9')"],
                            "run_command": [
                                sys.executable,
                                "-c",
                                "import sys,pathlib; pathlib.Path(sys.argv[1]).read_bytes(); print('ok')",
                                "{input}",
                            ],
                        },
                        "synthetic-broken": {
                            "capability": "sequence.stats.v1",
                            "dataset_class": "sequence",
                            "tool_name": "broken tool",
                            "version_command": [sys.executable, "-c", "print('broken 0.0.1')"],
                            "run_command": [sys.executable, "-c", "import sys; sys.exit(3)"],
                        },
                    },
                }
            ),
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_runner(self, *extra: str) -> int:
        return self.runner.main(
            [
                "--tool",
                "synthetic",
                "--input",
                str(self.input_path),
                "--registry",
                str(self.registry),
                "--output",
                str(self.root / "out"),
                "--warmup",
                "1",
                *extra,
            ]
        )

    def test_short_runs_are_refused_without_the_smoke_flag(self) -> None:
        with self.assertRaises(SystemExit) as caught:
            self.run_runner("--repeat", "3")
        self.assertIn("at least 10", str(caught.exception))

    def test_report_is_schema_valid_and_declares_the_arm_class(self) -> None:
        self.assertEqual(self.run_runner("--repeat", "3", "--allow-short"), 0)
        report_path = self.root / "out" / "sequence.stats.v1.synthetic.tool-arm.json"
        report = json.loads(report_path.read_text(encoding="utf-8"))

        self.assertEqual(report["arm_class"], "tool")
        self.assertEqual(report["consistency"], "not-applicable")
        self.assertIsNone(report["speedup"], "a tool arm must not store a cross-class ratio")
        self.assertIsNone(report["memory_saving"])
        self.assertEqual(report["tool"]["name"], "synthetic tool")
        self.assertEqual(report["tool"]["version"], "synthetic 9.9.9")
        # The recorded invocation is the substituted argv, which is what makes a
        # disagreement attributable to the call rather than guessed. The runner records
        # the *resolved* input path (tool-arms.py resolves before joining the argv), so
        # compare resolved forms: on the Windows CI runner a temporary directory comes
        # back as an 8.3 short name (RUNNER~1) while the child process sees the long one,
        # and that spelling difference is not what this assertion is about. A substring
        # check also survives paths that contain spaces, which re-splitting the joined
        # argv would not.
        self.assertIn(str(self.input_path.resolve()), report["tool"]["invocation"])
        # The measurement's input identity travels with the report: a standalone
        # tool-arm report has no window descriptor beside it to lean on.
        recorded = report["inputs"]
        self.assertEqual(len(recorded), 1)
        self.assertEqual(recorded[0]["sha256"], hashlib.sha256(self.input_path.read_bytes()).hexdigest())
        self.assertEqual(recorded[0]["bytes"], self.input_path.stat().st_size)

        backend = report["backends"][0]
        self.assertEqual(backend["repeats"], 3)
        self.assertEqual(len(backend["runs"]), 3)
        self.assertTrue(all(run["ok"] for run in backend["runs"]))
        self.assertTrue(all(run["output_bytes"] > 0 for run in backend["runs"]))
        self.assertGreaterEqual(backend["median_wall_ms"], 0)
        self.assertGreaterEqual(backend["max_wall_ms"], backend["min_wall_ms"])

        if HAVE_JSONSCHEMA:
            schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
            jsonschema.validate(instance=report, schema=schema)

    def test_a_tool_that_fails_every_run_is_an_error(self) -> None:
        with self.assertRaises(self.runner.ToolArmError):
            self.runner.main(
                [
                    "--tool",
                    "synthetic-broken",
                    "--input",
                    str(self.input_path),
                    "--registry",
                    str(self.registry),
                    "--output",
                    str(self.root / "out"),
                    "--repeat",
                    "2",
                    "--allow-short",
                    "--warmup",
                    "0",
                ]
            )


class ToolRegistryTests(unittest.TestCase):
    """The registry names exact argv; a wrong path there fails only at run time, so the
    paths are asserted here. This caught `{script_dir}/commands/...` pointing one level
    above where the command scripts live."""

    def test_script_paths_in_registry_exist(self) -> None:
        registry = json.loads(
            (REPO_ROOT / "scripts" / "benchmark" / "run" / "tool-arms" / "tools.json").read_text(
                encoding="utf-8"
            )
        )
        script_dir = REPO_ROOT / "scripts" / "benchmark" / "run"
        checked = 0
        for key, entry in registry["tools"].items():
            commands = [entry["version_command"], entry["run_command"]]
            for command in commands:
                for argument in command:
                    if "{script_dir}" not in argument:
                        continue
                    resolved = Path(argument.replace("{script_dir}", str(script_dir)))
                    self.assertTrue(resolved.is_file(), f"{key}: {resolved} does not exist")
                    checked += 1
        self.assertGreaterEqual(checked, 1, "registry has no {script_dir} commands to check")


class ArmClassContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tables = load_module("statistics_tables_module", TABLES_PATH)
        self.figures = load_module("verify_figures_module", FIGURES_PATH)
        self.temporary = tempfile.TemporaryDirectory(prefix="arm-class-test-")
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_report(self, name: str, arm_class: str | None) -> Path:
        report = {
            "schema_version": "1",
            "capability": "sequence.stats.v1",
            "dataset_class": "sequence",
            "backends": [
                {
                    "backend": "rust",
                    "repeats": 1,
                    "median_wall_ms": 10.0,
                    "min_wall_ms": 10.0,
                    "max_wall_ms": 10.0,
                    "iqr_wall_ms": 0.0,
                    "runs": [
                        {
                            "backend": "rust",
                            "run_index": 1,
                            "wall_ms": 10.0,
                            "output_bytes": 1,
                            "ok": True,
                        }
                    ],
                }
            ],
            "consistency": "consistent",
            "environment": {
                "os": "linux",
                "engine_version": "test",
                "in_container": False,
                "page_cache": "warm",
                "precision": "high",
            },
            "sampled_at": "2026-10-12T00:00:00Z",
        }
        if arm_class is not None:
            report["arm_class"] = arm_class
        path = self.root / name
        path.write_text(json.dumps(report), encoding="utf-8")
        return path

    def test_reports_without_the_field_are_parity_arms(self) -> None:
        path = self.write_report("legacy.benchmark.json", None)
        report = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(self.tables.report_arm_class(report), "parity")
        self.figures.require_arm_class(report, path, "parity")

    def test_a_tool_report_cannot_enter_the_parity_tables(self) -> None:
        path = self.write_report("tool.benchmark.json", "tool")
        entry = {"report": path.name}
        with self.assertRaises(SystemExit) as caught:
            self.tables.load_parity_report(self.root, entry)
        message = str(caught.exception)
        self.assertIn("arm_class='tool'", message)
        self.assertIn("--tool-arms", message)

    def test_a_figure_set_refuses_the_other_class(self) -> None:
        path = self.write_report("tool.benchmark.json", "tool")
        report = json.loads(path.read_text(encoding="utf-8"))
        with self.assertRaises(SystemExit):
            self.figures.require_arm_class(report, path, "parity")

    def test_tool_arm_table_rows_name_their_class(self) -> None:
        report = {
            "schema_version": "1",
            "capability": "sequence.stats.v1",
            "dataset_class": "sequence",
            "arm_class": "tool",
            "tool": {"name": "pyfastx", "version": "pyfastx version 2.3.1", "invocation": "pyfastx stat x.fa"},
            "backends": [
                {
                    "backend": "pyfastx-stat",
                    "repeats": 2,
                    "median_wall_ms": 42.0,
                    "min_wall_ms": 40.0,
                    "max_wall_ms": 44.0,
                    "iqr_wall_ms": 4.0,
                    "median_peak_rss_mb": None,
                    "runs": [
                        {"backend": "pyfastx-stat", "run_index": 1, "wall_ms": 40.0,
                         "output_bytes": 5, "ok": True},
                        {"backend": "pyfastx-stat", "run_index": 2, "wall_ms": 44.0,
                         "output_bytes": 5, "ok": True},
                    ],
                }
            ],
            "speedup": None,
            "memory_saving": None,
            "consistency": "not-applicable",
            "findings": [],
            "sampled_at": "2026-10-12T00:00:00Z",
        }
        (self.root / "sequence.stats.v1.pyfastx-stat.tool-arm.json").write_text(
            json.dumps(report), encoding="utf-8"
        )
        rows = self.tables.tool_arm_rows(self.root)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0][0], "sequence.stats.v1")
        self.assertEqual(rows[0][2], "tool")
        self.assertEqual(rows[0][3], "pyfastx")
        self.assertEqual(rows[0][5], "2")
        self.assertEqual(rows[0][6], "42.000")


class SchemaArmClassTests(unittest.TestCase):
    @unittest.skipUnless(HAVE_JSONSCHEMA, "jsonschema is not installed")
    def test_tool_arms_require_the_tool_block(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
        base = {
            "schema_version": "1",
            "capability": "sequence.stats.v1",
            "dataset_class": "sequence",
            "backends": [],
            "consistency": "not-applicable",
            "environment": {
                "os": "linux",
                "engine_version": "tool-arm-runner/1",
                "in_container": False,
                "page_cache": "warm",
                "precision": "high",
            },
            "sampled_at": "2026-10-12T00:00:00Z",
        }
        jsonschema.validate(instance=dict(base), schema=schema)
        jsonschema.validate(
            instance={
                **base,
                "arm_class": "tool",
                "tool": {"name": "x", "version": "1", "invocation": "x --help"},
            },
            schema=schema,
        )
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(instance={**base, "arm_class": "tool"}, schema=schema)
        with self.assertRaises(jsonschema.ValidationError):
            jsonschema.validate(
                instance={
                    **base,
                    "arm_class": "parity",
                    "tool": {"name": "x", "version": "1", "invocation": "x --help"},
                },
                schema=schema,
            )


if __name__ == "__main__":
    unittest.main()
