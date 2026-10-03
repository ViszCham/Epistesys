"""Negative tests for aggregate consistency, not benchmark grading accuracy."""

import copy
import importlib.util
from pathlib import Path
import unittest


SPEC = importlib.util.spec_from_file_location(
    "verify_gb_cc75", Path(__file__).with_name("verify-gb-cc75.py")
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class AggregateChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = MODULE.load_bundle(MODULE.DEFAULT_BUNDLE)

    def rejected(self, mutate):
        data = copy.deepcopy(self.data)
        mutate(data)
        with self.assertRaises(ValueError):
            MODULE.validate_aggregates(data)

    def test_current_public_aggregates(self):
        MODULE.verify_hashes(MODULE.DEFAULT_BUNDLE)
        MODULE.validate_aggregates(self.data)

    def test_duplicate_pair_is_rejected(self):
        self.rejected(lambda d: d["pairs"].__setitem__(1, d["pairs"][0].copy()))

    def test_duplicate_qid_is_rejected(self):
        self.rejected(lambda d: d["qids"].__setitem__(1, d["qids"][0].copy()))

    def test_out_of_range_pass_is_rejected(self):
        self.rejected(lambda d: d["pairs"][0].update(direct_passed="9999"))

    def test_nonfinite_fraction_is_rejected(self):
        self.rejected(lambda d: d["pairs"][0].update(direct_fraction="nan"))

    def test_forged_strict_is_rejected(self):
        self.rejected(lambda d: d["pairs"][0].update(direct_strict="false"))

    def test_changed_qid_mean_is_rejected(self):
        self.rejected(lambda d: d["qids"][0].update(labyrinth_mean="0.5"))

    def test_changed_summary_is_rejected(self):
        self.rejected(
            lambda d: d["summary"]["automated_result"]["primary"].update(delta=0.0)
        )

    def test_changed_missingness_is_rejected(self):
        self.rejected(lambda d: d["missingness"].update(successful_pair_count=168))

    def test_changed_manifest_is_rejected(self):
        manifest = copy.deepcopy(self.data["manifest"])
        manifest["files"][0]["sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            MODULE.verify_hashes(MODULE.DEFAULT_BUNDLE, manifest)

    def test_manifest_traversal_is_rejected(self):
        manifest = copy.deepcopy(self.data["manifest"])
        manifest["files"][0]["path"] = "../README.md"
        with self.assertRaises(ValueError):
            MODULE.verify_hashes(MODULE.DEFAULT_BUNDLE, manifest)


if __name__ == "__main__":
    unittest.main()
