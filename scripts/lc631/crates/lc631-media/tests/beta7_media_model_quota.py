"""Tiny selected-subtree quota checks; no real weights, model or GPU."""
from __future__ import annotations
import importlib.util
from pathlib import Path
import tempfile
import types
import sys
import unittest
from unittest import mock

PARENT = Path(__file__).with_name("beta7_media_worker.py")
SPEC = importlib.util.spec_from_file_location("beta7_quota_fixture", PARENT)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
backend = fixture.backend


class SelectedModelQuota(unittest.TestCase):
    def test_sparse_cache_stages_only_the_selected_subtree(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-sparse-") as tmp:
            root = Path(tmp)
            selected = root / "chosen"
            selected.mkdir()
            (selected / "model.bin").write_bytes(b"A")
            unused = root / "unused"
            unused.mkdir()
            (unused / "blob.bin").write_bytes(b"U" * 32768)
            uses = []
            class Factory:
                def __init__(self, directory, **_kwargs):
                    folder = Path(directory)
                    uses.append((sorted(p.relative_to(folder).as_posix() for p in folder.rglob("*")),
                                 (folder / "model.bin").read_bytes()))
                def transcribe(self, *_args, **_kwargs):
                    return [], None
            module = types.ModuleType("faster_whisper")
            module.WhisperModel = Factory
            registration = {
                "LC631_MEDIA_MODEL_ROOT_SHA256": backend.tree_manifest_digest(root),
                "LC631_MEDIA_ASR_DIRECTORY": "chosen",
                "LC631_MEDIA_ASR_SUBTREE_SHA256": backend.tree_manifest_digest(selected),
            }
            with (mock.patch.dict(sys.modules, {"faster_whisper": module}),
                  mock.patch.dict("os.environ", registration),
                  mock.patch.object(backend, "MAX_MODEL_ENTRIES", 10),
                  mock.patch.object(backend, "MAX_TEMP_BYTES", 1)):
                backend.analyze_asr(root / "unused-audio.wav", root, [], set(), [])
            self.assertEqual(uses, [(["model.bin"], b"A")],
                             "unselected cache payload consumed the one-byte snapshot allowance")

    def test_tiny_metadata_allowance_does_not_bypass_selected_data_quota(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-quota-") as tmp:
            root = Path(tmp)
            (root / "chosen").mkdir()
            (root / "chosen/model.bin").write_bytes(b"12345")
            case = fixture.MediaWorkerBoundary()
            with mock.patch.object(backend, "MAX_TEMP_BYTES", 4):
                self.assertEqual(case.model_attempt(root, 10), [],
                                 "five selected bytes were loaded under a four-byte snapshot cap")

    def test_cache_root_itself_requires_explicit_occurrence_registration(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-root-registration-") as tmp:
            root = Path(tmp)
            (root / "model.bin").write_bytes(b"A")
            (root / "unselected-payload.bin").write_bytes(b"unselected")
            case = fixture.MediaWorkerBoundary()
            self.assertEqual(case.model_attempt(root, 10), [],
                             "an unregistered cache-root selection staged arbitrary cache contents")


if __name__ == "__main__":
    unittest.main()
