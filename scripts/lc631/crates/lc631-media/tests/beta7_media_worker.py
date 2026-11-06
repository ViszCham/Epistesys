"""Exercise the packaged worker with local synthetic fragments and a model constructor spy."""
from __future__ import annotations

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest import mock
import wave


WORKER = Path(__file__).resolve().parents[4] / "media_backend.py"
SPEC = importlib.util.spec_from_file_location("epistesys_fixture_worker", WORKER)
backend = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(backend)


class MediaWorkerBoundary(unittest.TestCase):
    def temporal_result(self, start, end, *, origin_known=True):
        with tempfile.TemporaryDirectory(prefix="beta7-media-time-") as tmp:
            root = Path(tmp)
            source = root / "source.bin"
            source.write_bytes(b"synthetic")
            ffmpeg = root / ("ffmpeg.exe" if os.name == "nt" else "ffmpeg")
            ffmpeg.touch()
            ffmpeg.with_name("ffprobe" + ffmpeg.suffix).touch()
            models = root / "models"
            models.mkdir()
            streams = [
                {"index": i, "codec_type": kind, "time_base": "1/1000",
                 "start_pts": 0, "duration_ts": 20000}
                for i, kind in enumerate(("video", "audio"))
            ]
            if not origin_known:
                streams[1].pop("start_pts")

            def extracted(_ffmpeg, _media, folder, _diagnostics):
                frame = folder / "frame-00000.ppm"
                frame.write_bytes(b"P6\n1 1\n255\n\0\0\0")
                audio = folder / "audio.wav"
                with wave.open(str(audio), "wb") as out:
                    out.setparams((1, 2, 16000, 0, "NONE", "not compressed"))
                    out.writeframes(b"\0\0" * 8000)
                return [frame], audio

            def transcript(_audio, _cache, observations, executed, _diagnostics, *args, **kwargs):
                backend.add_observation(observations, "transcript_candidate", start, end, "synthetic span")
                executed.add("asr")
                return "synthetic-asr"

            argv = [str(WORKER), "--media", str(source), "--ffmpeg", str(ffmpeg),
                    "--model-cache", str(models), "--max-temp-bytes", "1048576",
                    "--max-frames", "2", "--max-pcm-samples", "16000",
                    "--max-observations", "16"]
            stdout = io.StringIO()
            with (mock.patch.object(sys, "argv", argv),
                  mock.patch.object(backend, "configure_worker_resource_limits", return_value={}),
                  mock.patch.object(backend, "probe_media", return_value={"streams": streams}),
                  mock.patch.object(backend, "extract_media", side_effect=extracted),
                  mock.patch.object(backend, "analyze_vision"),
                  mock.patch.object(backend, "analyze_asr", side_effect=transcript),
                  mock.patch.object(backend, "analyze_diarization"),
                  contextlib.redirect_stdout(stdout)):
                self.assertEqual(backend.main(), 0)
            return json.loads(stdout.getvalue())

    def test_temporal_disjoint_and_touching_samples_do_not_publish(self):
        for start, end in [(10000, 11000), (500, 600)]:
            with self.subTest(interval=(start, end)):
                report = self.temporal_result(start, end)
                self.assertIn("asr", report["executed_families"])
                self.assertNotIn("temporal_co_presence", report["executed_families"],
                                 "family claimed despite no half-open interval intersection")
                self.assertFalse(any(o["kind"] == "temporal_co_presence_candidate"
                                     for o in report["observations"]))

    def test_temporal_unknown_clock_does_not_publish(self):
        report = self.temporal_result(100, 650, origin_known=False)
        self.assertNotIn("temporal_co_presence", report["executed_families"],
                         "family claimed without a common stream origin")

    def test_temporal_overlap_binds_both_fragments_and_intersection(self):
        report = self.temporal_result(100, 650)
        self.assertIn("temporal_co_presence", report["executed_families"])
        transcript, pair = report["observations"]
        self.assertEqual((pair["start_ms"], pair["end_ms"]), (100, 500),
                         "expected intersection of [100,650) and the actual sampled [0,500) frame")
        evidence = pair["temporal_overlap"]
        self.assertEqual(evidence["transcript_id"], transcript["observation_id"])
        self.assertTrue(evidence["frame_digest"].startswith("sha256:"))
        self.assertEqual((evidence["frame_start_ms"], evidence["frame_end_ms"]), (0, 500))

    def model_attempt(self, root, entry_cap, *, before_load=None):
        attempts = []
        class FakeWhisper:
            def __init__(self, directory, **kwargs):
                if before_load is not None:
                    before_load()
                attempts.append((Path(directory) / "model.bin").read_bytes())
            def transcribe(self, *args, **kwargs):
                return [], None
        fake = types.ModuleType("faster_whisper")
        fake.WhisperModel = FakeWhisper
        with (mock.patch.dict(sys.modules, {"faster_whisper": fake}),
              mock.patch.object(backend, "MAX_MODEL_ENTRIES", entry_cap)):
            try:
                backend.analyze_asr(root / "fixture.wav", root, [], set(), [])
            except (ValueError, backend.ResourceBudgetExceeded):
                pass
        return attempts

    def test_model_discovery_requires_one_unambiguous_occurrence_before_load(self):
        with tempfile.TemporaryDirectory(prefix="beta7-media-models-") as tmp:
            root = Path(tmp)
            first = root / "a-model"
            first.mkdir()
            (first / "model.bin").write_bytes(b"A")
            self.assertEqual(self.model_attempt(root, 10), [b"A"], "single model's actual loader bytes")
            second = root / "z-model"
            second.mkdir()
            (second / "model.bin").write_bytes(b"Z")
            self.assertEqual(self.model_attempt(root, 10), [],
                             "lexicographic first model was loaded without a selection")

    def test_model_discovery_checks_whole_root_entry_cap_before_load(self):
        with tempfile.TemporaryDirectory(prefix="beta7-media-model-cap-") as tmp:
            root = Path(tmp)
            model = root / "only-model"
            model.mkdir()
            (model / "model.bin").write_bytes(b"M")
            (root / "outside-selected-subtree").touch()
            self.assertEqual(self.model_attempt(root, 2), [],
                             "model constructor ran after the whole cache exceeded two entries")

    def test_explicit_model_occurrence_can_select_nonfirst_directory(self):
        with tempfile.TemporaryDirectory(prefix="beta7-media-pinned-model-") as tmp:
            root = Path(tmp)
            for name, payload in [("a-model", b"A"), ("z-model", b"Z")]:
                (root / name).mkdir()
                (root / name / "model.bin").write_bytes(payload)
            registration = {
                "LC631_MEDIA_MODEL_ROOT_SHA256": backend.tree_manifest_digest(root),
                "LC631_MEDIA_ASR_DIRECTORY": "z-model",
                "LC631_MEDIA_ASR_SUBTREE_SHA256": backend.tree_manifest_digest(root / "z-model"),
            }
            with mock.patch.dict(os.environ, registration):
                self.assertEqual(self.model_attempt(root, 10), [b"Z"],
                                 "the registered non-first model occurrence was not loaded")

    def test_model_constructor_reads_the_admitted_bytes_after_source_replacement(self):
        with tempfile.TemporaryDirectory(prefix="beta7-media-model-swap-") as tmp:
            root = Path(tmp)
            model = root / "model"
            model.mkdir()
            original = model / "model.bin"
            original.write_bytes(b"admitted model")
            pin = backend.tree_manifest_digest(root)
            # The model constructor is the use boundary. It changes only the
            # test-owned original cache, before reading its supplied input.
            with mock.patch.dict(os.environ, {"LC631_MEDIA_MODEL_ROOT_SHA256": pin}):
                observed = self.model_attempt(root, 10, before_load=lambda: original.write_bytes(b"swapped model!"))
            self.assertEqual(original.read_bytes(), b"swapped model!")
            self.assertEqual(observed, [b"admitted model"],
                             "model constructor reopened the mutable source instead of admitted bytes")


if __name__ == "__main__":
    unittest.main()
