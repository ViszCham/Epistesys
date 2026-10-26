"""Portable wire/resource regression; detector stubs are not model validation."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest

ROOT = Path(__file__).resolve().parents[3]
BACKEND = ROOT / "scripts/media_backend.py"
module = types.ModuleType("media_backend_under_test")
if "--against-baseline" in sys.argv:
    sys.argv.remove("--against-baseline")
    source = subprocess.check_output([
        "git", "-C", str(ROOT), "show",
        "75500ed7a33a009f5e1ce813862dc0a7986372c9:scripts/media_backend.py",
    ]).decode("utf-8")
else:
    source = BACKEND.read_text(encoding="utf-8")
exec(compile(source, str(BACKEND), "exec"), module.__dict__)


class MediaWireTests(unittest.TestCase):
    def test_manifest_is_canonical_across_prefix_paths(self):
        with tempfile.TemporaryDirectory(prefix="epr-media-vector-") as temporary:
            root = Path(temporary)
            (root / "a").mkdir()
            (root / "a.txt").write_bytes(b"alpha")
            (root / "a/file.bin").write_bytes(b"beta")
            expected_entries = [
                ["a.txt", 5, "sha256:" + hashlib.sha256(b"alpha").hexdigest()],
                ["a/file.bin", 4, "sha256:" + hashlib.sha256(b"beta").hexdigest()],
            ]
            canonical = "lc631-directory-manifest.v3\0" + json.dumps(
                expected_entries, ensure_ascii=False, separators=(",", ":"))
            expected = "sha256:" + hashlib.sha256(canonical.encode("utf-8")).hexdigest()
            self.assertEqual(module.tree_manifest_digest(root), expected)

    def test_label_budget_is_utf8_bytes_and_retains_truncation(self):
        observations = []
        module.add_observation(observations, "transcript_candidate", 0, 1, "日" * 1500)
        observation = observations[0]
        self.assertLessEqual(len(observation["label"].encode("utf-8")), 4096)
        self.assertTrue(observation["label_truncated"])
        self.assertEqual(observation["original_label_bytes"], 4500)

    def test_observation_exhaustion_cannot_look_like_no_detection(self):
        old_limit = module.MAX_OBSERVATIONS
        try:
            module.MAX_OBSERVATIONS = 1
            observations = []
            module.add_observation(observations, "object_box", 0, 1, "first")
            with self.assertRaises(RuntimeError):
                module.add_observation(observations, "object_box", 0, 1, "unobserved")
        finally:
            module.MAX_OBSERVATIONS = old_limit

    def test_person_and_face_kinds_match_the_actual_method(self):
        class Hog:
            def setSVMDetector(self, _detector): pass
            def detectMultiScale(self, _image, **_kwargs): return ([[0, 0, 1, 2]], [0.8])
        class Face:
            def detectMultiScale(self, _gray, **_kwargs): return [[0, 0, 1, 1]]
        cv2 = types.SimpleNamespace(
            HOGDescriptor=Hog, HOGDescriptor_getDefaultPeopleDetector=lambda: None,
            data=types.SimpleNamespace(haarcascades="stub"), CascadeClassifier=lambda _: Face(),
            imread=lambda _: object(), cvtColor=lambda *_: object(), calcHist=lambda *_: object(),
            normalize=lambda *_: None, COLOR_BGR2GRAY=0)
        old_cv2, old_np = sys.modules.get("cv2"), sys.modules.get("numpy")
        try:
            sys.modules["cv2"], sys.modules["numpy"] = cv2, types.SimpleNamespace()
            observations = []
            module.analyze_vision([Path("stub-frame")], observations, set(), [])
            self.assertEqual([item["kind"] for item in observations], ["object_box", "face_region_candidate"])
        finally:
            for name, old in [("cv2", old_cv2), ("numpy", old_np)]:
                if old is None: sys.modules.pop(name, None)
                else: sys.modules[name] = old


if __name__ == "__main__":
    unittest.main()
