"""Fail-closed checks for the append-only TLCX design-revision record."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any


BASELINE_PATH = "validation/tlcx/baseline-registration-2026-10-28.json"
CHAIN_PATH = "validation/tlcx/design-revision-chain-2026-10-29.json"
ORIGINAL_ROADMAP_SHA256 = "aba6a886d28f43a84ad02c9ed781ce1666ab081869c13c00a583fd44ca17d5fc"
ORIGINAL_FORMULA_SHA256 = "1bd7540cbb53a907ecd84c1fe0ebbd6bcaf5d495bcf69f75170bf155db386d04"
PREVIOUS_MEASUREMENT_REVISION_ID = "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.1"
PREVIOUS_MEASUREMENT_SHA256 = "0fae4786111c2cb19a2c3d7746a2371df042103e2d48559ce7667260239a6e7d"
CURRENT_PREVIOUS_MEASUREMENT_REVISION_ID = "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.3"
CURRENT_PREVIOUS_MEASUREMENT_SHA256 = "7eb0a90898d1ae2f6f919aa689972e12ecf2ae32a4f6a9679a20ff4bef5b51fd"
CURRENT_MEASUREMENT_REVISION_ID = "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.4"
REGISTERED_MEASUREMENT_PINS = (
    (PREVIOUS_MEASUREMENT_REVISION_ID, PREVIOUS_MEASUREMENT_SHA256),
    (
        "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.2",
        "386d4012666e0a29514b3f9499987d8d49db5e9fb24e4a9066cf3e7089b29457",
    ),
    (CURRENT_PREVIOUS_MEASUREMENT_REVISION_ID, CURRENT_PREVIOUS_MEASUREMENT_SHA256),
    (
        CURRENT_MEASUREMENT_REVISION_ID,
        "3d1489b7e1320e85e48458c58caad8bd4907116e0d57a277cb94bd7a95dbb7fb",
    ),
)
CURRENT_OPERATIONAL_REVISION = "OperationalMeasurement/finite-formal-v1.4"
CURRENT_ROADMAP_REVISION_ID = "TLCX-ROADMAP-2026-10-29-MATH-REAUDIT"
MATH_REAUDIT_SOURCE_SHA256 = "c4a6021dfd59303bec73f45f3d50712c33a603781e43f08526605e1c5d76f28b"
PINNED_LF_FILES = (
    "docs/epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md",
    "docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md",
    "docs/tlcx-source-reading-ledger-2026-10-28.json",
)

AUDIT_RECONCILIATION_MARKERS = {
    "Japanese": (
        "finite-formal-v1.4",
        "[1−upperExp(dL/s_geo),1−lowerExp(dU/s_geo)]",
        "square([a,b])",
        "max(a²,b²)",
        "min(a²,b²)",
        "DistanceToFrontierUnion-v1",
        "NearestVerifiedCandidate-v1",
        *(f"G{number}" for number in range(31, 38)),
    ),
    "English": (
        "finite-formal-v1.4",
        "[1−upperExp(dL/s_geo),1−lowerExp(dU/s_geo)]",
        "square([a,b])",
        "max(a²,b²)",
        "min(a²,b²)",
        "DistanceToFrontierUnion-v1",
        "NearestVerifiedCandidate-v1",
        *(f"G{number}" for number in range(31, 38)),
    ),
}


def _read_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


def _object_field(record: dict[str, Any], key: str, errors: list[str]) -> dict[str, Any]:
    value = record.get(key, {})
    if isinstance(value, dict):
        return value
    errors.append(f"{key}_not_object")
    return {}


def _digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_checkout_eol(root: Path) -> list[str]:
    """Require LF checkout semantics for every byte-hashed design/registration file."""
    root = root.resolve()
    relative_paths = list(PINNED_LF_FILES)
    relative_paths.extend(
        path.relative_to(root).as_posix()
        for path in sorted((root / "validation/tlcx").glob("*.json"))
    )
    try:
        result = subprocess.run(
            ["git", "check-attr", "--stdin", "-z", "eol"],
            cwd=root,
            check=True,
            capture_output=True,
            input=b"".join(path.encode("utf-8") + b"\0" for path in relative_paths),
        )
    except (OSError, subprocess.CalledProcessError):
        return ["git_eol_attribute_check_unavailable"]

    errors: list[str] = []
    expected = {(path.encode("utf-8"), b"eol", b"lf") for path in relative_paths}
    fields = result.stdout.split(b"\0")
    terminated = fields.pop() == b""
    well_formed = terminated and len(fields) % 3 == 0
    actual = set(zip(fields[::3], fields[1::3], fields[2::3])) if well_formed else set()
    for path in relative_paths:
        if (path.encode("utf-8"), b"eol", b"lf") not in actual:
            errors.append(f"pinned_eol_policy_missing:{path}")
        try:
            if b"\r\n" in (root / path).read_bytes():
                errors.append(f"checkout_crlf_detected:{path}")
        except OSError:
            errors.append(f"pinned_file_unreadable:{path}")
    if not well_formed or actual != expected:
        errors.append("git_eol_attribute_result_mismatch")
    return sorted(set(errors))


def _bound_file(root: Path, entry: dict[str, Any], label: str, errors: list[str]) -> Path | None:
    raw_path = entry.get("path")
    if not isinstance(raw_path, str) or not raw_path:
        errors.append(f"{label}_path_missing")
        return None
    relative = Path(raw_path)
    if relative.is_absolute() or ".." in relative.parts:
        errors.append(f"{label}_path_invalid")
        return None
    target = (root / relative).resolve()
    try:
        target.relative_to(root.resolve())
    except ValueError:
        errors.append(f"{label}_path_escapes_repository")
        return None
    if not target.is_file():
        errors.append(f"{label}_missing")
        return None
    if entry.get("sha256") != _digest(target):
        errors.append(f"{label}_digest_mismatch")
    return target


def _validate_audit_reconciliation(contract_path: Path) -> list[str]:
    try:
        text = contract_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return ["math_audit_contract_unreadable"]
    sections = text.split("## English", maxsplit=1)
    if len(sections) != 2:
        return ["math_audit_bilingual_boundary_missing"]
    errors: list[str] = []
    for label, section, markers in (
        ("ja", sections[0], AUDIT_RECONCILIATION_MARKERS["Japanese"]),
        ("en", sections[1], AUDIT_RECONCILIATION_MARKERS["English"]),
    ):
        for marker in markers:
            if marker not in section:
                errors.append(f"math_audit_reconciliation_missing:{label}:{marker}")
        if "[1−upperExp(dU/s_geo),1−lowerExp(dL/s_geo)]" in section:
            errors.append(f"math_audit_reversed_n_geo_bounds:{label}")
    return errors


def validate_revision_chain(
    root: Path,
    chain: object | None = None,
    baseline: object | None = None,
) -> list[str]:
    """Check registered order, links and current bytes. Historical hashes are pins only."""
    root = root.resolve()
    errors: list[str] = []
    baseline_path = root / BASELINE_PATH
    try:
        baseline_record = baseline if baseline is not None else _read_json(baseline_path)
    except (OSError, json.JSONDecodeError):
        return ["baseline_registration_unavailable"]
    try:
        chain_record = chain if chain is not None else _read_json(root / CHAIN_PATH)
    except (OSError, json.JSONDecodeError):
        return ["design_revision_chain_unavailable"]

    if not isinstance(chain_record, dict):
        return ["chain_not_object"]
    if not isinstance(baseline_record, dict):
        return ["baseline_registration_not_object"]
    if chain_record.get("schema_version") != "epistesys-tlcx-design-revision-chain.v1":
        errors.append("chain_schema_mismatch")
    baseline_ref = _object_field(chain_record, "baseline_registration", errors)
    if baseline_ref.get("path") != BASELINE_PATH:
        errors.append("baseline_registration_path_mismatch")
    elif not baseline_path.is_file() or baseline_ref.get("sha256") != _digest(baseline_path):
        errors.append("baseline_registration_digest_mismatch")

    baseline_meta = _object_field(baseline_record, "baseline", errors)
    history = chain_record.get("history", [])
    if not isinstance(history, list):
        return sorted(set([*errors, "history_not_list"]))
    history_errors = []
    for index, item in enumerate(history):
        if not isinstance(item, dict):
            history_errors.append(f"history_entry_not_object:{index}")
        elif not isinstance(item.get("revision_id"), str) or not item["revision_id"]:
            history_errors.append(f"history_revision_id_invalid:{index}")
    if history_errors:
        return sorted(set(errors + history_errors))
    historical = next(
        (item for item in history if item.get("revision_id") == "TLCX-ROADMAP-2026-10-28"),
        None,
    )
    if historical is None or historical.get("sha256") != ORIGINAL_ROADMAP_SHA256:
        errors.append("historical_roadmap_pin_missing_or_changed")
    if baseline_meta.get("roadmap_sha256") != ORIGINAL_ROADMAP_SHA256:
        errors.append("original_baseline_registration_was_overwritten")
    if historical is not None and (
        historical.get("content_available") is not False
        or historical.get("content_claim") != "hash_pin_only"
    ):
        errors.append("historical_content_overclaimed")

    source_identity = _object_field(chain_record, "source_identity", errors)
    if source_identity.get("commit") != baseline_meta.get("commit"):
        errors.append("source_commit_mismatch")
    if source_identity.get("tree") != baseline_meta.get("tree"):
        errors.append("source_tree_mismatch")

    current = _object_field(chain_record, "current_design", errors)
    if current.get("revision_id") != CURRENT_MEASUREMENT_REVISION_ID:
        errors.append("current_design_revision_id_mismatch")
    if current.get("roadmap_revision_id") != CURRENT_ROADMAP_REVISION_ID:
        errors.append("current_roadmap_revision_id_mismatch")
    roadmap = _object_field(current, "roadmap", errors)
    contract = _object_field(current, "measurement_contract", errors)
    ledger = _object_field(current, "source_reading_ledger", errors)
    road_file = _bound_file(root, roadmap, "current_roadmap", errors)
    contract_file = _bound_file(root, contract, "current_measurement_contract", errors)
    _bound_file(root, ledger, "current_source_ledger", errors)

    if contract.get("operational_revision") != CURRENT_OPERATIONAL_REVISION:
        errors.append("operational_revision_mismatch")
    if contract.get("math_reaudit_source_sha256") != MATH_REAUDIT_SOURCE_SHA256:
        errors.append("math_reaudit_source_pin_mismatch")
    if contract.get("original_concept_sha256") != ORIGINAL_FORMULA_SHA256:
        errors.append("original_formula_reference_changed")
    if current.get("previous_revision_id") != CURRENT_PREVIOUS_MEASUREMENT_REVISION_ID:
        errors.append("current_previous_revision_id_mismatch")
    if current.get("previous_revision_sha256") != CURRENT_PREVIOUS_MEASUREMENT_SHA256:
        errors.append("current_previous_revision_mismatch")
    if current.get("current_roadmap_sha256") != roadmap.get("sha256"):
        errors.append("current_roadmap_chain_link_mismatch")

    revision_ids = [item.get("revision_id") for item in history]
    if len(revision_ids) != len(set(revision_ids)):
        errors.append("duplicate_revision_id")
    revision_by_id = {item.get("revision_id"): item for item in history}
    history_position = {item.get("revision_id"): index for index, item in enumerate(history)}
    measurement_history = [
        item for item in history if item["revision_id"].startswith("TLCX-MEASUREMENT-")
    ]
    expected_ids = [revision_id for revision_id, _ in REGISTERED_MEASUREMENT_PINS]
    if [item["revision_id"] for item in measurement_history] != expected_ids:
        errors.append("registered_measurement_order_invalid")
    for revision_id, expected_hash in REGISTERED_MEASUREMENT_PINS:
        entry = revision_by_id.get(revision_id)
        if entry is None:
            errors.append(f"registered_measurement_missing:{revision_id}")
        elif entry.get("sha256") != expected_hash:
            errors.append(f"registered_measurement_digest_mismatch:{revision_id}")
    if measurement_history and any(
        key in measurement_history[0]
        for key in ("previous_measurement_revision_id", "previous_measurement_revision_sha256")
    ):
        errors.append("unexpected_initial_measurement_predecessor")
    for previous, current_measurement in zip(measurement_history, measurement_history[1:]):
        current_id = current_measurement["revision_id"]
        predecessor_id = current_measurement.get("previous_measurement_revision_id")
        predecessor = (
            revision_by_id.get(predecessor_id) if isinstance(predecessor_id, str) else None
        )
        if predecessor is None:
            errors.append(f"measurement_predecessor_missing:{current_id}")
            continue
        if predecessor_id != previous["revision_id"]:
            errors.append(f"measurement_predecessor_not_immediate:{current_id}")
        if history_position.get(predecessor_id, len(history)) >= history_position.get(
            current_id, -1
        ):
            errors.append(f"measurement_predecessor_order_invalid:{current_id}")
        if current_measurement.get("previous_measurement_revision_sha256") != predecessor.get(
            "sha256"
        ):
            errors.append(f"measurement_predecessor_digest_mismatch:{current_id}")
    previous_measurement_revision = revision_by_id.get(PREVIOUS_MEASUREMENT_REVISION_ID)
    if (
        previous_measurement_revision is None
        or previous_measurement_revision.get("sha256") != PREVIOUS_MEASUREMENT_SHA256
    ):
        errors.append("previous_measurement_pin_missing_or_changed")
    current_previous_measurement_revision = revision_by_id.get(CURRENT_PREVIOUS_MEASUREMENT_REVISION_ID)
    if (
        current_previous_measurement_revision is None
        or current_previous_measurement_revision.get("sha256")
        != CURRENT_PREVIOUS_MEASUREMENT_SHA256
    ):
        errors.append("current_previous_measurement_pin_missing_or_changed")
    current_measurement_revision = revision_by_id.get(CURRENT_MEASUREMENT_REVISION_ID)
    current_roadmap_revision = revision_by_id.get(CURRENT_ROADMAP_REVISION_ID)
    if (
        current_measurement_revision is None
        or current_measurement_revision.get("sha256") != contract.get("sha256")
        or current_measurement_revision.get("operational_revision") != CURRENT_OPERATIONAL_REVISION
        or current_measurement_revision.get("previous_measurement_revision_id")
        != CURRENT_PREVIOUS_MEASUREMENT_REVISION_ID
        or current_measurement_revision.get("previous_measurement_revision_sha256")
        != CURRENT_PREVIOUS_MEASUREMENT_SHA256
        or current_measurement_revision.get("math_reaudit_source_sha256")
        != MATH_REAUDIT_SOURCE_SHA256
    ):
        errors.append("current_measurement_revision_missing_or_mismatch")
    if (
        current_roadmap_revision is None
        or current_roadmap_revision.get("sha256") != roadmap.get("sha256")
    ):
        errors.append("current_roadmap_revision_missing_or_mismatch")
    if not history or history[-1].get("revision_id") != current.get("revision_id"):
        errors.append("current_revision_not_last_in_chain")
    if road_file is None or contract_file is None:
        errors.append("current_design_binding_incomplete")
    if contract_file is not None:
        errors.extend(_validate_audit_reconciliation(contract_file))
    return sorted(set(errors))
