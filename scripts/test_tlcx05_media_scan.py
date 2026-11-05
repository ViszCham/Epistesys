"""Bounded model-scan contracts; no model, network or decoder is executed."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "tlcx05_media_backend", Path(__file__).with_name("media_backend.py")
)
backend = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backend)


class ModelScanContracts(unittest.TestCase):
    def fixture(self):
        # Retain tiny owned fixtures for inspection, matching this repository's
        # test-retention policy rather than recursively deleting user paths.
        return Path(tempfile.mkdtemp(prefix="epistesys-tlcx05-python-model-"))

    def test_flat_directory_count_is_checked_before_manifest_materialization(self):
        root = self.fixture()
        (root / "a").write_bytes(b"a")
        (root / "b").write_bytes(b"b")
        with self.assertRaises(backend.ResourceBudgetExceeded):
            backend.tree_manifest_digest(root, max_entries=1)

    def test_hashing_has_file_and_aggregate_actual_byte_caps(self):
        root = self.fixture()
        (root / "a").write_bytes(b"12345")
        (root / "b").write_bytes(b"12345")
        with self.assertRaises(backend.ResourceBudgetExceeded):
            backend.tree_manifest_digest(root, max_file_bytes=4)
        with self.assertRaises(backend.ResourceBudgetExceeded):
            backend.tree_manifest_digest(root, max_total_bytes=8)

    def test_license_read_is_bounded_before_read_text(self):
        root = self.fixture()
        (root / "README.md").write_bytes(b"license: " + b"x" * 100)
        with self.assertRaises(backend.ResourceBudgetExceeded):
            backend.model_card_license_hint(root, max_file_bytes=16)

    def test_bounded_manifest_keeps_the_legacy_digest(self):
        root = self.fixture()
        (root / "a").write_bytes(b"a")
        import hashlib
        basis = (
            'lc631-directory-manifest.v3\0[["a",1,"sha256:'
            'ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"]]'
        )
        expected = "sha256:" + hashlib.sha256(basis.encode("utf-8")).hexdigest()
        self.assertEqual(
            backend.tree_manifest_digest(root, max_entries=4, max_file_bytes=4), expected
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
