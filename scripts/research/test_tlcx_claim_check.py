"""日本語: claim整合の負例。English: bounded claim-consistency negatives."""
from __future__ import annotations

import copy
import hashlib
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import tlcx_claim_check as checker


class Tlcx41ClaimCheckTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        (self.root / "docs").mkdir()
        self.text = "試験は呼出側の成功申告です。\nTests are reported successful by the caller.\n"
        (self.root / "docs/record.md").write_bytes(self.text.encode())
        source = {"path": "docs/record.md", "sha256": hashlib.sha256(self.text.encode()).hexdigest()}
        digest = hashlib.sha256((source["path"] + "\t" + source["sha256"] + "\n").encode()).hexdigest()
        self.bundle = {
            "schema_version": checker.INPUT_SCHEMA,
            "source_session_digest": digest,
            "formal_ready": False,
            "sources": [source],
            "observations": [{"id": "run", "source_paths": [source["path"]],
                "kind": "execution", "outcome": "reported_pass", "basis": "caller_reported",
                "command": ["python", "example.py"]}],
            "claims": [{"id": "claim", "ja": {"source_path": source["path"],
                "start_line": 1, "end_line": 1, "text": self.text.splitlines()[0],
                "status": "reported_pass", "evidence_ids": ["run"]},
                "en": {"source_path": source["path"], "start_line": 2, "end_line": 2,
                "text": self.text.splitlines()[1], "status": "reported_pass", "evidence_ids": ["run"]}}],
        }

    def run_bundle(self, bundle=None):
        data = self.bundle if bundle is None else bundle
        return checker.check_claim_bundle(json.dumps(data, ensure_ascii=False).encode(), self.root)

    def assert_issue(self, code, bundle=None):
        report = self.run_bundle(bundle)
        self.assertIn(code, [issue["code"] for issue in report["issues"]])
        self.assertEqual(report["status"], "needs_evidence")
        self.assertFalse(report["formal_ready"])
        self.assertFalse(report["independent_execution_checked"])
        return report

    def test_tlcx41_reported_pass_is_only_declared_consistency(self):
        report = self.run_bundle()
        self.assertEqual(report["status"], "consistent_with_declared_records")
        self.assertEqual(report["observations"][0]["outcome"], "reported_pass")
        self.assertEqual(report["observations"][0]["basis"], "caller_reported")
        self.assertFalse(report["independent_execution_checked"])
        self.assertFalse(report["formal_ready"])
        self.assertFalse(report["atomic_snapshot"])

    def test_tlcx41_source_tamper_is_stale_and_does_not_drop_observation(self):
        (self.root / "docs/record.md").write_bytes(b"changed\n")
        report = self.assert_issue("source_sha256_mismatch")
        self.assertEqual(report["observations"][0]["outcome"], "reported_pass")
        self.assertFalse(report["claims"][0]["source_binding_current"])

    def test_tlcx41_source_session_swap_is_rejected(self):
        self.bundle["source_session_digest"] = "0" * 64
        self.assert_issue("source_session_digest_mismatch")

    def test_tlcx41_bilingual_status_mismatch_is_not_semantic_alignment(self):
        self.bundle["claims"][0]["en"]["status"] = "unknown"
        self.assert_issue("bilingual_status_or_evidence_mismatch")

    def test_tlcx41_wrong_excerpt_is_not_accepted_by_hash_alone(self):
        self.bundle["claims"][0]["ja"]["text"] = "証明済み"
        self.assert_issue("claim_text_mismatch")

    def test_tlcx41_authored_unrun_does_not_satisfy_reported_pass(self):
        self.bundle["observations"][0].update(kind="authoring", outcome="authored", command=[])
        self.assert_issue("claim_outcome_mismatch")

    def test_tlcx41_unknown_partial_unsupported_remain_explicit(self):
        for outcome in ("unknown", "partial", "unsupported", "pending", "unrun", "reported_fail"):
            with self.subTest(outcome=outcome):
                bundle = copy.deepcopy(self.bundle)
                bundle["observations"][0]["outcome"] = outcome
                for language in ("ja", "en"):
                    bundle["claims"][0][language]["status"] = outcome
                report = self.run_bundle(bundle)
                self.assertEqual(report["observations"][0]["outcome"], outcome)
                self.assertEqual(report["claims"][0]["reported_status"], outcome)
                self.assertFalse(report["formal_ready"])

    def test_tlcx41_promoted_execution_basis_and_closure_are_rejected(self):
        for key, value in (("basis", "independently_verified"), ("outcome", "observed_pass")):
            with self.subTest(key=key):
                bundle = copy.deepcopy(self.bundle)
                bundle["observations"][0][key] = value
                self.assert_issue("invalid_observation", bundle)
        self.bundle["formal_ready"] = True
        self.assert_issue("formal_ready_promotion_forbidden")

    def test_tlcx41_empty_sources_observations_claims_do_not_vacuously_succeed(self):
        for key in ("sources", "observations", "claims"):
            with self.subTest(key=key):
                bundle = copy.deepcopy(self.bundle)
                bundle[key] = []
                self.assert_issue("empty_or_oversize_" + key, bundle)

    def test_tlcx41_missing_and_duplicate_evidence_rejected(self):
        self.bundle["claims"][0]["en"]["evidence_ids"] = ["missing"]
        self.assert_issue("unknown_observation_reference")
        self.bundle["observations"].append(copy.deepcopy(self.bundle["observations"][0]))
        self.assert_issue("duplicate_observation_id")

    def test_tlcx41_duplicate_case_path_and_parent_escape_rejected(self):
        self.bundle["sources"].append({"path": "docs/RECORD.md", "sha256": "0" * 64})
        self.assert_issue("duplicate_source_path")
        self.bundle["sources"][0]["path"] = "../secret.md"
        self.assert_issue("invalid_source_path")

    def link_case(self, kind):
        outside = self.root / "outside.md"
        outside.write_text("outside", encoding="utf-8")
        target = self.root / (kind + ".md")
        try:
            target.symlink_to(outside) if kind == "symlink" else os.link(outside, target)
        except (OSError, NotImplementedError) as exc:
            self.skipTest(f"{kind} fixture unavailable: {type(exc).__name__}")
        bundle = copy.deepcopy(self.bundle)
        bundle["sources"][0]["path"] = target.name
        report = self.run_bundle(bundle)
        self.assertEqual(report["sources"][0]["state"], "unsupported")
        self.assertIsNone(report["sources"][0]["current_sha256"])

    def test_tlcx41_symlink_is_not_followed(self):
        self.link_case("symlink")

    def test_tlcx41_hardlink_is_not_followed(self):
        self.link_case("hardlink")

    def test_tlcx41_strict_json_duplicate_key_unknown_field_and_nan(self):
        for payload, code in ((b'{"x":1,"x":2}', "invalid_json"),
                              (b'{"x":NaN}', "invalid_json")):
            report = checker.check_claim_bundle(payload, self.root)
            self.assertIn(code, [issue["code"] for issue in report["issues"]])
        self.bundle["completion_percent"] = 65
        self.assert_issue("invalid_bundle_fields")

    def test_tlcx41_read_budget_and_invalid_line_span_are_not_zero(self):
        with patch.object(checker, "MAX_TOTAL_SOURCE_BYTES", 1):
            self.assert_issue("source_read_budget")
        self.bundle["claims"][0]["ja"]["start_line"] = 0
        self.assert_issue("invalid_claim_variant")

    def test_tlcx41_oversize_and_excessive_json_depth_are_typed(self):
        report = checker.check_claim_bundle(b"x" * (checker.MAX_BUNDLE_BYTES + 1), self.root)
        self.assertEqual(report["issues"][0]["code"], "bundle_byte_limit")
        report = checker.check_claim_bundle(b"[" * 129 + b"0" + b"]" * 129, self.root)
        self.assertEqual(report["issues"][0]["code"], "json_nesting_limit")

    def test_tlcx41_root_change_clears_all_current_claim_bindings(self):
        with patch.object(checker.snapshot._RootLease, "verify_current",
                          side_effect=checker.snapshot._CheckError("repository_root_changed_during_check")):
            report = self.assert_issue("repository_root_changed_during_check")
        self.assertFalse(report["claims"][0]["source_binding_current"])

    def test_tlcx41_cli_returns_consistency_only_and_never_executes_command(self):
        (self.root / "bundle.json").write_text(json.dumps(self.bundle), encoding="utf-8")
        before = (self.root / "docs/record.md").read_bytes()
        output = io.StringIO()
        with patch.object(checker.sys, "stdout", output):
            status = checker.main(["--repo", str(self.root), "--bundle", "bundle.json"])
        self.assertEqual(status, 0)
        self.assertEqual(json.loads(output.getvalue())["status"], "consistent_with_declared_records")
        self.assertEqual(before, (self.root / "docs/record.md").read_bytes())

    def test_tlcx41_diagnostic_cap_remains_incomplete(self):
        with patch.object(checker, "MAX_ISSUES", 1):
            self.bundle["claims"][0]["ja"]["evidence_ids"] = ["a", "b", "c"]
            report = self.assert_issue("unknown_observation_reference")
        self.assertEqual(len(report["issues"]), 1)
        self.assertTrue(report["issues_truncated"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
