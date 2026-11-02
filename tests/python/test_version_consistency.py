from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "check-version-consistency.py"


class VersionConsistencyTests(unittest.TestCase):
    def _run(self, product: str, sdk: str, conda: str, tag: str | None = None):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "sdk" / "python" / "linxira_bio").mkdir(parents=True)
            (root / "packaging" / "bioconda").mkdir(parents=True)

            (root / "Cargo.toml").write_text(
                f'[workspace.package]\nversion = "{product}"\n', encoding="utf-8"
            )
            (root / "sdk" / "python" / "pyproject.toml").write_text(
                f'[project]\nversion = "{sdk}"\n', encoding="utf-8"
            )
            (root / "sdk" / "python" / "linxira_bio" / "__init__.py").write_text(
                f'__version__ = "{sdk}"\n', encoding="utf-8"
            )
            (root / "packaging" / "bioconda" / "meta.yaml").write_text(
                f'{{% set version = "{conda}" %}}\n', encoding="utf-8"
            )

            env = os.environ.copy()
            env.pop("GITHUB_REF_TYPE", None)
            env.pop("GITHUB_REF_NAME", None)
            command = [sys.executable, str(SCRIPT), "--root", str(root)]
            if tag is not None:
                command.extend(["--tag", tag])
            return subprocess.run(command, capture_output=True, text=True, env=env)

    def test_lockstep_versions_pass(self) -> None:
        result = self._run("1.2.3", "1.2.3", "1.2.3", "v1.2.3")

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_legacy_sdk_exception_passes(self) -> None:
        result = self._run("1.1.5", "0.1.0", "1.1.5", "v1.1.5")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("legacy SDK version exception", result.stderr)

    def test_sdk_mismatch_fails(self) -> None:
        result = self._run("1.2.3", "0.1.0", "1.2.3")

        self.assertEqual(result.returncode, 1)
        self.assertIn("SDK version 0.1.0 != product 1.2.3", result.stderr)

    def test_conda_mismatch_fails(self) -> None:
        result = self._run("1.2.3", "1.2.3", "1.2.2")

        self.assertEqual(result.returncode, 1)
        self.assertIn("Bioconda recipe version 1.2.2 != product 1.2.3", result.stderr)

    def test_tag_mismatch_fails(self) -> None:
        result = self._run("1.2.3", "1.2.3", "1.2.3", "v1.2.4")

        self.assertEqual(result.returncode, 1)
        self.assertIn("release tag v1.2.4 != v1.2.3", result.stderr)


if __name__ == "__main__":
    unittest.main()
