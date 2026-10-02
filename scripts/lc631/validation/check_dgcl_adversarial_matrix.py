from __future__ import annotations

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
MATRIX = ROOT / "fixtures" / "dgcl-adversarial-disconnect-matrix.v1.json"
EXPECTED_IDS = {f"A{index:02d}" for index in range(1, 17)}
ALLOWED_STATUS = {
    "covered_by_named_regression",
    "bounded_single_route_only",
    "caller_supplied_lifecycle_only",
    "trusted_head_api_only",
    "stage_contract_only",
}


def main() -> int:
    matrix = json.loads(MATRIX.read_text(encoding="utf-8"))
    if matrix.get("schema_version") != "epistesys-dgcl-adversarial-disconnect-matrix.v1":
        raise SystemExit("matrix_schema_version_mismatch")
    if matrix.get("independent_gold") is not False or matrix.get("repository_wide_proof") is not False:
        raise SystemExit("matrix_claim_boundary_invalid")
    cases = matrix.get("cases")
    if not isinstance(cases, list) or {case.get("id", "").split("-", 1)[0] for case in cases} != EXPECTED_IDS:
        raise SystemExit("matrix_must_cover_exactly_A01_through_A16")
    if len(cases) != 16:
        raise SystemExit("matrix_duplicate_or_missing_case")

    all_ids: set[str] = set()
    for case in cases:
        case_id = case["id"].split("-", 1)[0]
        if case_id in all_ids:
            raise SystemExit(f"duplicate_case:{case_id}")
        all_ids.add(case_id)
        if not case.get("mutation") or case.get("local_status") not in ALLOWED_STATUS:
            raise SystemExit(f"incomplete_case:{case_id}")
        refs = case.get("test_refs")
        if not isinstance(refs, list) or not refs:
            raise SystemExit(f"missing_test_reference:{case_id}")
        for reference in refs:
            relative_path, separator, test_name = reference.partition("::")
            if not separator or not test_name:
                raise SystemExit(f"invalid_test_reference:{case_id}:{reference}")
            target = (ROOT / relative_path).resolve()
            if ROOT not in target.parents:
                raise SystemExit(f"test_reference_outside_repository:{case_id}")
            source = target.read_text(encoding="utf-8")
            pattern = rf"\bfn\s+{re.escape(test_name)}\s*\("
            if re.search(pattern, source) is None:
                raise SystemExit(f"test_symbol_not_found:{case_id}:{test_name}")

    print("matrix=16_cases reference-integrity=pass claim-boundary=bounded local-regression-map=pass")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
