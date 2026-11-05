"""Fail-closed tests for rechecking historical TLCX source snapshots."""

from __future__ import annotations

import hashlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from typing import Any
from unittest.mock import patch

import tlcx_snapshot_check as checker
from tlcx_snapshot_check import (
    MAX_FILE_COUNT,
    MAX_FILE_SIZE_BYTES,
    MAX_MANIFEST_BYTES,
    check_manifest,
)


SCHEMA = "epistesys-tlcx-source-snapshot.v1"
MANIFEST_REL = "validation/tlcx/test-source-manifest.json"


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _aggregate(entries: list[dict[str, Any]]) -> str:
    rows = "".join(
        f"{entry['path']}\t{entry['sha256']}\n"
        for entry in sorted(entries, key=lambda item: item["path"])
    )
    return _sha256(rows.encode("utf-8"))


def _write_manifest(
    root: Path,
    files: dict[str, bytes],
    *,
    order: list[str] | None = None,
    hash_overrides: dict[str, str] | None = None,
    byte_fields: bool = True,
) -> Path:
    for relative, content in files.items():
        target = root.joinpath(*relative.split("/"))
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)

    paths = order if order is not None else list(files)
    entries: list[dict[str, Any]] = []
    for relative in paths:
        content = files.get(relative, b"")
        entry: dict[str, Any] = {
            "path": relative,
            "sha256": (hash_overrides or {}).get(relative, _sha256(content)),
        }
        if byte_fields:
            entry["bytes"] = len(content)
        entries.append(entry)

    record = {
        "schema_version": SCHEMA,
        "slice": "TLCX-TEST",
        "recorded_at": "2026-10-29",
        "digest_contract": (
            "Sort repository-relative paths lexically; concatenate UTF-8 "
            "path<TAB>raw-file-sha256<LF> rows; SHA-256 the resulting UTF-8 bytes."
        ),
        "file_count": len(entries),
        "aggregate_sha256": _aggregate(entries),
        "files": entries,
    }
    manifest = root.joinpath(*MANIFEST_REL.split("/"))
    manifest.parent.mkdir(parents=True, exist_ok=True)
    manifest.write_text(json.dumps(record), encoding="utf-8")
    return manifest


class TlcxSnapshotCheckTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "repo"
        self.root.mkdir()

    def test_valid_current_snapshot_is_reported_without_mutation(self) -> None:
        original = {"src/a.rs": b"fn a() {}\n", "src/b.rs": b"fn b() {}\n"}
        manifest = _write_manifest(self.root, original, order=["src/b.rs", "src/a.rs"])
        manifest_bytes = manifest.read_bytes()
        source_bytes = (self.root / "src/a.rs").read_bytes()

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "internally_consistent")
        self.assertEqual(report["historical_evidence"]["state"], "recorded_manifest_only")
        self.assertFalse(report["historical_evidence"]["authenticated"])
        self.assertFalse(report["historical_evidence"]["rewritten"])
        self.assertEqual(report["current_validation"]["status"], "match")
        self.assertEqual(report["current_validation"]["consistency"], "per_file_non_atomic")
        self.assertFalse(report["current_validation"]["atomic_snapshot"])
        self.assertFalse(report["current_validation"]["release_or_slice_completion_claimed"])
        self.assertEqual(
            [item["path"] for item in report["current_validation"]["files"]],
            ["src/a.rs", "src/b.rs"],
        )
        self.assertEqual(manifest.read_bytes(), manifest_bytes)
        self.assertEqual((self.root / "src/a.rs").read_bytes(), source_bytes)
        self.assertNotIn(str(self.root), json.dumps(report))

    def test_changed_file_keeps_historical_and_current_states_separate(self) -> None:
        original = b"recorded bytes\n"
        _write_manifest(self.root, {"src/changed.rs": original})
        (self.root / "src/changed.rs").write_bytes(b"current bytes\n")

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "internally_consistent")
        self.assertEqual(report["current_validation"]["status"], "mismatch")
        item = report["current_validation"]["files"][0]
        self.assertEqual(item["status"], "changed_since_historical")
        self.assertEqual(item["historical_sha256"], _sha256(original))
        self.assertEqual(item["current_sha256"], _sha256(b"current bytes\n"))

    def test_root_generation_switch_between_files_cannot_report_match(self) -> None:
        _write_manifest(self.root, {"src/a.bin": b"a0", "src/b.bin": b"b0"})
        manifest = self.root.joinpath(*MANIFEST_REL.split("/"))
        record = json.loads(manifest.read_text(encoding="utf-8"))
        record["files"][1]["sha256"] = _sha256(b"b1")
        record["aggregate_sha256"] = _aggregate(record["files"])
        manifest.write_text(json.dumps(record), encoding="utf-8")

        replacement = Path(self.temporary.name) / "repo-generation-b"
        replacement_src = replacement / "src"
        replacement_src.mkdir(parents=True)
        (replacement_src / "a.bin").write_bytes(b"a1")
        (replacement_src / "b.bin").write_bytes(b"b1")
        retired = Path(self.temporary.name) / "repo-generation-a-retired"
        real_hash_file = checker._hash_current_file
        switched = False

        def switch_after_first_file(root: Path, entry: dict[str, Any], budget: Any, *lease: Any):
            nonlocal switched
            result = real_hash_file(root, entry, budget, *lease)
            if entry["path"] == "src/a.bin":
                moved_original = False
                try:
                    os.replace(self.root, retired)
                    moved_original = True
                    os.replace(replacement, self.root)
                    switched = True
                except OSError:
                    if moved_original and not self.root.exists():
                        os.replace(retired, self.root)
            return result

        try:
            with patch.object(checker, "_hash_current_file", side_effect=switch_after_first_file):
                report = check_manifest(self.root, MANIFEST_REL)
        finally:
            if switched:
                os.replace(self.root, replacement)
                os.replace(retired, self.root)

        post_release = Path(self.temporary.name) / "repo-after-release"
        os.replace(self.root, post_release)
        os.replace(post_release, self.root)

        self.assertEqual(report["historical_manifest"]["integrity"], "internally_consistent")
        self.assertNotEqual(report["current_validation"]["status"], "match")
        self.assertNotEqual(checker._exit_code(report), 0)
        self.assertEqual(report["current_validation"]["consistency"], "per_file_non_atomic")
        self.assertFalse(report["current_validation"]["atomic_snapshot"])
        self.assertFalse(report["current_validation"]["release_or_slice_completion_claimed"])
        if os.name == "nt":
            self.assertFalse(switched, "Windows root/ancestor lease did not block root replacement")
            self.assertEqual(
                report["current_validation"]["root_identity_status"],
                "windows_ancestors_pinned_through_scan",
            )
        else:
            self.assertTrue(switched, "POSIX replacement hook did not exercise final identity verification")
            self.assertEqual(report["current_validation"]["status"], "unsupported")
            self.assertEqual(
                report["current_validation"]["root_identity_status"], "changed_or_unverified"
            )
        if switched:
            self.assertIn("repository_root_changed_during_check", report["errors"])

    def test_missing_current_file_is_reported_without_hash(self) -> None:
        _write_manifest(self.root, {"src/missing.rs": b"historical\n"})
        (self.root / "src/missing.rs").unlink()

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "internally_consistent")
        self.assertEqual(report["current_validation"]["status"], "mismatch")
        item = report["current_validation"]["files"][0]
        self.assertEqual(item["status"], "missing_from_current")
        self.assertIsNone(item["current_sha256"])

    def test_malformed_hash_fails_before_current_files_are_checked(self) -> None:
        _write_manifest(
            self.root,
            {"src/a.rs": b"contents"},
            hash_overrides={"src/a.rs": "not-a-sha256"},
        )

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
        self.assertEqual(report["current_validation"]["status"], "not_run")
        self.assertIn("invalid_sha256", report["errors"])

    def test_path_traversal_absolute_and_windows_escapes_fail_closed(self) -> None:
        outside = Path(self.temporary.name) / "outside.txt"
        outside.write_text("must not be read", encoding="utf-8")
        for unsafe in (
            "../outside.txt",
            "/outside.txt",
            "C:\\outside.txt",
            "C:outside.txt",
            "\\outside.txt",
            "\\\\server\\share\\outside.txt",
            "..\\outside.txt",
        ):
            with self.subTest(path=unsafe):
                _write_manifest(self.root, {}, order=[unsafe])
                report = check_manifest(self.root, MANIFEST_REL)
                self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
                self.assertEqual(report["current_validation"]["status"], "not_run")
                self.assertIn("invalid_repository_relative_path", report["errors"])
                self.assertNotIn("must not be read", json.dumps(report))

    def test_absolute_or_escaping_manifest_location_is_not_emitted(self) -> None:
        outside_manifest = Path(self.temporary.name) / "outside.json"
        outside_manifest.write_text("{}", encoding="utf-8")

        for unsafe in (str(outside_manifest), "../outside.json", "C:\\outside.json"):
            with self.subTest(path=unsafe):
                report = check_manifest(self.root, unsafe)
                self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
                self.assertEqual(report["current_validation"]["status"], "not_run")
                self.assertNotIn(str(outside_manifest), json.dumps(report))

    def test_symlinked_requested_repository_root_is_unsupported(self) -> None:
        _write_manifest(self.root, {"src/a.rs": b"inside\n"})
        alias = Path(self.temporary.name) / "repo-alias"
        try:
            alias.symlink_to(self.root, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlink fixture unavailable: {type(error).__name__}")

        report = check_manifest(alias, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "unsupported")
        self.assertEqual(report["current_validation"]["status"], "not_run")
        self.assertIn("reparse_or_symlink_unsupported", report["errors"])
        self.assertNotIn(str(alias), json.dumps(report))

    def test_oversized_manifest_is_rejected_before_json_parse(self) -> None:
        manifest = self.root.joinpath(*MANIFEST_REL.split("/"))
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_bytes(b" " * (MAX_MANIFEST_BYTES + 1))

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
        self.assertIn("manifest_too_large", report["errors"])
        self.assertEqual(report["current_validation"]["status"], "not_run")

    def test_file_count_limit_is_enforced_before_target_reads(self) -> None:
        entries = [f"src/{index:04d}.rs" for index in range(MAX_FILE_COUNT + 1)]
        _write_manifest(self.root, {}, order=entries)

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
        self.assertIn("file_count_limit_exceeded", report["errors"])
        self.assertEqual(report["current_validation"]["status"], "not_run")

    def test_file_size_limit_is_reported_without_hashing_oversized_file(self) -> None:
        target = self.root / "src" / "huge.bin"
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open("wb") as stream:
            stream.seek(MAX_FILE_SIZE_BYTES)
            stream.write(b"x")
        _write_manifest(
            self.root,
            {},
            order=["src/huge.bin"],
            hash_overrides={"src/huge.bin": "0" * 64},
        )

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "internally_consistent")
        self.assertEqual(report["current_validation"]["status"], "unsupported")
        self.assertEqual(
            report["current_validation"]["files"][0]["status"], "unsupported"
        )
        self.assertIn("file_size_limit_exceeded", report["errors"])
        self.assertIsNone(report["current_validation"]["files"][0]["current_sha256"])

    def test_duplicate_paths_are_rejected_including_windows_case_aliases(self) -> None:
        for duplicate_paths in (
            ["src/a.rs", "src/a.rs"],
            ["src/a.rs", "src/A.rs"],
        ):
            with self.subTest(paths=duplicate_paths):
                _write_manifest(self.root, {}, order=duplicate_paths)
                report = check_manifest(self.root, MANIFEST_REL)
                self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
                self.assertIn("duplicate_path", report["errors"])
                self.assertEqual(report["current_validation"]["status"], "not_run")

    def test_symlink_escape_is_unsupported_and_never_followed(self) -> None:
        outside = Path(self.temporary.name) / "outside"
        outside.mkdir()
        (outside / "secret.rs").write_text("outside bytes", encoding="utf-8")
        link = self.root / "linked"
        try:
            link.symlink_to(outside, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlink fixture unavailable: {type(error).__name__}")
        _write_manifest(
            self.root,
            {},
            order=["linked/secret.rs"],
            hash_overrides={"linked/secret.rs": _sha256(b"outside bytes")},
        )

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "internally_consistent")
        self.assertEqual(report["current_validation"]["status"], "unsupported")
        self.assertIn("reparse_or_symlink_unsupported", report["errors"])
        self.assertIsNone(report["current_validation"]["files"][0]["current_sha256"])
        self.assertNotIn("outside bytes", json.dumps(report))

    def test_file_symlink_escape_is_unsupported(self) -> None:
        outside = Path(self.temporary.name) / "outside.rs"
        outside.write_bytes(b"outside\n")
        target = self.root / "src" / "linked.rs"
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            target.symlink_to(outside)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlink fixture unavailable: {type(error).__name__}")
        _write_manifest(
            self.root,
            {},
            order=["src/linked.rs"],
            hash_overrides={"src/linked.rs": _sha256(b"outside\n")},
        )

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["current_validation"]["status"], "unsupported")
        self.assertIn("reparse_or_symlink_unsupported", report["errors"])
        self.assertIsNone(report["current_validation"]["files"][0]["current_sha256"])

    def test_manifest_symlink_escape_is_unsupported_before_reading(self) -> None:
        outside = Path(self.temporary.name) / "outside-manifest.json"
        outside.write_text("{\"secret\":\"outside bytes\"}", encoding="utf-8")
        manifest = self.root.joinpath(*MANIFEST_REL.split("/"))
        manifest.parent.mkdir(parents=True, exist_ok=True)
        try:
            manifest.symlink_to(outside)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlink fixture unavailable: {type(error).__name__}")

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "unsupported")
        self.assertEqual(report["current_validation"]["status"], "not_run")
        self.assertIn("reparse_or_symlink_unsupported", report["errors"])
        self.assertNotIn("outside bytes", json.dumps(report))

    def test_hard_link_alias_is_unsupported(self) -> None:
        outside = Path(self.temporary.name) / "outside-hardlink.rs"
        outside.write_bytes(b"shared bytes\n")
        target = self.root / "src" / "aliased.rs"
        target.parent.mkdir(parents=True, exist_ok=True)
        try:
            os.link(outside, target)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"hard-link fixture unavailable: {type(error).__name__}")
        _write_manifest(
            self.root,
            {},
            order=["src/aliased.rs"],
            hash_overrides={"src/aliased.rs": _sha256(b"shared bytes\n")},
        )

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["current_validation"]["status"], "unsupported")
        self.assertIn("hardlink_alias_unsupported", report["errors"])
        self.assertEqual(report["current_validation"]["files"][0]["current_sha256"], None)

    @unittest.skipUnless(os.name == "nt", "Windows directory-handle pinning")
    def test_windows_ancestor_rename_is_denied_while_file_lease_is_open(self) -> None:
        _write_manifest(self.root, {"src/a.rs": b"pinned\n"})
        directory = self.root / "src"
        moved = self.root / "src-moved"
        opened = checker._open_file_beneath(self.root, ("src", "a.rs"))
        rename_blocked = False
        try:
            try:
                os.rename(directory, moved)
            except OSError:
                rename_blocked = True
            self.assertGreaterEqual(getattr(opened, "guard_count", 0), 1)
        finally:
            if hasattr(opened, "close"):
                opened.close()
            else:
                checker._close_fd(opened)
        if moved.exists() and not directory.exists():
            os.rename(moved, directory)
        self.assertTrue(rename_blocked, "ancestor rename succeeded while validation lease was open")
        os.rename(directory, moved)
        os.rename(moved, directory)

    def test_partial_read_failures_share_and_consume_the_total_budget(self) -> None:
        _write_manifest(
            self.root,
            {"src/a.bin": b"aaaa", "src/b.bin": b"bbbb"},
        )
        real_hash_file = checker._hash_current_file
        real_os_read = os.read
        read_sizes: list[int] = []

        def fail_second_chunk_for_first_file(
            root: Path,
            entry: dict[str, Any],
            budget: Any,
            root_lease: Any = None,
        ) -> tuple[str, int]:
            if entry["path"] != "src/a.bin":
                return real_hash_file(root, entry, budget, root_lease)

            reads = 0

            def injected_read(fd: int, size: int) -> bytes:
                nonlocal reads
                read_sizes.append(size)
                reads += 1
                if reads == 2:
                    raise OSError("injected read failure")
                return real_os_read(fd, size)

            with patch.object(checker.os, "read", side_effect=injected_read):
                return real_hash_file(root, entry, budget, root_lease)

        with patch.object(checker, "MAX_TOTAL_FILE_BYTES", 5), patch.object(
            checker, "READ_CHUNK_BYTES", 2
        ), patch.object(checker, "_hash_current_file", side_effect=fail_second_chunk_for_first_file):
            report = check_manifest(self.root, MANIFEST_REL)

        current = report["current_validation"]
        self.assertEqual(current["status"], "unsupported")
        self.assertEqual(current["unsupported_count"], 2)
        self.assertEqual(current["bytes_read"], 2)
        self.assertEqual(current["bytes_limit"], 5)
        self.assertTrue(read_sizes)
        self.assertLessEqual(max(read_sizes), 2)

    def test_reporting_order_is_sorted_and_repeatable(self) -> None:
        files = {"z/file.rs": b"z", "a/file.rs": b"a", "m/file.rs": b"m"}
        _write_manifest(self.root, files, order=["z/file.rs", "m/file.rs", "a/file.rs"])

        first = check_manifest(self.root, MANIFEST_REL)
        second = check_manifest(self.root, MANIFEST_REL)

        first_files = first["current_validation"]["files"]
        self.assertEqual([item["path"] for item in first_files], ["a/file.rs", "m/file.rs", "z/file.rs"])
        self.assertEqual(first_files, second["current_validation"]["files"])

    def test_manifest_aggregate_and_count_must_match_its_recorded_entries(self) -> None:
        _write_manifest(self.root, {"src/a.rs": b"a"})
        manifest = self.root.joinpath(*MANIFEST_REL.split("/"))
        record = json.loads(manifest.read_text(encoding="utf-8"))
        record["aggregate_sha256"] = "0" * 64
        record["file_count"] = 2
        manifest.write_text(json.dumps(record), encoding="utf-8")

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
        self.assertIn("aggregate_sha256_mismatch", report["errors"])
        self.assertIn("file_count_mismatch", report["errors"])
        self.assertEqual(report["current_validation"]["status"], "not_run")

    def test_zero_entry_manifest_is_invalid_and_cli_exits_nonzero(self) -> None:
        _write_manifest(self.root, {}, order=[])

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
        self.assertEqual(report["current_validation"]["status"], "not_run")
        self.assertIn("empty_file_list", report["errors"])
        output = io.StringIO()
        with patch.object(checker.sys, "stdout", output):
            exit_code = checker.main(["--repo", str(self.root), "--manifest", MANIFEST_REL])
        self.assertNotEqual(exit_code, 0)
        self.assertEqual(
            json.loads(output.getvalue())["current_validation"]["status"], "not_run"
        )

    def test_excessive_json_nesting_is_invalid_and_cli_exits_nonzero(self) -> None:
        manifest = self.root.joinpath(*MANIFEST_REL.split("/"))
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_bytes(
            b"{" + b'"a":{' * 10000 + b'"x":0' + b"}" * 10001
        )

        report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
        self.assertEqual(report["current_validation"]["status"], "not_run")
        self.assertIn("manifest_nesting_limit_exceeded", report["errors"])
        output = io.StringIO()
        with patch.object(checker.sys, "stdout", output):
            exit_code = checker.main(["--repo", str(self.root), "--manifest", MANIFEST_REL])
        self.assertNotEqual(exit_code, 0)
        self.assertEqual(
            json.loads(output.getvalue())["current_validation"]["status"], "not_run"
        )

    def test_json_recursion_error_is_typed_invalid_and_cli_exits_nonzero(self) -> None:
        _write_manifest(self.root, {"src/a.rs": b"a"})

        with patch.object(checker.json, "loads", side_effect=RecursionError):
            report = check_manifest(self.root, MANIFEST_REL)

        self.assertEqual(report["historical_manifest"]["integrity"], "invalid")
        self.assertEqual(report["current_validation"]["status"], "not_run")
        self.assertIn("manifest_nesting_limit_exceeded", report["errors"])
        output = io.StringIO()
        with patch.object(checker.sys, "stdout", output), patch.object(
            checker.json, "loads", side_effect=RecursionError
        ):
            exit_code = checker.main(["--repo", str(self.root), "--manifest", MANIFEST_REL])
        self.assertNotEqual(exit_code, 0)
        self.assertEqual(
            json.loads(output.getvalue())["current_validation"]["status"], "not_run"
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
