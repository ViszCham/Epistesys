"""Local resource-contract checks. Fixtures and subprocesses are owned by this test."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import wave

spec = importlib.util.spec_from_file_location("media_resources_backend", Path(__file__).with_name("media_backend.py"))
backend = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backend)


class ResourceTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="epistesys-owned-resource-test-"))
        self.source = self.root / "owned.wav"
        with wave.open(str(self.source), "wb") as stream:
            stream.setnchannels(1)
            stream.setsampwidth(2)
            stream.setframerate(16000)
            stream.writeframes(bytes(8000))
        self.output = self.root / "decoded"
        self.output.mkdir()
        self.saved = (backend.MAX_TEMP_BYTES, backend.MAX_FRAMES, backend.MAX_PCM_SAMPLES)
        backend.MAX_FRAMES = 1
        backend.MAX_PCM_SAMPLES = 4000

    def tearDown(self):
        backend.MAX_TEMP_BYTES, backend.MAX_FRAMES, backend.MAX_PCM_SAMPLES = self.saved
        # Leave owned artifacts for inspection; never delete user data.

    def ffmpeg(self):
        path = shutil.which("ffmpeg")
        if not path:
            self.skipTest("local ffmpeg unavailable; positive decode not observed")
        return Path(path)

    def test_unsatisfiable_temp_budget_rejected_before_any_file_is_written(self):
        backend.MAX_TEMP_BYTES = 1
        with self.assertRaises(ValueError):
            backend.extract_media(Path("must-not-start-unsatisfiable-budget"), self.source, self.output, [])
        self.assertEqual(list(self.output.iterdir()), [])

    def test_text_runner_does_not_capture_unbounded_subprocess_output(self):
        with self.assertRaises(RuntimeError):
            backend.run_checked([sys.executable, "-c", "print('x'*1000000)"], stdout_limit=128)

    def test_exact_pcm_budget_and_positive_decode(self):
        backend.MAX_TEMP_BYTES = 9000
        frames, audio = backend.extract_media(self.ffmpeg(), self.source, self.output, [])
        self.assertEqual(frames, [])
        self.assertIsNotNone(audio)
        with wave.open(str(audio), "rb") as stream:
            self.assertLessEqual(stream.getnframes(), 4000)
        self.assertLessEqual(backend.tree_size(self.output), 9000)

    def test_ppm_frame_limit_rejects_before_writing_a_second_frame(self):
        arena = backend._TempArena(self.output, 64)
        sink = backend._PpmFrames(arena, 1)
        frame = b"P6\n2 2\n255\n" + bytes(12)
        with self.assertRaises(backend.ResourceBudgetExceeded):
            sink.feed(frame + frame)
        self.assertEqual(len(list(self.output.iterdir())), 1)
        self.assertLessEqual(backend.tree_size(self.output), 64)

    def test_ppm_dimensions_are_checked_before_pixel_allocation_or_write(self):
        sink = backend._PpmFrames(backend._TempArena(self.output, 64), 1)
        with self.assertRaises(backend.ResourceBudgetExceeded):
            sink.feed(b"P6\n640 641\n255\n")
        self.assertEqual(list(self.output.iterdir()), [])

    def test_pcm_sample_limit_rejects_before_file_creation(self):
        sink = backend._PcmAudio(backend._TempArena(self.output, 64), 4)
        with self.assertRaises(backend.ResourceBudgetExceeded):
            sink.feed(bytes(10))
        self.assertEqual(list(self.output.iterdir()), [])

    def test_partial_pcm_sample_is_not_committed_as_complete_audio(self):
        sink = backend._PcmAudio(backend._TempArena(self.output, 64), 4)
        sink.feed(bytes(3))
        with self.assertRaises(ValueError):
            sink.finish()
        self.assertLessEqual(backend.tree_size(self.output), 64)

    def test_pipe_deadline_is_bounded_without_reader_threads(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            backend.run_checked([sys.executable, "-c", "import time;time.sleep(10)"], timeout=0.1)

    def test_nonfinite_or_unbounded_runner_budgets_are_rejected_before_spawn(self):
        for timeout in (float("nan"), float("inf"), -1):
            with self.assertRaises(ValueError):
                backend.run_checked(["must-not-start-invalid-budget"], timeout=timeout)
        with self.assertRaises(ValueError):
            backend.run_checked(["must-not-start-invalid-budget"], stdout_limit=1 << 40)

    def test_invalid_utf8_backend_output_is_visible_not_silently_replaced(self):
        with self.assertRaises(UnicodeDecodeError):
            backend.run_checked([sys.executable, "-c", "import sys;sys.stdout.buffer.write(bytes([255]))"])

    @unittest.skipIf(os.name == "nt", "Windows Job scope is observed by the Rust supervisor test")
    def test_unix_worker_limit_is_observed_as_address_space_not_rss(self):
        code = ("import json,sys;sys.path.insert(0,sys.argv[1]);import media_backend;"
                "print(json.dumps(media_backend.configure_worker_resource_limits()))")
        result = backend.run_checked([sys.executable, "-c", code, str(Path(__file__).resolve().parent)])
        observed = json.loads(result.stdout)
        self.assertTrue(observed["memory_limit_enforced"])
        self.assertEqual(observed["memory_limit_kind"], "per_process_address_space")
        self.assertLessEqual(observed["memory_limit_bytes"], 4 * 1024 * 1024 * 1024)
        self.assertFalse(observed["rss_hard_limit"])
        self.assertFalse(observed["aggregate_process_tree_limit"])


if __name__ == "__main__":
    unittest.main()
