"""Independent wire-shape vectors, not journal or authority evidence."""
import copy
import json
from pathlib import Path
import unittest

from jsonschema import Draft202012Validator


class BundleInspectionSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[2]
        schema = json.loads((root / "schemas/lc631-journal-bundle-inspect.v1.schema.json").read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        cls.validator = Draft202012Validator(schema)

    def positive(self):
        return {
            "schema_version": "lc631-journal-bundle-inspect.v1",
            "status": "inspected_unanchored", "hold_code": None,
            "segment_count": 1, "record_count": 2, "segment_input_bytes": 512,
            "latest_record_sequence": 2, "local_head_frame_digest_bytes": list(range(32)),
            "head_state": "unanchored", "rollback_resistance": "unavailable",
            "fresh_revalidation_required": True, "external_anchor_observed": False,
            "authority_created": False, "completion_restored": False,
            "output_commit_allowed": False, "diagnostic_only": True,
            "claim_boundary": "local bytes only; not proof or permission",
        }

    def held(self):
        result = self.positive()
        result.update(status="hold", hold_code="bundle_rebuild_required")
        for name in ("segment_count", "record_count", "segment_input_bytes",
                     "latest_record_sequence", "local_head_frame_digest_bytes", "head_state"):
            result[name] = None
        return result

    def test_tlcx07_schema_accepts_explicit_unanchored_and_held_shapes(self):
        self.validator.validate(self.positive())
        self.validator.validate(self.held())
        held = self.held()
        held["hold_code"] = "unexpected_write_may_have_applied"
        self.validator.validate(held)

    def test_tlcx07_schema_rejects_authority_and_completion_promotions(self):
        for field in ("authority_created", "completion_restored", "output_commit_allowed", "external_anchor_observed"):
            with self.subTest(field=field):
                report = self.positive()
                report[field] = True
                self.assertFalse(self.validator.is_valid(report))

    def test_tlcx07_schema_rejects_unknown_fields_and_profile_changes(self):
        for field, value in (("proof_verified", True), ("status", "accepted"),
                             ("rollback_resistance", "verified"), ("diagnostic_only", False),
                             ("schema_version", "other.v1")):
            with self.subTest(field=field):
                report = self.positive()
                report[field] = value
                self.assertFalse(self.validator.is_valid(report))

    def test_tlcx07_schema_rejects_invalid_counts_and_digest_shape(self):
        for field, value in (("segment_count", 0), ("segment_count", 1001),
                             ("record_count", True), ("record_count", 131073),
                             ("segment_input_bytes", 16777217),
                             ("local_head_frame_digest_bytes", [0] * 31),
                             ("local_head_frame_digest_bytes", [256] * 32)):
            with self.subTest(field=field, value=value):
                report = self.positive()
                report[field] = value
                self.assertFalse(self.validator.is_valid(report))

    def test_tlcx07_schema_preserves_unobserved_not_zero(self):
        for field in ("segment_count", "record_count", "segment_input_bytes", "latest_record_sequence"):
            for value in (0, 1):
                with self.subTest(field=field, value=value):
                    report = self.held()
                    report[field] = value
                    self.assertFalse(self.validator.is_valid(report))
        report = self.positive()
        report["record_count"] = None
        self.assertFalse(self.validator.is_valid(report))

    def test_tlcx07_schema_requires_all_declared_fields_and_status_consistency(self):
        valid = self.positive()
        for field in valid:
            with self.subTest(field=field):
                report = copy.deepcopy(valid)
                del report[field]
                self.assertFalse(self.validator.is_valid(report))
        positive = self.positive()
        positive["hold_code"] = "resource_hold"
        self.assertFalse(self.validator.is_valid(positive))
        held = self.held()
        held["hold_code"] = None
        self.assertFalse(self.validator.is_valid(held))


if __name__ == "__main__":
    unittest.main()
