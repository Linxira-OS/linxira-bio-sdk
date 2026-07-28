import hashlib
import json
import unittest
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[2]
CATALOG_PATH = ROOT / "workflows" / "catalog.json"


def load_json(path: Path) -> dict:
    with path.open("r", encoding="utf-8") as handle:
        value = json.load(handle)
    if type(value) is not dict:
        raise AssertionError(f"{path} must contain an object")
    return value


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


class WorkflowManifestTests(unittest.TestCase):
    def test_cataloged_pack_manifests_are_complete_and_exact(self) -> None:
        catalog = load_json(CATALOG_PATH)
        cataloged = [pack for pack in catalog["packs"] if pack["status"] == "cataloged"]
        self.assertTrue(cataloged, "at least one cataloged workflow pack is required")
        for pack in cataloged:
            with self.subTest(pack=pack["id"]):
                self.verify_pack(pack)

    def verify_pack(self, catalog_entry: dict) -> None:
        manifest_path = ROOT / catalog_entry["manifest"]
        self.assertTrue(manifest_path.is_file(), f"missing manifest: {manifest_path}")
        manifest = load_json(manifest_path)
        self.assertEqual(manifest["id"], catalog_entry["id"])
        self.assertEqual(manifest["runtime"]["kind"], catalog_entry["runtime"])
        pack_root = manifest_path.parent

        declared: dict[str, str] = {}
        for item in manifest["files"]:
            relative = PurePosixPath(item["path"])
            self.assertFalse(relative.is_absolute())
            self.assertNotIn("..", relative.parts)
            self.assertNotIn(item["path"], declared, "duplicate manifest file")
            declared[item["path"]] = item["sha256"].lower()

        actual = {
            path.relative_to(pack_root).as_posix()
            for path in pack_root.rglob("*")
            if path.is_file()
            and path.name != "manifest.json"
            and "__pycache__" not in path.parts
            and path.suffix != ".pyc"
        }
        self.assertEqual(set(declared), actual, "manifest must cover every distributed pack file")
        for relative, expected in declared.items():
            path = pack_root / relative
            self.assertEqual(sha256_file(path), expected, f"SHA-256 mismatch: {path}")

        entrypoint = manifest["entrypoint"]["path"]
        lock = manifest["runtime"]["dependency_lock"]
        self.assertIn(entrypoint, declared)
        self.assertIn(lock["path"], declared)
        self.assertEqual(lock["sha256"].lower(), declared[lock["path"]])
        for contract in (manifest["input_schema"], manifest["output_schema"]):
            reference = contract.get("$ref")
            self.assertIsInstance(reference, str)
            self.assertIn(reference, declared)

        notice = (pack_root / "NOTICE.md").read_text(encoding="utf-8")
        self.assertIn("AGPL-3.0-or-later", notice)
        self.assertIn("Runtime dependencies are installed separately", notice)
        self.assertIn("not vendored", notice)


if __name__ == "__main__":
    unittest.main()
