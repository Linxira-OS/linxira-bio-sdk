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
    def test_catalog_capabilities_and_aliases_are_globally_unique(self) -> None:
        catalog = load_json(CATALOG_PATH)
        pack_ids: set[str] = set()
        capabilities: set[str] = set()
        for pack in catalog["packs"]:
            self.assertNotIn(pack["id"], pack_ids)
            pack_ids.add(pack["id"])
            aliases = pack.get("capability_aliases", [])
            self.assertEqual(len(aliases), len(set(aliases)))
            self.assertNotIn(pack["capability"], aliases)
            for capability in [pack["capability"], *aliases]:
                self.assertNotIn(capability, capabilities)
                capabilities.add(capability)

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

        if manifest["runtime"]["kind"] == "r":
            self.verify_r_runtime_policy(pack_root, manifest, catalog_entry)

        notice = (pack_root / "NOTICE.md").read_text(encoding="utf-8")
        self.assertIn("AGPL-3.0-or-later", notice)
        self.assertIn("Runtime dependencies are installed separately", notice)
        self.assertIn("not vendored", notice)

    def verify_r_runtime_policy(
        self, pack_root: Path, manifest: dict, catalog_entry: dict
    ) -> None:
        self.assertEqual(manifest["runtime"]["version"], ">=4.6.1,<4.7.0")
        self.assertEqual(catalog_entry["status"], "cataloged")
        self.assertEqual(catalog_entry["capability"], "expression.differential.v1")
        self.assertEqual(
            catalog_entry["capability_aliases"],
            ["medical.bulk-rnaseq.v1", "expression.deseq2.v1"],
        )
        lock = load_json(pack_root / manifest["runtime"]["dependency_lock"]["path"])
        self.assertEqual(lock["schema_version"], "2")
        self.assertEqual(lock["lock_kind"], "compatibility-and-resolution-policy")
        self.assertEqual(lock["runtime"]["preferred_version"], "4.6.1")
        self.assertEqual(
            lock["runtime"]["version_requirement"], manifest["runtime"]["version"]
        )
        self.assertEqual(lock["runtime"]["bioconductor_release"], "3.23")
        isolation = lock["isolation"]
        self.assertEqual(isolation["scope"], "project")
        self.assertEqual(
            isolation["interpreter_environment_variable"],
            "LINXIRA_BIO_WORKFLOW_R",
        )
        self.assertEqual(
            isolation["library_environment_variable"],
            "LINXIRA_BIO_WORKFLOW_R_LIBRARY",
        )
        self.assertTrue(isolation["declared_packages_must_resolve_from_project_library"])
        self.assertTrue(
            isolation["all_loaded_non_base_packages_must_resolve_from_project_library"]
        )
        self.assertFalse(isolation["global_library_mutation"])
        self.assertFalse(isolation["global_path_mutation"])
        self.assertTrue(isolation["side_by_side_runtime_versions"])
        requirements = {
            package["name"]: package["version_requirement"]
            for package in lock["direct_requirements"]
        }
        self.assertEqual(
            requirements,
            {
                "DESeq2": ">=1.52.0,<1.53.0",
                "jsonlite": ">=1.8.9,<3.0.0",
                "digest": ">=0.6.37,<0.7.0",
            },
        )
        resolved = lock["resolved_environment_lock"]
        self.assertTrue(resolved["required_before_activation"])
        self.assertEqual(resolved["completeness"], "direct-and-transitive")
        self.assertEqual(
            set(resolved["required_fields"]),
            {"name", "version", "repository", "source_url", "sha256", "license"},
        )
        self.assertFalse(lock["installable"])

        supported_capabilities = {
            "expression.differential.v1",
            "medical.bulk-rnaseq.v1",
            "expression.deseq2.v1",
        }
        input_schema = load_json(pack_root / "schemas" / "input.schema.json")
        output_schema = load_json(pack_root / "schemas" / "output.schema.json")
        self.assertEqual(
            set(input_schema["properties"]["capability"]["enum"]),
            supported_capabilities,
        )
        self.assertEqual(
            set(output_schema["properties"]["capability"]["enum"]),
            supported_capabilities,
        )
        result_properties = output_schema["properties"]["result"]["properties"]
        self.assertEqual(result_properties["intended_use"]["const"], "research-use-only")
        self.assertFalse(result_properties["clinical_use"]["const"])
        script = (pack_root / "src" / "run_deseq2.R").read_text(encoding="utf-8")
        self.assertIn("capability = config$capability", script)
        self.assertIn('code = "research_use_only"', script)
        self.assertIn("provide diagnosis, treatment advice, or clinical interpretation", script)

        distributed_text = "\n".join(
            path.read_text(encoding="utf-8")
            for path in pack_root.rglob("*")
            if path.is_file() and path.suffix.lower() in {".json", ".md", ".r"}
        )
        self.assertNotIn("4.4.3", distributed_text)
        for requirement in requirements.values():
            self.assertIn(requirement, distributed_text)


if __name__ == "__main__":
    unittest.main()
