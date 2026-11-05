"""日本語: 有界claim整合検査。English: bounded consistency, never proof.

Observations are caller-reported. Source hashes and strict JSON do not establish
independent execution, semantic truth, bilingual equivalence or formal closure.
The existing snapshot checker supplies anchored read-only filesystem primitives.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
from typing import Any

import tlcx_snapshot_check as snapshot

INPUT_SCHEMA = "epistesys-tlcx-claim-bundle.v1"
OUTPUT_SCHEMA = "epistesys-tlcx-claim-check.v1"
MAX_BUNDLE_BYTES = 1_048_576
MAX_SOURCE_BYTES = 1_048_576
MAX_TOTAL_SOURCE_BYTES = 8_388_608
MAX_ITEMS = 256
MAX_TEXT_CHARS = 16_384
MAX_ISSUES = 1_024
OUTCOMES = frozenset({"authored", "unrun", "reported_pass", "reported_fail",
                      "unknown", "unsupported", "pending", "partial"})
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
LABEL = re.compile(r"[A-Za-z0-9_.-]{1,96}\Z")


def _report() -> dict[str, Any]:
    return {"schema_version": OUTPUT_SCHEMA, "status": "needs_evidence",
            "formal_ready": False, "independent_execution_checked": False,
            "semantic_validation": "unobserved", "bilingual_alignment": "structured_fields_only",
            "atomic_snapshot": False, "source_session_digest": None,
            "sources": [], "observations": [], "claims": [], "issues": [], "issues_truncated": False,
            "claim_boundary": "Declared-record and source-byte consistency only; caller-reported "
            "observations remain unauthenticated; no semantic, execution, proof, authority or closure certification."}


def _issue(report, code, target=None):
    if len(report["issues"]) >= MAX_ISSUES:
        report["issues_truncated"] = True
        return
    report["issues"].append({"code": code, "target": target})


def _fields(value, names):
    return isinstance(value, dict) and set(value) == set(names)


def _label(value):
    return isinstance(value, str) and LABEL.fullmatch(value) is not None


def _digest(value):
    return isinstance(value, str) and SHA256.fullmatch(value) is not None


def _array(value, nonempty=True):
    return isinstance(value, list) and (bool(value) or not nonempty) and len(value) <= MAX_ITEMS


def _strict_json(data):
    text = data.decode("utf-8", errors="strict")
    if snapshot._json_exceeds_nesting_limit(text):
        raise snapshot._CheckError("json_nesting_limit")
    def reject_constant(_value):
        raise ValueError("nonfinite JSON constant")
    return json.loads(text, object_pairs_hook=snapshot._reject_duplicate_keys,
                      parse_constant=reject_constant)


def _read_source(root, parts, lease, budget):
    if budget.remaining <= 0:
        raise snapshot._CheckError("source_read_budget")
    opened = snapshot._open_file_beneath(root, parts, root_lease=lease)
    try:
        before = os.fstat(opened.fd)
        limit = min(MAX_SOURCE_BYTES, budget.remaining)
        if before.st_size < 0 or before.st_size > limit:
            raise snapshot._CheckError("source_read_budget")
        chunks, count = [], 0
        while count <= limit:
            chunk = os.read(opened.fd, min(65_536, limit + 1 - count))
            if not chunk:
                break
            # Failed/partial reads consume the same aggregate byte budget.
            budget.charge(len(chunk))
            chunks.append(chunk)
            count += len(chunk)
        if count > limit:
            raise snapshot._CheckError("source_read_budget")
        after = os.fstat(opened.fd)
        identity = lambda st: (st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns, st.st_ctime_ns)
        if identity(before) != identity(after) or count != after.st_size:
            raise snapshot._CheckError("source_changed_during_read")
        return b"".join(chunks)
    finally:
        opened.close()


def _source_entries(record, report):
    entries, seen = [], set()
    for source in record["sources"]:
        if not _fields(source, ("path", "sha256")) or not _digest(source.get("sha256")):
            _issue(report, "invalid_source_entry")
            continue
        parts = snapshot._validate_relative_path(source["path"])
        if parts is None:
            _issue(report, "invalid_source_path")
            continue
        key = source["path"].casefold()
        if key in seen:
            _issue(report, "duplicate_source_path", source["path"])
            continue
        seen.add(key)
        entries.append({**source, "parts": parts})
    digest = hashlib.sha256()
    for source in sorted(entries, key=lambda item: item["path"]):
        digest.update((source["path"] + "\t" + source["sha256"] + "\n").encode())
    report["source_session_digest"] = digest.hexdigest()
    if not _digest(record["source_session_digest"]) or digest.hexdigest() != record["source_session_digest"]:
        _issue(report, "source_session_digest_mismatch")
    return entries


def _sources(root, entries, report):
    """One root lease; file observations are sequential, never atomic."""
    current, lease = {}, None
    budget = snapshot._ReadBudget(MAX_TOTAL_SOURCE_BYTES)
    try:
        if os.name != "nt":
            lease = snapshot._RootLease(root, posix_fd=snapshot._open_posix_directory(root))
        elif entries:
            pin = snapshot._open_file_beneath(root, entries[0]["parts"])
            lease = snapshot._RootLease(root, windows_pin=pin)
        for source in entries:
            state = {"path": source["path"], "recorded_sha256": source["sha256"],
                     "current_sha256": None, "state": "unsupported"}
            try:
                data = _read_source(root, source["parts"], lease, budget)
                state["current_sha256"] = hashlib.sha256(data).hexdigest()
                if state["current_sha256"] != source["sha256"]:
                    state["state"] = "stale"
                    _issue(report, "source_sha256_mismatch", source["path"])
                else:
                    current[source["path"]] = data.decode("utf-8", errors="strict").splitlines()
                    state["state"] = "matches_declared_bytes"
            except (snapshot._CheckError, OSError, UnicodeError) as exc:
                _issue(report, getattr(exc, "code", "source_unreadable_or_non_utf8"), source["path"])
            report["sources"].append(state)
        if lease is not None:
            lease.verify_current()
    except (snapshot._CheckError, OSError) as exc:
        _issue(report, getattr(exc, "code", "root_unavailable"))
        current.clear()
        recorded = {item["path"] for item in report["sources"]}
        for source in entries:
            if source["path"] not in recorded:
                report["sources"].append({"path": source["path"],
                    "recorded_sha256": source["sha256"], "current_sha256": None,
                    "state": "unsupported"})
        # An unavailable root pin never leaves claim bindings marked current.
    finally:
        if lease is not None:
            lease.close()
    report["source_bytes_read"] = budget.consumed
    return current


def _observations(record, source_paths, report):
    indexed = {}
    for observation in record["observations"]:
        fields = ("id", "source_paths", "kind", "outcome", "basis", "command")
        if not _fields(observation, fields):
            _issue(report, "invalid_observation")
            continue
        name = observation["id"]
        valid = (_label(name) and observation["basis"] == "caller_reported"
                 and isinstance(observation["kind"], str) and observation["kind"] in {"authoring", "execution"}
                 and isinstance(observation["outcome"], str) and observation["outcome"] in OUTCOMES
                 and _array(observation["source_paths"]) and _array(observation["command"], False))
        if valid:
            valid = (all(isinstance(p, str) and p in source_paths for p in observation["source_paths"])
                     and len(set(observation["source_paths"])) == len(observation["source_paths"])
                     and all(isinstance(c, str) and 0 < len(c) <= 512 for c in observation["command"]))
        if valid and observation["kind"] == "execution":
            valid = bool(observation["command"]) and observation["outcome"] != "authored"
        if valid and observation["kind"] == "authoring":
            valid = observation["outcome"] in {"authored", "unrun", "unknown", "pending"} and not observation["command"]
        if not valid:
            _issue(report, "invalid_observation", name if _label(name) else None)
            continue
        if name in indexed:
            _issue(report, "duplicate_observation_id", name)
            continue
        indexed[name] = observation
        report["observations"].append({**observation, "independent_execution_checked": False})
    return indexed


def _variant(variant, current, observations, report, claim_id):
    fields = ("source_path", "start_line", "end_line", "text", "status", "evidence_ids")
    if not _fields(variant, fields):
        _issue(report, "invalid_claim_variant", claim_id)
        return False
    start, end = variant["start_line"], variant["end_line"]
    valid = (type(start) is int and type(end) is int and 1 <= start <= end
             and isinstance(variant["source_path"], str)
             and isinstance(variant["text"], str) and 0 < len(variant["text"]) <= MAX_TEXT_CHARS
             and isinstance(variant["status"], str) and variant["status"] in OUTCOMES
             and _array(variant["evidence_ids"]) and all(_label(i) for i in variant["evidence_ids"]))
    if not valid:
        _issue(report, "invalid_claim_variant", claim_id)
        return False
    evidence = variant["evidence_ids"]
    if len(set(evidence)) != len(evidence):
        _issue(report, "duplicate_claim_evidence", claim_id)
    for name in evidence:
        observation = observations.get(name)
        if observation is None:
            _issue(report, "unknown_observation_reference", claim_id)
        elif observation["outcome"] != variant["status"]:
            _issue(report, "claim_outcome_mismatch", claim_id)
        elif variant["source_path"] not in observation["source_paths"]:
            _issue(report, "claim_observation_source_mismatch", claim_id)
    lines = current.get(variant["source_path"])
    if lines is None:
        _issue(report, "claim_source_binding_unavailable", claim_id)
        return False
    if end > len(lines) or "\n".join(lines[start - 1:end]) != variant["text"]:
        _issue(report, "claim_text_mismatch", claim_id)
        return False
    return True


def _claims(record, current, observations, report):
    seen = set()
    for claim in record["claims"]:
        if not _fields(claim, ("id", "ja", "en")) or not _label(claim.get("id")):
            _issue(report, "invalid_claim")
            continue
        name = claim["id"]
        if name in seen:
            _issue(report, "duplicate_claim_id", name)
            continue
        seen.add(name)
        ja_ok = _variant(claim["ja"], current, observations, report, name)
        en_ok = _variant(claim["en"], current, observations, report, name)
        if isinstance(claim["ja"], dict) and isinstance(claim["en"], dict):
            left, right = claim["ja"], claim["en"]
            if left.get("status") != right.get("status") or left.get("evidence_ids") != right.get("evidence_ids"):
                _issue(report, "bilingual_status_or_evidence_mismatch", name)
        status = claim["ja"].get("status") if isinstance(claim["ja"], dict) else None
        report["claims"].append({"id": name, "source_binding_current": ja_ok and en_ok,
                                 "reported_status": status if isinstance(status, str) and status in OUTCOMES else None,
                                 "semantic_equivalence_checked": False})


def check_claim_bundle(bundle: bytes, repository_root: str | os.PathLike[str]) -> dict[str, Any]:
    """Check bounded caller records against current bytes; never execute commands."""
    report = _report()
    if not isinstance(bundle, bytes) or len(bundle) > MAX_BUNDLE_BYTES:
        _issue(report, "bundle_byte_limit")
        return report
    try:
        record = _strict_json(bundle)
    except snapshot._CheckError as exc:
        _issue(report, exc.code)
        return report
    except (ValueError, UnicodeError, RecursionError):
        _issue(report, "invalid_json")
        return report
    fields = ("schema_version", "source_session_digest", "formal_ready", "sources", "observations", "claims")
    if not _fields(record, fields) or record["schema_version"] != INPUT_SCHEMA:
        _issue(report, "invalid_bundle_fields")
        return report
    if record["formal_ready"] is not False:
        _issue(report, "formal_ready_promotion_forbidden")
    for key in ("sources", "observations", "claims"):
        if not _array(record[key]):
            _issue(report, "empty_or_oversize_" + key)
    if any(not _array(record[key]) for key in ("sources", "observations", "claims")):
        return report
    entries = _source_entries(record, report)
    try:
        root = Path(repository_root).absolute()
        current = _sources(root, entries, report)
    except (OSError, ValueError, TypeError, RuntimeError):
        _issue(report, "root_unavailable")
        current = {}
    observations = _observations(record, {source["path"] for source in entries}, report)
    _claims(record, current, observations, report)
    if not report["issues"]:
        report["status"] = "consistent_with_declared_records"
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--bundle", required=True, help="repository-relative bounded input file")
    args = parser.parse_args(argv)
    parts = snapshot._validate_relative_path(args.bundle)
    if parts is None:
        report = _report()
        _issue(report, "invalid_bundle_path")
    else:
        try:
            data, opened = snapshot._read_manifest(Path(args.repo).absolute(), parts)
            opened.close()
            report = check_claim_bundle(data, args.repo)
        except (snapshot._CheckError, OSError, ValueError) as exc:
            report = _report()
            _issue(report, getattr(exc, "code", "bundle_unavailable"))
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0 if report["status"] == "consistent_with_declared_records" else 2


if __name__ == "__main__":
    sys.exit(main())
