"""Tiny fake-loader ownership checks; no model, decoder, GPU or network."""
from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock

HELPER = Path(__file__).with_name("beta7_media_worker.py")
SPEC = importlib.util.spec_from_file_location("beta7_lifetime_fixture", HELPER)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
backend = fixture.backend


class ModelSnapshotLifetime(unittest.TestCase):
    def layout(self, base):
        cache = base / "cache"
        selected = cache / "chosen"
        selected.mkdir(parents=True)
        (selected / "model.bin").write_bytes(b"A")
        request = base / "request"
        request.mkdir()
        registration = {
            "LC631_MEDIA_MODEL_ROOT_SHA256": backend.tree_manifest_digest(cache),
            "LC631_MEDIA_ASR_DIRECTORY": "chosen",
            "LC631_MEDIA_ASR_SUBTREE_SHA256": backend.tree_manifest_digest(selected),
        }
        return cache, selected, request, registration

    def test_private_subtree_lives_through_lazy_read_and_fallback_then_is_removed(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-lifetime-") as tmp:
            cache, original, request, registration = self.layout(Path(tmp))
            case = self
            constructed = []
            read_bytes = []

            class Factory:
                def __init__(self, directory, **kwargs):
                    self.folder = Path(directory)
                    self.first_attempt = kwargs.get("device") == "cuda"
                    constructed.append(self.folder)
                    case.assertNotEqual(self.folder, original)
                    case.assertEqual(self.folder.parent, request)
                    case.assertEqual((self.folder / "model.bin").read_bytes(), b"A")

                def transcribe(self, *_args, **_kwargs):
                    def segments():
                        # The read happens during iteration, after transcribe returns.
                        case.assertTrue(self.folder.is_dir())
                        if self.first_attempt:
                            raise RuntimeError("synthetic first iterator failure")
                        read_bytes.append((self.folder / "model.bin").read_bytes())
                        yield types.SimpleNamespace(text="synthetic", start=0, end=0.1)
                    return segments(), None

            module = types.ModuleType("faster_whisper")
            module.WhisperModel = Factory
            observations, executed, diagnostics = [], set(), []
            with (mock.patch.dict(sys.modules, {"faster_whisper": module}),
                  mock.patch.dict(os.environ, registration)):
                backend.analyze_asr(
                    cache / "not-opened.wav", cache, observations, executed, diagnostics,
                    temporary_root=request, remaining_temp_bytes=1,
                )
            self.assertEqual(read_bytes, [b"A"])
            self.assertEqual(len(constructed), 2)
            self.assertEqual(len(set(constructed)), 1, "retry changed the admitted path")
            self.assertTrue(all(not path.exists() for path in constructed))
            self.assertEqual(list(request.iterdir()), [], "private snapshot remained after use")
            self.assertEqual((original / "model.bin").read_bytes(), b"A")
            self.assertIn("asr", executed)
            self.assertEqual([item["label"] for item in observations], ["synthetic"])

    def test_constructor_failure_releases_only_the_private_snapshot(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-failure-") as tmp:
            cache, original, request, registration = self.layout(Path(tmp))
            case = self
            attempted = []

            class Factory:
                def __init__(self, directory, **_kwargs):
                    folder = Path(directory)
                    attempted.append(folder)
                    case.assertEqual((folder / "model.bin").read_bytes(), b"A")
                    raise RuntimeError("synthetic loader failed")

            module = types.ModuleType("faster_whisper")
            module.WhisperModel = Factory
            with (mock.patch.dict(sys.modules, {"faster_whisper": module}),
                  mock.patch.dict(os.environ, registration)):
                with self.assertRaisesRegex(RuntimeError, "synthetic loader failed"):
                    backend.analyze_asr(
                        cache / "not-opened.wav", cache, [], set(), [],
                        temporary_root=request, remaining_temp_bytes=1,
                    )
            self.assertEqual(len(attempted), 2)
            self.assertTrue(all(not path.exists() for path in attempted))
            self.assertEqual(list(request.iterdir()), [], "failed loader left its private snapshot")
            self.assertEqual((original / "model.bin").read_bytes(), b"A")


if __name__ == "__main__":
    unittest.main()
