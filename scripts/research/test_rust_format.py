"""Controls for complete selection, bounded argv and failure propagation."""
from pathlib import Path
from types import SimpleNamespace
import unittest

from rust_format import batches, format_roots, target_roots


class FormatTests(unittest.TestCase):
    def test_batched_selection_loses_no_targets(self):
        roots = [("2021", "x" * 151 + str(i)) for i in range(600)]
        selected = [path for _, group in batches(roots) for path in group]
        self.assertEqual(selected, [path for _, path in roots])
        for _, group in batches(roots):
            self.assertLessEqual(sum(len(p.encode("utf-16-le")) // 2 + 3 for p in group), 6000)

    def test_mixed_editions_remain_separate(self):
        self.assertEqual(list(batches([("2018", "a"), ("2021", "b")])),
                         [("2018", ["a"]), ("2021", ["b"])])

    def test_early_failure_not_hidden_by_later_success(self):
        calls = []
        def run(command, **_):
            calls.append(command)
            return SimpleNamespace(returncode=1 if len(calls) == 1 else 0)
        self.assertEqual(format_roots([("2018", "a"), ("2021", "b")], "1.98.0", True, run), 1)
        self.assertEqual(len(calls), 2)
        self.assertTrue(all("--check" in command for command in calls))

    def test_empty_and_incomplete_metadata_rejected(self):
        for metadata in ({"workspace_members": [], "packages": []},
                         {"workspace_members": ["missing"], "packages": []}):
            with self.assertRaises(ValueError):
                target_roots(metadata)

    def test_missing_target_is_not_a_skip(self):
        metadata = {"workspace_members": ["p"], "packages": [{"id": "p",
            "manifest_path": str(Path(__file__).parent / "Cargo.toml"), "dependencies": [],
            "targets": [{"edition": "2021", "src_path": str(Path(__file__).parent / "missing.rs")}]}]}
        with self.assertRaises(ValueError):
            target_roots(metadata)


if __name__ == "__main__":
    unittest.main()
