"""Engineering schema regressions; validation is not receipt authentication."""
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parent.parent
DIGEST = "sha256:" + "1" * 64
SCHEMA = json.loads(
    (ROOT / "schemas/epistesys-dgcl-surfaces.v1.schema.json").read_text(encoding="utf-8-sig")
)


def connection():
    return {
        "schema_version": "epistesys-dgcl-connection-spec.v1",
        "integration_target": "connection",
        "test_name": "required_direct_api",
        "entrypoint": "fixture_package::checked_add",
        "implementation_path": "src/lib.rs",
        "driver_path": "tests/connection.rs",
        "driver_digest": DIGEST,
        "observer_id": "cargo-exact-direct-api-driver.v1",
        "oracle_id": "fixture-pinned-oracle",
        "oracle_revision": DIGEST,
        "coverage": "registered_driver_direct_public_api_and_pinned_assertion",
        "registration": None,
    }


def legacy_plan():
    return {
        "schema_version": "epistesys-dgcl-cargo-validation-plan.v1",
        "source_revision": DIGEST,
        "completion_plan_digest": DIGEST,
        "requirement_id": DIGEST,
        "task_id": DIGEST,
        "kind": "production_connection",
        "cargo_path": "fixture-toolchain/cargo",
        "cargo_digest": DIGEST,
        "manifest_path": "Cargo.toml",
        "manifest_digest": DIGEST,
        "repository_root_digest": DIGEST,
        "acceptance_target": None,
        "acceptance_test": None,
        "timeout_ms": 1000,
        "tool_registration": None,
    }


class EprSchemaTests(unittest.TestCase):
    def setUp(self):
        self.validator = Draft202012Validator(SCHEMA)

    def test_legacy_plan_without_connection_retains_its_shape(self):
        self.assertTrue(self.validator.is_valid(legacy_plan()))

    def test_registered_connection_roundtrips_inside_the_real_plan(self):
        plan = legacy_plan()
        plan["connection"] = connection()
        self.assertTrue(self.validator.is_valid(plan))
        plan["connection"] = None
        self.assertTrue(self.validator.is_valid(plan))

    def test_connection_contract_does_not_open_arbitrary_properties(self):
        for name, value in (
            ("observer_id", "unchecked-observer"),
            ("driver_digest", "not-a-digest"),
            ("driver_path", "../outside.rs"),
            ("coverage", "repository_wide_proof"),
            ("authority_created", True),
        ):
            with self.subTest(field=name):
                plan = legacy_plan()
                plan["connection"] = connection()
                plan["connection"][name] = value
                self.assertFalse(self.validator.is_valid(plan))
        plan = legacy_plan()
        plan["connection"] = connection()
        del plan["connection"]["oracle_revision"]
        self.assertFalse(self.validator.is_valid(plan))

    def test_embedded_and_standalone_connection_shapes_do_not_drift(self):
        standalone = json.loads(
            (ROOT / "schemas/epistesys-dgcl-connection-spec.v1.schema.json").read_text(
                encoding="utf-8-sig"
            )
        )
        expected = {k: v for k, v in standalone.items() if k not in {"$schema", "$id", "title"}}
        self.assertEqual(SCHEMA["$defs"]["registered_connection_spec"], expected)

    def test_historical_metadata_is_optional_and_typed_not_permission(self):
        resume = SCHEMA["$defs"]["package_resume"]
        for name in ("history_authenticated", "historical_head_renewed_after_fresh_validation"):
            with self.subTest(field=name):
                self.assertNotIn(name, resume["required"])
                field = Draft202012Validator(resume["properties"][name])
                self.assertTrue(field.is_valid(True))
                self.assertTrue(field.is_valid(False))
                self.assertFalse(field.is_valid("true"))
        for name in ("authority_created", "host_send_authorized", "output_commit_allowed"):
            field = Draft202012Validator(resume["properties"][name])
            self.assertFalse(field.is_valid(True))


if __name__ == "__main__":
    unittest.main()
