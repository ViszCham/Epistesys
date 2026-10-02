import unittest

import dgcl_nlp_worker as worker


class RuntimeLockTests(unittest.TestCase):
    def test_exact_distribution_set_accepts_pep503_name_normalization(self):
        lock = b"Stanza==1.14.0\nSome_Pkg==2.0\n"
        actual = {"stanza": "1.14.0", "some-pkg": "2.0"}
        self.assertEqual(worker.validate_distribution_set(lock, actual), 2)

    def test_exact_distribution_set_rejects_missing_extra_and_version_drift(self):
        lock = b"stanza==1.14.0\nsome-pkg==2.0\n"
        for actual in (
            {"stanza": "1.14.0"},
            {"stanza": "1.14.0", "some-pkg": "2.0", "extra": "1"},
            {"stanza": "1.14.0", "some-pkg": "2.1"},
        ):
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                worker.validate_distribution_set(lock, actual)

    def test_lock_rejects_ranges_direct_references_and_duplicate_names(self):
        invalid_locks = (
            b"stanza>=1.14\n",
            b"stanza @ https://example.invalid/stanza.whl\n",
            b"Some_Pkg==2.0\nsome-pkg==2.0\n",
            b"# empty lock\n",
        )
        for lock in invalid_locks:
            with self.subTest(lock=lock), self.assertRaises(ValueError):
                worker.parse_distribution_lock(lock)


if __name__ == "__main__":
    unittest.main()
