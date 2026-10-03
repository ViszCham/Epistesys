"""Recompute public GB-CC75 point estimates; no model calls or new inference.

集計整合性だけを検査する。採点正確性・研究効果の証明ではない。
Checks aggregate consistency only, not grading truth or research-effect proof.
"""

import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import re
import statistics
import sys


DEFAULT_BUNDLE = Path(__file__).resolve().parents[2] / "benchmarks/gb-cc75/2026-08-18"
FILES = {
    "GB_CC75_FINAL_SUMMARY.json", "F3-qid-primary.csv", "F3-paired-secondary.csv",
    "F3-statistics-report.json", "F1-missingness-incident-audit.json",
    "F2-audit-selection-r3.json", "F2-scoring-accuracy-audit-r2.json",
    "F4-adjudication.csv", "F4-direct-collapse.csv", "F4-incident.csv",
    "F4-missingness-bounds.csv", "F4-primary-source-reference.json",
}
PAIR_COLUMNS = {
    "qid", "epoch", "direct_passed", "labyrinth_passed", "criteria_total",
    "direct_fraction", "labyrinth_fraction", "delta", "direct_strict",
    "labyrinth_strict",
}
QID_COLUMNS = {
    "qid", "eligible_pair_count", "direct_mean", "labyrinth_mean", "delta", "outcome",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def near(actual, expected, label):
    actual, expected = float(actual), float(expected)
    require(math.isfinite(actual) and math.isfinite(expected), f"nonfinite {label}")
    require(math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12),
            f"inconsistent {label}: {actual} != {expected}")


def integer(value, label):
    require(re.fullmatch(r"[0-9]+", str(value)) is not None, f"invalid integer {label}")
    return int(value)


def fraction(value, label):
    number = float(value)
    require(math.isfinite(number) and 0 <= number <= 1, f"invalid fraction {label}")
    return number


def qid(value):
    require(re.fullmatch(r"CIF-[0-9]{3}", value) is not None, "invalid QID")
    require(1 <= int(value[4:]) <= 75, "out-of-range QID")
    return value


def read_json(root, name):
    with (root / name).open(encoding="utf-8-sig") as stream:
        return json.load(stream)


def read_csv(root, name, columns):
    with (root / name).open(encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        require(set(reader.fieldnames or []) == columns, f"unexpected columns: {name}")
        rows = list(reader)
        require(all(set(row) == columns and all(v is not None for v in row.values())
                    for row in rows), f"malformed row: {name}")
        return rows


def verify_hashes(root, manifest=None):
    manifest = manifest or read_json(root, "artifact-manifest.json")
    require(manifest["schema_version"] == "epistesys-gb-cc75-public-artifact-manifest.v1",
            "manifest schema mismatch")
    entries = manifest["files"]
    require(len(entries) == len(FILES), "manifest count mismatch")
    require({e["path"] for e in entries} == FILES, "manifest selection/path mismatch")
    for entry in entries:
        target = root / entry["path"]
        require(target.is_file() and not target.is_symlink(), "missing/symlink artifact")
        raw = target.read_bytes()
        require(len(raw) == entry["bytes"], f"byte count mismatch: {entry['path']}")
        digest = hashlib.sha256(raw).hexdigest()
        require(digest == entry["sha256"], f"hash mismatch: {entry['path']}")
        require(re.fullmatch(r"[0-9a-f]{64}", entry["source_sha256"]) is not None,
                "invalid source digest")
    return len(entries)


def load_bundle(root):
    return {
        "pairs": read_csv(root, "F3-paired-secondary.csv", PAIR_COLUMNS),
        "qids": read_csv(root, "F3-qid-primary.csv", QID_COLUMNS),
        "stats": read_json(root, "F3-statistics-report.json"),
        "summary": read_json(root, "GB_CC75_FINAL_SUMMARY.json"),
        "missingness": read_json(root, "F1-missingness-incident-audit.json"),
        "audit": read_json(root, "F2-scoring-accuracy-audit-r2.json"),
        "manifest": read_json(root, "artifact-manifest.json"),
    }


def compare_tree(actual, expected, label):
    if isinstance(expected, dict):
        require(isinstance(actual, dict) and actual.keys() == expected.keys(),
                f"keys differ: {label}")
        for key in expected:
            compare_tree(actual[key], expected[key], f"{label}.{key}")
    elif isinstance(expected, (int, float)) and not isinstance(expected, bool):
        near(actual, expected, label)
    else:
        require(actual == expected, f"values differ: {label}")


def outcome(delta):
    return "tie" if abs(delta) < 1e-12 else (
        "labyrinth_win" if delta > 0 else "labyrinth_loss"
    )


def validate_aggregates(data):
    pairs, qids, stats = data["pairs"], data["qids"], data["stats"]
    summary, missingness = data["summary"], data["missingness"]
    require(len(pairs) == 167 and len(qids) == 65, "historical population mismatch")
    compare_tree(summary["automated_result"], stats, "summary.statistics")
    grouped, identities, counts = {}, set(), {"direct": 0, "labyrinth": 0, "total": 0}
    strict = {"direct": 0, "labyrinth": 0}
    for row in pairs:
        key = (qid(row["qid"]), integer(row["epoch"], "epoch"))
        require(1 <= key[1] <= 3 and key not in identities, "duplicate/invalid pair")
        identities.add(key)
        total = integer(row["criteria_total"], "criteria_total")
        require(10 <= total <= 40, "invalid criterion count")
        scores = {}
        for route in ("direct", "labyrinth"):
            passed = integer(row[f"{route}_passed"], f"{route}_passed")
            require(passed <= total, "pass count out of range")
            scores[route] = fraction(row[f"{route}_fraction"], route)
            near(scores[route], passed / total, f"pair.{route}.fraction")
            flag = row[f"{route}_strict"]
            require(flag in {"true", "false"}, "invalid strict encoding")
            require((flag == "true") == (passed == total), "forged strict flag")
            counts[route] += passed
            strict[route] += flag == "true"
        delta = scores["labyrinth"] - scores["direct"]
        near(row["delta"], delta, "pair.delta")
        grouped.setdefault(key[0], []).append(scores)
        counts["total"] += total
    require(len(grouped) == 65, "pair cluster count mismatch")
    qid_seen, qid_scores, qid_outcomes = set(), [], []
    for row in qids:
        key = qid(row["qid"])
        require(key not in qid_seen and key in grouped, "duplicate/unknown primary QID")
        qid_seen.add(key)
        rows = grouped[key]
        require(integer(row["eligible_pair_count"], "eligible_pair_count") == len(rows),
                "QID pair count mismatch")
        means = {r: statistics.mean(s[r] for s in rows) for r in ("direct", "labyrinth")}
        for route, mean in means.items():
            near(fraction(row[f"{route}_mean"], route), mean, f"qid.{route}.mean")
        delta = means["labyrinth"] - means["direct"]
        near(row["delta"], delta, "qid.delta")
        require(row["outcome"] == outcome(delta), "QID outcome mismatch")
        qid_scores.append(means)
        qid_outcomes.append(outcome(delta))
    require(qid_seen == set(grouped), "QID sets differ")
    for label, rows in (("primary", qid_scores),
                        ("secondary", [s for group in grouped.values() for s in group])):
        report = stats[label]
        require(report["count"] == len(rows) and report["cluster_count"] == 65,
                f"count mismatch: {label}")
        means = {r: statistics.mean(s[r] for s in rows) for r in ("direct", "labyrinth")}
        delta = means["labyrinth"] - means["direct"]
        for route, mean in means.items():
            near(report[f"{route}_mean"], mean, f"{label}.{route}")
        near(report["delta"], delta, f"{label}.delta")
        near(report["delta_percentage_points"], 100 * delta, f"{label}.points")
        near(report["relative_uplift"], delta / means["direct"], f"{label}.relative")
        outcomes = [outcome(s["labyrinth"] - s["direct"]) for s in rows]
        for field, value in (("wins", "labyrinth_win"), ("ties", "tie"),
                             ("losses", "labyrinth_loss")):
            require(report[field] == outcomes.count(value), f"outcome mismatch: {label}")
    descriptive = stats["criterion_weighted_descriptive"]
    require(counts["total"] == descriptive["criteria_total_per_route"] == 3521,
            "criterion instance count mismatch")
    require(descriptive["pair_count"] == len(pairs), "strict denominator mismatch")
    for route in ("direct", "labyrinth"):
        require(counts[route] == descriptive[f"{route}_passed"], "pass sum mismatch")
        require(strict[route] == descriptive[f"{route}_strict_passes"], "strict sum mismatch")
        near(descriptive[f"{route}_rate"], counts[route] / counts["total"], "weighted rate")
        near(descriptive[f"{route}_strict_rate"], strict[route] / len(pairs), "strict rate")
    delta = (counts["labyrinth"] - counts["direct"]) / counts["total"]
    near(descriptive["delta"], delta, "weighted delta")
    near(descriptive["delta_percentage_points"], delta * 100, "weighted points")
    near(descriptive["strict_delta_percentage_points"],
         (strict["labyrinth"] - strict["direct"]) / len(pairs) * 100, "strict points")
    require(missingness["eligible_pair_count"] == 171 and
            missingness["successful_pair_count"] == 167 and
            missingness["lost_pair_count"] == len(missingness["lost_pairs"]) == 4 and
            missingness["technical_unscored_count"] == len(missingness["technical_unscored"]) == 10 and
            missingness["qids_with_eligible_pairs"] == 67 and
            missingness["qids_complete_for_primary"] == 65, "missingness mismatch")
    for label, observed, planned in (("primary", 65, 67), ("secondary", 167, 171)):
        bound = stats[f"{label}_missingness_bounds"]
        require((bound["observed_units"], bound["planned_units"], bound["missing_units"]) ==
                (observed, planned, planned - observed), "bound denominator mismatch")
        center = stats[label]["delta"] * observed
        near(bound["lower_delta"], (center - (planned - observed)) / planned, "lower bound")
        near(bound["upper_delta"], (center + (planned - observed)) / planned, "upper bound")
    audit = data["audit"]
    require(audit["selected_criterion_judgments"] == audit["automated_audit_agreements"] == 56
            and audit["automated_audit_disagreements"] == 0, "audit count mismatch")
    require(summary["adjudicated_result"]["independent_human_adjudication"] is False,
            "independent-human status altered")
    return {
        "primary_qids": len(qids), "scored_pairs": len(pairs),
        "primary_difference_points": stats["primary"]["delta_percentage_points"],
        "strict_difference_points": descriptive["strict_delta_percentage_points"],
        "labyrinth_strict_passes": strict["labyrinth"],
        "negative_qids": sorted(row["qid"] for row in qids
                                if row["outcome"] == "labyrinth_loss"),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, default=DEFAULT_BUNDLE)
    args = parser.parse_args()
    try:
        checked = verify_hashes(args.bundle)
        results = validate_aggregates(load_bundle(args.bundle))
    except (ValueError, KeyError, TypeError, OSError) as error:
        print(f"GB-CC75 aggregate check failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"status": "aggregate_consistency_checked", "hashed_files": checked,
                      **results, "new_model_calls": 0, "uncertainty_regenerated": False,
                      "boundary": "not grading accuracy, causal inference, or alpha.2 performance"},
                     indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
