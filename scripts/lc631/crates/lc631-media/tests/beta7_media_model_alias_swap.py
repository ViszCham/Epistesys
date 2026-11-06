"""Configured-alias replacement during a fake loader's use; synthetic bytes only."""
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
SPEC = importlib.util.spec_from_file_location("beta7_alias_swap_fixture", HELPER)
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)
backend = fixture.backend


class ModelRootAliasSwap(unittest.TestCase):
    def test_replaced_alias_cannot_redirect_loading_or_return_a_successful_analysis(self):
        with tempfile.TemporaryDirectory(prefix="beta7-model-alias-swap-") as tmp:
            base = Path(tmp)
            actual, foreign, request = base / "actual", base / "foreign", base / "request"
            for folder, payload in ((actual, b"A"), (foreign, b"B")):
                folder.mkdir()
                (folder / "model.bin").write_bytes(payload)
            request.mkdir()
            alias = base / "configured-cache"
            try:
                alias.symlink_to(actual, target_is_directory=True)
            except OSError as error:
                if os.name == "nt":
                    self.skipTest(f"Windows symlink capability unavailable: {error.winerror}")
                raise
            digest = backend.tree_manifest_digest(actual)
            registration = {
                "LC631_MEDIA_MODEL_ROOT_SHA256": digest,
                "LC631_MEDIA_ASR_DIRECTORY": ".",
                "LC631_MEDIA_ASR_SUBTREE_SHA256": digest,
            }
            loaded, staged_paths = [], []

            class Factory:
                def __init__(self, directory, **_kwargs):
                    folder = Path(directory)
                    staged_paths.append(folder)
                    alias.unlink()
                    alias.symlink_to(foreign, target_is_directory=True)
                    loaded.append((folder / "model.bin").read_bytes())

                def transcribe(self, *_args, **_kwargs):
                    return [], None

            module = types.ModuleType("faster_whisper")
            module.WhisperModel = Factory
            with (mock.patch.dict(sys.modules, {"faster_whisper": module}),
                  mock.patch.dict(os.environ, registration)):
                with self.assertRaisesRegex(ValueError, "configured model root occurrence changed"):
                    backend.analyze_asr(
                        alias / "not-opened.wav", alias, [], set(), [],
                        temporary_root=request, remaining_temp_bytes=1,
                    )
            self.assertEqual(loaded, [b"A"], "the new alias target reached the loader")
            self.assertEqual(alias.resolve(), foreign.resolve())
            self.assertTrue(all(not path.exists() for path in staged_paths))
            self.assertEqual(list(request.iterdir()), [])
            self.assertEqual((actual / "model.bin").read_bytes(), b"A")
            self.assertEqual((foreign / "model.bin").read_bytes(), b"B")


if __name__ == "__main__":
    unittest.main()
