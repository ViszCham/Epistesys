"""Physical bytes, not universal-newline text, define published NLP pins."""
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]


class PinnedByteTests(unittest.TestCase):
    def test_runtime_lock_and_worker_match_declared_byte_digests(self):
        pins = json.loads((ROOT / "validation/dgcl-nlp-model-pins.v1.json").read_text(encoding="utf-8-sig"))
        references = [pins["runtime_manifest"], pins["requirements_lock"],
                      {"path": "scripts/lc631/workers/dgcl_nlp_worker.py", "sha256": pins["worker_script_digest"]}]
        for reference in references:
            with self.subTest(path=reference["path"]):
                data = (ROOT / reference["path"]).read_bytes()
                self.assertNotIn(b"\r\n", data)
                self.assertEqual("sha256:" + hashlib.sha256(data).hexdigest(), reference["sha256"])


if __name__ == "__main__":
    unittest.main()
