from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "generate_third_party_notices.py"
SPEC = importlib.util.spec_from_file_location("third_party_notices", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
notices = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(notices)


class DocumentCoverageTests(unittest.TestCase):
    def test_compound_expression_requires_distinct_texts(self) -> None:
        package = {
            "name": "example",
            "version": "1.0.0",
            "license": "MIT OR Apache-2.0",
            "license_file": None,
        }
        mit_document = {
            "sha256": "a" * 64,
            "path": "LICENSE-MIT",
            "text": "Permission is hereby granted, free of charge, to any person.",
        }
        apache_document = {
            "sha256": "b" * 64,
            "path": "LICENSE-APACHE",
            "text": "Apache License\nVersion 2.0, January 2004\n",
        }
        with self.assertRaisesRegex(notices.NoticeError, "ambiguous license text"):
            notices.assert_document_coverage(package, [mit_document])
        notices.assert_document_coverage(
            package, [mit_document, apache_document]
        )

    def test_legacy_slash_expression_is_compound(self) -> None:
        package = {
            "name": "example",
            "version": "1.0.0",
            "license": "MIT/Apache-2.0",
            "license_file": None,
        }
        document = {
            "sha256": "a" * 64,
            "path": "LICENSE-MIT",
            "text": "Permission is hereby granted, free of charge, to any person.",
        }
        with self.assertRaisesRegex(notices.NoticeError, "needs at least 2"):
            notices.assert_document_coverage(package, [document])

    def test_unknown_spdx_identifier_requires_mapping(self) -> None:
        package = {
            "name": "example",
            "version": "1.0.0",
            "license": "LicenseRef-Unknown",
            "license_file": None,
        }
        document = {
            "sha256": "a" * 64,
            "path": "LICENSE",
            "text": "A non-empty but unknown license text.",
        }
        with self.assertRaisesRegex(notices.NoticeError, "no strict document mapping"):
            notices.assert_document_coverage(package, [document])

    def test_unresolved_path_pointer_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "LICENSE"
            path.write_text("../LICENSE\n", encoding="utf-8")
            with self.assertRaises(notices.UnresolvedNoticePointer):
                notices.read_notice(path)

    def test_isc_and_zero_bsd_texts_are_distinguished(self) -> None:
        shared = (
            "Permission to use, copy, modify, and/or distribute this software "
            "for any purpose with or without fee is hereby granted"
        )
        isc = (
            shared
            + ", provided that the above copyright notice and this permission "
            "notice appear in all copies."
        )
        self.assertTrue(notices.document_text_matches("ISC", isc))
        self.assertFalse(notices.document_text_matches("0BSD", isc))
        self.assertTrue(notices.document_text_matches("0BSD", shared + "."))
        self.assertFalse(notices.document_text_matches("ISC", shared + "."))

    def test_missing_license_metadata_fails(self) -> None:
        package = {
            "name": "example",
            "version": "1.0.0",
            "license": None,
            "license_file": None,
        }
        with self.assertRaisesRegex(notices.NoticeError, "neither license"):
            notices.assert_document_coverage(package, [])


class ReleaseClosureTests(unittest.TestCase):
    def test_dev_only_dependencies_are_excluded(self) -> None:
        root_id = "path+file:///workspace/root#1.0.0"
        normal_id = "registry+index#normal@1.0.0"
        dev_id = "registry+index#dev@1.0.0"
        metadata = {
            "workspace_members": [root_id],
            "packages": [
                {"id": root_id, "name": "root", "version": "1.0.0", "source": None},
                {
                    "id": normal_id,
                    "name": "normal",
                    "version": "1.0.0",
                    "source": "registry+index",
                },
                {
                    "id": dev_id,
                    "name": "dev",
                    "version": "1.0.0",
                    "source": "registry+index",
                },
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": root_id,
                        "features": [],
                        "deps": [
                            {
                                "pkg": normal_id,
                                "dep_kinds": [{"kind": None, "target": None}],
                            },
                            {
                                "pkg": dev_id,
                                "dep_kinds": [{"kind": "dev", "target": None}],
                            },
                        ],
                    },
                    {"id": normal_id, "features": ["default"], "deps": []},
                    {"id": dev_id, "features": [], "deps": []},
                ]
            },
        }
        selected = {
            ("root", "1.0.0"): [],
            ("normal", "1.0.0"): ["default"],
        }
        roots, packages = notices.release_closure(metadata, ["root"], selected)
        self.assertEqual(roots, [{"name": "root", "version": "1.0.0"}])
        self.assertEqual([package["name"] for package in packages], ["normal"])

    def test_cargo_tree_parser_deduplicates_and_unions_features(self) -> None:
        output = "\n".join(
            [
                "root v1.0.0 (C:\\workspace)|default",
                "normal v2.0.0|feature-a",
                "normal v2.0.0|feature-b (*)",
            ]
        )
        self.assertEqual(
            notices.parse_cargo_tree(output),
            {
                ("root", "1.0.0"): ["default"],
                ("normal", "2.0.0"): ["feature-a", "feature-b"],
            },
        )

    def test_cargo_tree_selects_only_normal_and_build_edges(self) -> None:
        completed = SimpleNamespace(
            returncode=0,
            stdout="root v1.0.0 (C:\\workspace)|default\nnormal v2.0.0|\n",
            stderr="",
        )
        with patch.object(notices.subprocess, "run", return_value=completed) as run:
            selected = notices.cargo_release_tree(
                "x86_64-pc-windows-gnu", ["root"], offline=True
            )
        command = run.call_args.args[0]
        edge_index = command.index("--edges")
        self.assertEqual(command[edge_index + 1], "normal,build")
        self.assertIn("--offline", command)
        self.assertEqual(
            selected,
            {("root", "1.0.0"): ["default"], ("normal", "2.0.0"): []},
        )


class ArchiveIntegrityTests(unittest.TestCase):
    @staticmethod
    def write_crate(path: Path, members: dict[str, bytes]) -> bytes:
        path.parent.mkdir(parents=True)
        with tarfile.open(path, mode="w:gz") as archive:
            for name, content in members.items():
                member = tarfile.TarInfo(name)
                member.size = len(content)
                member.mode = 0o644
                archive.addfile(member, io.BytesIO(content))
        return path.read_bytes()

    def test_registry_documents_come_from_locked_archive_not_unpack_directory(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            cargo_home = Path(directory) / ".cargo"
            partition = "index.crates.io-test"
            package_root = (
                cargo_home / "registry" / "src" / partition / "example-1.0.0"
            )
            package_root.mkdir(parents=True)
            manifest = package_root / "Cargo.toml"
            manifest.write_text("[package]\nname='example'\nversion='1.0.0'\n")
            unpacked_license = package_root / "LICENSE"
            unpacked_license.write_text("tampered unpacked license\n", encoding="utf-8")
            archive_path = (
                cargo_home
                / "registry"
                / "cache"
                / partition
                / "example-1.0.0.crate"
            )
            archive_license = (
                b"Permission is hereby granted, free of charge, to any person.\n"
            )
            revision = "a" * 40
            archive_content = self.write_crate(
                archive_path,
                {
                    "example-1.0.0/LICENSE": archive_license,
                    "example-1.0.0/.cargo_vcs_info.json": json.dumps(
                        {"git": {"sha1": revision}}
                    ).encode("utf-8"),
                },
            )
            package = {
                "id": "registry+index#example@1.0.0",
                "name": "example",
                "version": "1.0.0",
                "source": "registry+index",
                "manifest_path": str(manifest),
                "license_file": str(unpacked_license),
            }
            checksum = hashlib.sha256(archive_content).hexdigest()

            first = notices.registry_package_evidence(package, checksum)
            unpacked_license.write_text("different tampering\n", encoding="utf-8")
            second = notices.registry_package_evidence(package, checksum)

            self.assertEqual(first, second)
            self.assertEqual(first[0][0]["text"].encode("utf-8"), archive_license)
            self.assertEqual(first[2], {"revision": revision})

    def test_registry_archive_checksum_tampering_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            cargo_home = Path(directory) / ".cargo"
            partition = "index.crates.io-test"
            package_root = (
                cargo_home / "registry" / "src" / partition / "example-1.0.0"
            )
            package_root.mkdir(parents=True)
            manifest = package_root / "Cargo.toml"
            manifest.write_text("[package]\nname='example'\nversion='1.0.0'\n")
            archive_path = (
                cargo_home
                / "registry"
                / "cache"
                / partition
                / "example-1.0.0.crate"
            )
            archive_content = self.write_crate(
                archive_path,
                {
                    "example-1.0.0/LICENSE": (
                        b"Permission is hereby granted, free of charge.\n"
                    )
                },
            )
            package = {
                "id": "registry+index#example@1.0.0",
                "name": "example",
                "version": "1.0.0",
                "source": "registry+index",
                "manifest_path": str(manifest),
                "license_file": None,
            }
            checksum = hashlib.sha256(archive_content).hexdigest()
            archive_path.write_bytes(archive_content + b"tampered")

            with self.assertRaisesRegex(notices.NoticeError, "SHA-256 mismatch"):
                notices.registry_package_evidence(package, checksum)

    def test_registry_lock_requires_checksum(self) -> None:
        lock = b'''version = 4

[[package]]
name = "example"
version = "1.0.0"
source = "registry+index"
'''
        with self.assertRaisesRegex(notices.NoticeError, "invalid checksum"):
            notices.cargo_lock_registry_checksums(lock)


class OverrideConfigurationTests(unittest.TestCase):
    def test_hash_tampering_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            document = root / "licenses" / "cargo-overrides" / "example" / "LICENSE.txt"
            document.parent.mkdir(parents=True)
            document.write_bytes(b"Example license\n")
            override_path = root / "licenses" / "cargo-overrides.json"
            payload = {
                "schema_version": "1",
                "entries": [
                    {
                        "package": "example",
                        "version": "1.0.0",
                        "source": "registry+index",
                        "license_expression": "MIT",
                        "vcs_revision": "a" * 40,
                        "platforms": ["windows"],
                        "reason": "The crate archive omits its license.",
                        "documents": [
                            {
                                "path": "licenses/cargo-overrides/example/LICENSE.txt",
                                "sha256": hashlib.sha256(document.read_bytes()).hexdigest(),
                                "source_url": (
                                    f"https://example.invalid/blob/{'a' * 40}/LICENSE.txt"
                                ),
                            }
                        ],
                    }
                ],
            }
            override_path.write_text(json.dumps(payload), encoding="utf-8")
            with patch.object(notices, "ROOT", root), patch.object(
                notices, "OVERRIDES_PATH", override_path
            ):
                self.assertEqual(len(notices.validate_override_configuration()), 1)
                document.write_bytes(b"Tampered license\n")
                with self.assertRaisesRegex(notices.NoticeError, "SHA-256 mismatch"):
                    notices.validate_override_configuration()

    def test_path_traversal_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outside = root / "LICENSE"
            outside.write_bytes(b"Example license\n")
            override_path = root / "licenses" / "cargo-overrides.json"
            override_path.parent.mkdir(parents=True)
            payload = {
                "schema_version": "1",
                "entries": [
                    {
                        "package": "example",
                        "version": "1.0.0",
                        "source": "registry+index",
                        "license_expression": "MIT",
                        "vcs_revision": "a" * 40,
                        "platforms": ["windows"],
                        "reason": "The crate archive omits its license.",
                        "documents": [
                            {
                                "path": "licenses/cargo-overrides/../../LICENSE",
                                "sha256": hashlib.sha256(outside.read_bytes()).hexdigest(),
                                "source_url": f"https://example.invalid/blob/{'a' * 40}/LICENSE",
                            }
                        ],
                    }
                ],
            }
            override_path.write_text(json.dumps(payload), encoding="utf-8")
            with patch.object(notices, "ROOT", root), patch.object(
                notices, "OVERRIDES_PATH", override_path
            ):
                with self.assertRaisesRegex(notices.NoticeError, "leaves licenses"):
                    notices.validate_override_configuration()

    def test_symlink_escape_is_rejected_when_supported(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            override_root = root / "licenses" / "cargo-overrides"
            override_root.mkdir(parents=True)
            outside = root / "outside-license.txt"
            outside.write_bytes(b"Example license\n")
            link = override_root / "LICENSE.txt"
            try:
                link.symlink_to(outside)
            except OSError as error:
                self.skipTest(f"file symlinks are unavailable: {error}")
            override_path = root / "licenses" / "cargo-overrides.json"
            payload = {
                "schema_version": "1",
                "entries": [
                    {
                        "package": "example",
                        "version": "1.0.0",
                        "source": "registry+index",
                        "license_expression": "MIT",
                        "vcs_revision": "a" * 40,
                        "platforms": ["windows"],
                        "reason": "The crate archive omits its license.",
                        "documents": [
                            {
                                "path": "licenses/cargo-overrides/LICENSE.txt",
                                "sha256": hashlib.sha256(outside.read_bytes()).hexdigest(),
                                "source_url": f"https://example.invalid/blob/{'a' * 40}/LICENSE",
                            }
                        ],
                    }
                ],
            }
            override_path.write_text(json.dumps(payload), encoding="utf-8")
            with patch.object(notices, "ROOT", root), patch.object(
                notices, "OVERRIDES_PATH", override_path
            ):
                with self.assertRaisesRegex(notices.NoticeError, "leaves licenses"):
                    notices.validate_override_configuration()


class HumanReportTests(unittest.TestCase):
    def test_rendering_is_deterministic(self) -> None:
        report = {
            "platform": "windows",
            "target_triple": "x86_64-pc-windows-gnu",
            "cargo_version": "cargo 1.92.0 (000000000 2026-01-01)",
            "cargo_lock_sha256": "0" * 64,
            "dependency_count": 1,
            "release_roots": [{"name": "root", "version": "1.0.0"}],
            "dependencies": [
                {
                    "name": "example",
                    "version": "1.0.0",
                    "source": "registry+index",
                    "repository": None,
                    "license_expression": "MIT",
                    "documents": [
                        {
                            "origin": "crate-package",
                            "path": "LICENSE",
                            "sha256": "1" * 64,
                        }
                    ],
                }
            ],
            "license_texts": [{"sha256": "1" * 64, "text": "License text\n"}],
        }
        first = notices.render_human_report(report)
        second = notices.render_human_report(report)
        self.assertEqual(first, second)
        self.assertNotIn("Generated at", first)


if __name__ == "__main__":
    unittest.main()
