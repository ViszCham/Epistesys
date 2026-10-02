"""Validate local legacy fixtures and captured production surfaces, not semantics."""
import argparse
import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator


def read_json(path: Path):
    if path.is_symlink() or path.stat().st_size > 16 * 1024 * 1024:
        raise ValueError("unsafe or oversized JSON input")
    return json.loads(path.read_text(encoding="utf-8-sig"))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--captures", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    schema = read_json(root / "schemas/epistesys-dgcl-surfaces.v1.schema.json")
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema)
    files = sorted(root.glob("fixtures/dgcl-legacy-*.json"))
    files += sorted(root.glob("fixtures/dgcl-checkpoint-*.json"))
    captures = sorted(args.captures.glob("*.json"))
    if len(captures) < 6:
        raise ValueError("production capture set incomplete")
    files += captures
    checked = rejected = 0
    for path in files:
        payload = read_json(path)
        validator.validate(payload)
        if json.loads(json.dumps(payload, ensure_ascii=False)) != payload:
            raise ValueError("JSON roundtrip changed payload")
        checked += 1
        if payload.get("schema_version", "").startswith("epistesys-dgcl-package-") and "candidate" in payload:
            wrong = copy.deepcopy(payload)
            wrong["candidate"]["output_commit_allowed"] = True
            if validator.is_valid(wrong):
                raise ValueError("schema accepted unauthorized candidate output")
            rejected += 1
        for view in payload.get("consumer_views", {}).values():
            validator.validate(view)
            wrong = copy.deepcopy(view)
            wrong["host_send_authorized"] = True
            if validator.is_valid(wrong):
                raise ValueError("consumer schema accepted permission amplification")
            checked += 1
            rejected += 1
    print(json.dumps({"schema": "epistesys-dgcl-schema-check.v1", "checked": checked,
                      "rejected_mutations": rejected, "claim_boundary": "local schema/roundtrip only"}))


if __name__ == "__main__":
    main()
