"""Fail-closed tests for the append-only TLCX design revision chain."""

from __future__ import annotations

import copy
import json
import unittest
from decimal import Decimal, localcontext
from fractions import Fraction
from pathlib import Path

from tlcx_design_revision import validate_checkout_eol, validate_revision_chain


ROOT = Path(__file__).resolve().parents[2]
CHAIN_PATH = Path("validation/tlcx/design-revision-chain-2026-10-29.json")
BASELINE_PATH = Path("validation/tlcx/baseline-registration-2026-10-28.json")
MEASUREMENT_PATH = Path("docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md")


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


class TlcxDesignRevisionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.baseline = read_json(ROOT / BASELINE_PATH)

    def test_current_design_revision_chain_is_complete_and_bound(self) -> None:
        self.assertEqual(validate_revision_chain(ROOT, baseline=self.baseline), [])

    def test_byte_hashed_documents_have_platform_stable_lf_checkout(self) -> None:
        self.assertEqual(validate_checkout_eol(ROOT), [])

    def test_k8_distance_identity_is_explicit_and_bilingual(self) -> None:
        contract = (ROOT / MEASUREMENT_PATH).read_text(encoding="utf-8")
        japanese, english = contract.split("## English", maxsplit=1)
        required = (
            "t=κ_curv*||x-y||_2^2/((1-κ_curv*||x||_2^2)(1-κ_curv*||y||_2^2))",
            "acosh(1+2*t)=2*atanh(sqrt(t/(1+t)))",
            "2*atanh(q_i)/sqrt(κ_curv)",
        )
        for label, section in (("Japanese", japanese), ("English", english)):
            with self.subTest(language=label):
                for expression in required:
                    self.assertIn(expression, section)
                self.assertNotIn("q/sqrtκのendpoint-specific distance", section)

    def test_v14_math_audit_reconciliation_has_bilingual_regressions_and_history(self) -> None:
        contract = (ROOT / MEASUREMENT_PATH).read_text(encoding="utf-8")
        japanese, english = contract.split("## English", maxsplit=1)
        for label, section in (("Japanese", japanese), ("English", english)):
            with self.subTest(language=label):
                self.assertIn("finite-formal-v1.4", section)
                self.assertIn(
                    "[1−upperExp(dL/s_geo),1−lowerExp(dU/s_geo)]", section
                )
                self.assertIn("square([a,b])", section)
                self.assertIn("max(a²,b²)", section)
                self.assertIn("min(a²,b²)", section)
                self.assertIn("DistanceToFrontierUnion-v1", section)
                self.assertIn("NearestVerifiedCandidate-v1", section)
                for number in range(31, 38):
                    self.assertIn(f"G{number}", section)
                self.assertNotIn(
                    "[1−upperExp(dU/s_geo),1−lowerExp(dL/s_geo)]", section
                )

        chain = read_json(ROOT / CHAIN_PATH)
        v14 = next(
            item
            for item in chain["history"]
            if item.get("revision_id") == "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.4"
        )
        self.assertEqual(
            v14["previous_measurement_revision_id"],
            "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.3",
        )
        self.assertEqual(
            v14["math_reaudit_source_sha256"],
            "c4a6021dfd59303bec73f45f3d50712c33a603781e43f08526605e1c5d76f28b",
        )

    def test_v14_audit_counterexamples_recompute_without_production_geometry(self) -> None:
        # Contract-vector arithmetic only; this does not run or validate an EmbeddingPolicy.
        t = Fraction(1, 2) / (1 - Fraction(1, 4)) ** 2
        q_squared = t / (1 + t)
        self.assertEqual(t, Fraction(8, 9))
        self.assertEqual(q_squared, Fraction(8, 17))

        q_cell = Fraction(1, 2**80)
        q_upper_squared = q_squared + 2 * q_cell + q_cell**2
        self.assertLess(q_upper_squared, Fraction(1, 2))
        endpoint_distance_width_bound = 4 * q_cell
        self.assertLess(endpoint_distance_width_bound, Fraction(1, 2**64))

        with localcontext() as context:
            context.prec = 60
            novelty_lower = Decimal(1) - Decimal(0).exp()
            novelty_upper = Decimal(1) - Decimal(-1).exp()
            self.assertEqual(novelty_lower, Decimal(0))
            self.assertLessEqual(novelty_lower, novelty_upper)
            d_lower = (Decimal(27) / Decimal(11)).ln()
            d_upper = Decimal(3).ln()
            self.assertLess(d_lower, d_upper)
            self.assertGreater(d_upper - d_lower, Decimal("0.2"))

        frontier = ({"a"}, {"b"})
        union = set().union(*frontier)
        proposals = ({"a"}, {"a", "b"})

        def jaccard_distance(left: set[str], right: set[str]) -> Fraction:
            return Fraction(len(left ^ right), len(left | right))

        union_distance = tuple(jaccard_distance(proposal, union) for proposal in proposals)
        nearest_distance = tuple(
            min(jaccard_distance(proposal, branch) for branch in frontier)
            for proposal in proposals
        )
        self.assertEqual(union_distance, (Fraction(1, 2), Fraction(0)))
        self.assertEqual(nearest_distance, (Fraction(0), Fraction(1, 2)))

        square_interval = (
            Fraction(0),
            max(Fraction(-1, 10) ** 2, Fraction(1, 10) ** 2),
        )
        self.assertEqual(square_interval, (Fraction(0), Fraction(1, 100)))

    @unittest.skipUnless((ROOT / CHAIN_PATH).is_file(), "revision-chain prerequisite")
    def test_changed_current_roadmap_digest_fails_closed(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        broken = copy.deepcopy(chain)
        broken["current_design"]["roadmap"]["sha256"] = "0" * 64
        self.assertIn(
            "current_roadmap_digest_mismatch",
            validate_revision_chain(ROOT, chain=broken, baseline=self.baseline),
        )

    @unittest.skipUnless((ROOT / CHAIN_PATH).is_file(), "revision-chain prerequisite")
    def test_original_baseline_hash_is_not_rewritten(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        baseline = copy.deepcopy(self.baseline)
        baseline["baseline"]["roadmap_sha256"] = "0" * 64
        self.assertIn(
            "original_baseline_registration_was_overwritten",
            validate_revision_chain(ROOT, chain=chain, baseline=baseline),
        )

    @unittest.skipUnless((ROOT / CHAIN_PATH).is_file(), "revision-chain prerequisite")
    def test_current_design_requires_the_exact_previous_measurement_revision(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        broken = copy.deepcopy(chain)
        broken["current_design"]["previous_revision_id"] = "unregistered-revision"
        self.assertIn(
            "current_previous_revision_id_mismatch",
            validate_revision_chain(ROOT, chain=broken, baseline=self.baseline),
        )

    @unittest.skipUnless((ROOT / CHAIN_PATH).is_file(), "revision-chain prerequisite")
    def test_each_measurement_revision_digest_matches_its_predecessor(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        broken = copy.deepcopy(chain)
        v12 = next(
            item
            for item in broken["history"]
            if item.get("revision_id")
            == "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.2"
        )
        v12["previous_measurement_revision_sha256"] = "0" * 64
        self.assertIn(
            "measurement_predecessor_digest_mismatch:TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V1.2",
            validate_revision_chain(ROOT, chain=broken, baseline=self.baseline),
        )

    @unittest.skipUnless((ROOT / CHAIN_PATH).is_file(), "revision-chain prerequisite")
    def test_source_identity_drift_fails_closed(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        broken = copy.deepcopy(chain)
        broken["source_identity"]["commit"] = "0" * 40
        self.assertIn(
            "source_commit_mismatch",
            validate_revision_chain(ROOT, chain=broken, baseline=self.baseline),
        )

    @unittest.skipUnless((ROOT / CHAIN_PATH).is_file(), "revision-chain prerequisite")
    def test_path_escape_fails_closed(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        broken = copy.deepcopy(chain)
        broken["current_design"]["roadmap"]["path"] = "../../outside.md"
        self.assertIn(
            "current_roadmap_path_invalid",
            validate_revision_chain(ROOT, chain=broken, baseline=self.baseline),
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
