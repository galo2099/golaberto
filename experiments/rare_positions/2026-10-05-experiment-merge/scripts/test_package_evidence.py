#!/usr/bin/env python3
"""Synthetic safety checks for package_evidence prune verification."""
from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import stat
import tarfile
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock
from pathlib import Path

SCRIPT = Path(__file__).with_name("package_evidence.py")
SPEC = importlib.util.spec_from_file_location("package_evidence", SCRIPT)
packager = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(packager)


class PruneSafetyTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        base = Path(self.temp.name)
        self.repo = base / "repo"
        self.rare = self.repo / "experiments/rare_positions"
        self.root = "2026-10-05-prune-fixture"
        self.exp = self.rare / self.root
        self.exp.mkdir(parents=True)
        self.summary_path = self.rare / "2026-10-05-experiment-merge/integration-summary.json"
        self.summary_path.parent.mkdir(parents=True)
        packager.REPO = self.repo
        packager.RARE = self.rare
        packager.SUMMARY = self.summary_path

    def make_bundle(self, names=("raw-a.log", "raw-b.log"), duplicate=False):
        records = []
        for name in dict.fromkeys(names):
            path = self.exp / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(("original:" + name).encode())
            records.append(packager.item_metadata(path, name))
        archive = self.exp / packager.ARCHIVE_NAME
        with tarfile.open(archive, "w:gz") as tar:
            for name in names:
                source = self.exp / name
                tar.add(source, arcname=name, recursive=False)
                if duplicate:
                    tar.add(source, arcname=name, recursive=False)
        manifest = {
            "schema_version": 1,
            "experiment": self.root,
            "archive": packager.ARCHIVE_NAME,
            "archive_size": archive.stat().st_size,
            "archive_sha256": packager.digest(archive),
            "members": records,
        }
        (self.exp / packager.MANIFEST_NAME).write_text(json.dumps(manifest))
        archived = [f"experiments/rare_positions/{self.root}/{item['path']}" for item in records]
        self.summary_path.write_text(json.dumps({
            "schema_version": 1,
            "archived_paths": archived,
            "archives": [{
                "experiment": self.root,
                "archive": str(archive.relative_to(self.repo)),
                "manifest": str((self.exp / packager.MANIFEST_NAME).relative_to(self.repo)),
                "archive_sha256": manifest["archive_sha256"],
                "members": records,
            }],
        }))
        return archive, manifest

    def test_changed_original_aborts_before_any_removal(self):
        self.make_bundle()
        (self.exp / "raw-b.log").write_text("changed")
        with self.assertRaisesRegex(ValueError, "original changed after packaging"):
            packager.prune()
        self.assertTrue((self.exp / "raw-a.log").exists())
        self.assertTrue((self.exp / "raw-b.log").exists())

    def test_tampered_archive_aborts_without_removal(self):
        archive, _ = self.make_bundle()
        with archive.open("ab") as stream:
            stream.write(b"tampered")
        with self.assertRaisesRegex(ValueError, "archive hash mismatch"):
            packager.prune()
        self.assertTrue((self.exp / "raw-a.log").exists())
        self.assertTrue((self.exp / "raw-b.log").exists())

    def test_existing_prune_temp_aborts_before_any_unlink(self):
        self.make_bundle()
        temp_path = self.summary_path.with_suffix(".json.prune.tmp")
        temp_path.write_text("keep")
        with self.assertRaisesRegex(FileExistsError, "refusing prune"):
            packager.prune()
        self.assertTrue((self.exp / "raw-a.log").exists())
        self.assertTrue((self.exp / "raw-b.log").exists())
        self.assertEqual(temp_path.read_text(), "keep")

    def test_duplicate_tar_member_is_rejected(self):
        self.make_bundle(("raw-a.log",), duplicate=True)
        with self.assertRaisesRegex(ValueError, "duplicate tar member"):
            packager.prune()
        self.assertTrue((self.exp / "raw-a.log").exists())

    def test_unsafe_tar_member_is_rejected(self):
        archive, manifest = self.make_bundle(("raw-a.log",))
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(self.exp / "raw-a.log", arcname="raw-a.log", recursive=False)
            import io
            info = tarfile.TarInfo("../escape.txt")
            payload = b"unsafe"
            info.size = len(payload)
            tar.addfile(info, io.BytesIO(payload))
        manifest["archive_sha256"] = packager.digest(archive)
        manifest["archive_size"] = archive.stat().st_size
        (self.exp / packager.MANIFEST_NAME).write_text(json.dumps(manifest))
        record = json.loads(self.summary_path.read_text())
        record["archives"][0]["archive_sha256"] = manifest["archive_sha256"]
        self.summary_path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "unsafe relative path"):
            packager.prune()
        self.assertTrue((self.exp / "raw-a.log").exists())

    def test_symlink_ancestor_is_rejected(self):
        self.make_bundle(("nested/raw.log",))
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        (outside / "raw.log").write_text("original:nested/raw.log")
        shutil.rmtree(self.exp / "nested")
        os.symlink(outside, self.exp / "nested")
        with self.assertRaisesRegex(ValueError, "symlink in snapshot path"):
            packager.prune()
        self.assertTrue((outside / "raw.log").exists())

    def test_manifest_cannot_expand_summary_deletion_scope(self):
        archive, manifest = self.make_bundle(("raw-a.log",))
        extra = self.exp / "raw-b.log"
        extra.write_text("extra")
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(self.exp / "raw-a.log", arcname="raw-a.log", recursive=False)
            tar.add(extra, arcname="raw-b.log", recursive=False)
        manifest["members"].append(packager.item_metadata(extra, "raw-b.log"))
        manifest["archive_sha256"] = packager.digest(archive)
        manifest["archive_size"] = archive.stat().st_size
        (self.exp / packager.MANIFEST_NAME).write_text(json.dumps(manifest))
        summary = json.loads(self.summary_path.read_text())
        summary["archives"][0]["archive_sha256"] = manifest["archive_sha256"]
        self.summary_path.write_text(json.dumps(summary))
        with self.assertRaisesRegex(ValueError, "member list differs"):
            packager.prune()
        self.assertTrue((self.exp / "raw-a.log").exists())
        self.assertTrue(extra.exists())

    def test_unsafe_experiment_name_is_rejected(self):
        self.make_bundle(("raw-a.log",))
        summary = json.loads(self.summary_path.read_text())
        summary["archives"][0]["experiment"] = self.root + "/../../outside"
        self.summary_path.write_text(json.dumps(summary))
        with self.assertRaises(ValueError):
            packager.prune()
        self.assertTrue((self.exp / "raw-a.log").exists())

    def test_late_files_and_readable_candidates_are_never_pruned(self):
        self.make_bundle(("raw-a.log",))
        readable = self.exp / "analysis.md"
        readable.write_text("retained")
        late = self.exp / "added-after-snapshot.json"
        late.write_text("{}")
        with mock.patch.object(packager.subprocess, "run", return_value=SimpleNamespace(stdout="", returncode=0)):
            result = packager.prune()
        self.assertEqual(result["prune"]["removed_paths"], [
            f"experiments/rare_positions/{self.root}/raw-a.log",
        ])
        self.assertFalse((self.exp / "raw-a.log").exists())
        self.assertTrue(readable.exists())
        self.assertTrue(late.exists())
        self.assertTrue((self.exp / packager.MANIFEST_NAME).exists())


if __name__ == "__main__":
    unittest.main()
