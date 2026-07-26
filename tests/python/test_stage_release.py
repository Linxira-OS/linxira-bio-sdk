from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))
SCRIPT = SCRIPTS / "stage-release.py"
SPEC = importlib.util.spec_from_file_location("stage_release", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
stage_release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(stage_release)


class ReleaseSourceCopyTests(unittest.TestCase):
    def test_generated_python_caches_are_not_staged(self) -> None:
        with tempfile.TemporaryDirectory() as source_directory:
            with tempfile.TemporaryDirectory() as output_directory:
                source = Path(source_directory)
                tree = source / "workflows"
                cache = tree / "__pycache__"
                cache.mkdir(parents=True)
                (tree / "workflow.py").write_text("print('ok')\n", encoding="utf-8")
                (cache / "workflow.cpython-313.pyc").write_bytes(b"cache")
                (tree / "legacy.pyo").write_bytes(b"cache")

                destination = Path(output_directory)
                manifest = {"include_files": [], "include_trees": ["workflows"]}
                with patch.object(stage_release, "ROOT", source):
                    stage_release.copy_sources(manifest, destination)

                self.assertTrue((destination / "workflows" / "workflow.py").is_file())
                self.assertFalse((destination / "workflows" / "__pycache__").exists())
                self.assertFalse((destination / "workflows" / "legacy.pyo").exists())


if __name__ == "__main__":
    unittest.main()
