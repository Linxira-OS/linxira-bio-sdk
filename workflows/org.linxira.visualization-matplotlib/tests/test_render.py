"""Smoke and determinism tests for the matplotlib PlotSpec renderer (M1-T1).

Acceptance per ROADMAP: pack smoke tests green and the same PlotSpec renders
to identical bytes across three runs (fixed dpi, no timestamps). The data
summary must also match the ggplot2 backend for the same spec; that
cross-backend assertion runs when an R interpreter with the pack libraries is
available and is skipped otherwise (CI runs it on the Linux legs).
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

PACK_ROOT = Path(__file__).resolve().parents[1]
RENDER = PACK_ROOT / "src" / "render.py"
R_RENDER = PACK_ROOT.parent / "org.linxira.visualization-ggplot2" / "src" / "render.R"

SCATTER_SPEC = {
    "title": "PCA scores",
    "subtitle": "two groups",
    "x_label": "PC1 (38%)",
    "y_label": "PC2 (21%)",
    "theme": "publication",
    "palette": "set2",
    "legend": True,
    "figure": {"width": 800, "height": 600, "dpi": 150},
    "output": {"format": "svg"},
    "data": {
        "kind": "scatter",
        "series": [
            {"name": "control", "x": [1.0, 2.0, 3.0], "y": [2.0, 1.5, 3.5]},
            {"name": "treated", "x": [3.5, 4.0, 5.0], "y": [4.0, 5.5, 6.0]},
        ],
    },
}

HEATMAP_SPEC = {
    "title": "expression",
    "output": {"format": "png"},
    "data": {
        "kind": "heatmap",
        "matrix": {
            "row_labels": ["g1", "g2"],
            "col_labels": ["s1", "s2", "s3"],
            "values": [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]],
        },
    },
}


def run_renderer(plot_spec: dict, output_path: str, workdir: Path) -> dict:
    request_path = workdir / "request.json"
    result_path = workdir / "result.json"
    request_path.write_text(
        json.dumps({"plot_spec": plot_spec, "output_path": output_path}),
        encoding="utf-8",
    )
    completed = subprocess.run(
        [sys.executable, str(RENDER), "--request", str(request_path), "--result", str(result_path)],
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stderr
    return json.loads(result_path.read_text(encoding="utf-8"))


class MatplotlibRendererTests(unittest.TestCase):
    def test_renders_every_kind_and_records_summary(self) -> None:
        specs = [
            (SCATTER_SPEC, "figure.svg"),
            ({**SCATTER_SPEC, "data": {"kind": "line", "series": SCATTER_SPEC["data"]["series"]}}, "line.svg"),
            ({**SCATTER_SPEC, "data": {"kind": "bar", "series": [{"name": "counts", "x": ["a", "b"], "y": [3, 7]}]}}, "bar.svg"),
            ({**SCATTER_SPEC, "data": {"kind": "box", "groups": [{"name": "a", "values": [1.0, 2.0, 3.0]}]}}, "box.svg"),
            ({**SCATTER_SPEC, "data": {"kind": "violin", "groups": [{"name": "a", "values": [1.0, 2.0, 3.0]}]}}, "violin.svg"),
            (HEATMAP_SPEC, "heatmap.png"),
        ]
        with tempfile.TemporaryDirectory() as tmp:
            workdir = Path(tmp)
            for spec, name in specs:
                with self.subTest(kind=spec["data"]["kind"]):
                    result = run_renderer(spec, name, workdir)
                    self.assertEqual(result["backend"], "matplotlib")
                    self.assertEqual(result["artifact"]["path"], name)
                    artifact = workdir / name
                    self.assertTrue(artifact.exists())
                    self.assertGreater(artifact.stat().st_size, 0)
                    self.assertRegex(result["plot_spec_sha256"], r"^[0-9a-f]{64}$")

    def test_same_spec_renders_identical_bytes_three_times(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workdir = Path(tmp)
            digests = set()
            for _ in range(3):
                run_renderer(SCATTER_SPEC, "stable.svg", workdir)
                digests.add(hashlib.sha256((workdir / "stable.svg").read_bytes()).hexdigest())
            self.assertEqual(len(digests), 1, "svg output must be byte-stable")

    def test_data_summary_matches_expectations(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = run_renderer(SCATTER_SPEC, "f.svg", Path(tmp))
        summary = result["data_summary"]
        self.assertEqual(summary["kind"], "scatter")
        self.assertEqual(summary["series_count"], 2)
        self.assertEqual(summary["point_counts"], [3, 3])
        self.assertEqual(summary["x_range"], [1.0, 5.0])
        self.assertEqual(summary["y_range"], [1.5, 6.0])

    def test_unknown_kind_fails_structurally(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            workdir = Path(tmp)
            request_path = workdir / "request.json"
            result_path = workdir / "result.json"
            request_path.write_text(
                json.dumps({"plot_spec": {"data": {"kind": "hologram"}}, "output_path": "x.svg"}),
                encoding="utf-8",
            )
            completed = subprocess.run(
                [sys.executable, str(RENDER), "--request", str(request_path), "--result", str(result_path)],
                capture_output=True,
                text=True,
            )
            self.assertEqual(completed.returncode, 1)
            failure = json.loads(result_path.read_text(encoding="utf-8"))
            self.assertEqual(failure["status"], "error")

    def test_data_summary_matches_ggplot2_backend_when_r_available(self) -> None:
        if not R_RENDER.exists():
            self.skipTest("ggplot2 pack not present")
        import os

        r_library = os.environ.get("LINXIRA_BIO_WORKFLOW_R_LIBRARY", "")
        env = dict(os.environ)
        if r_library:
            env["R_LIBS_USER"] = r_library
        with tempfile.TemporaryDirectory() as tmp:
            workdir = Path(tmp)
            request_path = workdir / "request.json"
            r_result = workdir / "r-result.json"
            request_path.write_text(
                json.dumps({"plot_spec": SCATTER_SPEC, "output_path": "cross.svg"}),
                encoding="utf-8",
            )
            completed = subprocess.run(
                ["Rscript", str(R_RENDER), "--request", str(request_path), "--result", str(r_result)],
                capture_output=True,
                text=True,
                env=env,
            )
            if completed.returncode != 0:
                self.skipTest("R renderer unavailable: " + completed.stderr[:200])
            other = json.loads(r_result.read_text(encoding="utf-8"))
        here = run_renderer(SCATTER_SPEC, "cross.svg", Path(tempfile.mkdtemp()))
        # Data consistency per M1-T2: point counts and coordinate ranges must
        # match. plot_spec_sha256 is per-backend provenance (the two languages
        # canonicalize JSON differently) and is not expected to be equal.
        self.assertEqual(here["data_summary"], other["data_summary"])


if __name__ == "__main__":
    unittest.main()
