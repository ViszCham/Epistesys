"""Compatibility checks for the model-root snapshot profile; synthetic bytes only."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import tempfile
import unittest
import os

HELPER = Path(__file__).with_name("beta7_media_worker.py")
SPEC = importlib.util.spec_from_file_location("beta7_frozen_model_fixture", HELPER)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)


class ModelRootAliases(unittest.TestCase):
    def test_root_alias_keeps_existing_single_model_call_compatible(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-root-alias-") as tmp:
            base = Path(tmp)
            actual = base / "actual"
            actual.mkdir()
            (actual / "model.bin").write_bytes(b"A")
            alias = base / "cache-alias"
            try:
                alias.symlink_to(actual, target_is_directory=True)
            except OSError as error:
                if os.name == "nt":
                    self.skipTest(f"Windows symlink capability unavailable: {error.winerror}")
                raise
            case = fixture.MediaWorkerBoundary()
            self.assertEqual(case.model_attempt(actual, 10), [b"A"], "regular root control")
            self.assertEqual(case.model_attempt(alias, 10), [b"A"],
                             "a stable configured root alias was rejected instead of pinned once")

    def test_linked_member_is_still_rejected_before_model_construction(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-member-alias-") as tmp:
            base = Path(tmp)
            actual = base / "cache"
            actual.mkdir()
            original = base / "model-bytes"
            original.write_bytes(b"outside")
            try:
                (actual / "model.bin").symlink_to(original)
            except OSError as error:
                if os.name == "nt":
                    self.skipTest(f"Windows symlink capability unavailable: {error.winerror}")
                raise
            case = fixture.MediaWorkerBoundary()
            self.assertEqual(case.model_attempt(actual, 10), [],
                             "a linked member was admitted as a registered regular model file")


if __name__ == "__main__":
    unittest.main()
