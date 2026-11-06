"""Fail-closed tests for the append-only TLCX design revision chain."""

from __future__ import annotations

import copy
import json
import subprocess
import unittest
from decimal import Decimal, localcontext
from fractions import Fraction
from pathlib import Path
from unittest.mock import patch

from tlcx_design_revision import validate_checkout_eol, validate_revision_chain


ROOT = Path(__file__).resolve().parents[2]
CHAIN_PATH = Path("validation/tlcx/design-revision-chain-2026-10-29.json")
BASELINE_PATH = Path("validation/tlcx/baseline-registration-2026-10-28.json")
MEASUREMENT_PATH = Path("docs/tlcx-tl-screative-measurement-contract-v1-2026-10-29.md")
MEASUREMENT_REVISION = "TLCX-MEASUREMENT-2026-10-29-FINITE-FORMAL-V"


def read_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def measurement(chain: dict, version: str) -> dict:
    revision_id = MEASUREMENT_REVISION + version
    return next(item for item in chain["history"] if item["revision_id"] == revision_id)


def point_to(revision: dict, predecessor: dict) -> None:
    revision["previous_measurement_revision_id"] = predecessor["revision_id"]
    revision["previous_measurement_revision_sha256"] = predecessor["sha256"]


class TlcxDesignRevisionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.baseline = read_json(ROOT / BASELINE_PATH)

    def test_current_design_revision_chain_is_complete_and_bound(self) -> None:
        self.assertEqual(validate_revision_chain(ROOT, baseline=self.baseline), [])

    def test_measurement_lineage_cannot_delete_v12_and_relink_v13(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        v12 = measurement(chain, "1.2")
        chain["history"].remove(v12)
        point_to(measurement(chain, "1.3"), measurement(chain, "1.1"))
        self.assertIn(
            f"registered_measurement_missing:{v12['revision_id']}",
            validate_revision_chain(ROOT, chain=chain, baseline=self.baseline),
        )

    def test_measurement_lineage_cannot_replace_v12_digest_and_relink_v13(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        v12 = measurement(chain, "1.2")
        v12["sha256"] = "5" * 64
        point_to(measurement(chain, "1.3"), v12)
        self.assertIn(
            f"registered_measurement_digest_mismatch:{v12['revision_id']}",
            validate_revision_chain(ROOT, chain=chain, baseline=self.baseline),
        )

    def test_measurement_lineage_requires_immediate_measurement_predecessor(self) -> None:
        for predecessor_id in (
            MEASUREMENT_REVISION + "1.1",
            "TLCX-ROADMAP-2026-10-29-ADDENDUM",
        ):
            with self.subTest(predecessor=predecessor_id):
                chain = read_json(ROOT / CHAIN_PATH)
                v13 = measurement(chain, "1.3")
                predecessor = next(
                    x for x in chain["history"] if x["revision_id"] == predecessor_id
                )
                point_to(v13, predecessor)
                self.assertIn(
                    f"measurement_predecessor_not_immediate:{v13['revision_id']}",
                    validate_revision_chain(ROOT, chain=chain, baseline=self.baseline),
                )

    def test_measurement_lineage_rejects_reordering_even_after_relinking(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        v11, v12, v13 = (measurement(chain, version) for version in ("1.1", "1.2", "1.3"))
        history = chain["history"]
        first, second = history.index(v11), history.index(v12)
        history[first], history[second] = v12, v11
        del v12["previous_measurement_revision_id"]
        del v12["previous_measurement_revision_sha256"]
        point_to(v11, v12)
        point_to(v13, v11)
        self.assertIn(
            "registered_measurement_order_invalid",
            validate_revision_chain(ROOT, chain=chain, baseline=self.baseline),
        )

    def test_measurement_lineage_rejects_duplicate_registered_revision(self) -> None:
        chain = read_json(ROOT / CHAIN_PATH)
        entry = measurement(chain, "1.2")
        chain["history"].insert(chain["history"].index(entry), entry.copy())
        self.assertIn("duplicate_revision_id", validate_revision_chain(ROOT, chain, self.baseline))

    def test_measurement_lineage_malformed_json_returns_stable_errors(self) -> None:
        with self.subTest(shape="top-level array"):
            self.assertIn("chain_not_object", validate_revision_chain(ROOT, [], self.baseline))
        for history, error in (
            (None, "history_not_list"),
            (["not a revision object"], "history_entry_not_object:0"),
            ([{"revision_id": []}], "history_revision_id_invalid:0"),
        ):
            with self.subTest(history=history):
                chain = read_json(ROOT / CHAIN_PATH)
                chain["history"] = history
                self.assertIn(error, validate_revision_chain(ROOT, chain, self.baseline))
        with self.subTest(shape="non-string predecessor ID"):
            chain = read_json(ROOT / CHAIN_PATH)
            v13 = measurement(chain, "1.3")
            v13["previous_measurement_revision_id"] = []
            self.assertIn(
                f"measurement_predecessor_missing:{v13['revision_id']}",
                validate_revision_chain(ROOT, chain, self.baseline),
            )

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


class TlcxCheckoutEolTests(unittest.TestCase):
    def _validate_with_fake_git(
        self,
        paths: list[str],
        *,
        attributes: dict[str, str] | None = None,
        contents: dict[str, bytes | OSError] | None = None,
        raw_output: bytes | None = None,
        failure: Exception | None = None,
    ) -> list[str]:
        attributes = attributes or {}
        contents = contents or {}

        def fake_run(args: list[str], **kwargs: object) -> subprocess.CompletedProcess:
            # Model the Windows limit, without launching Git or requiring Windows.
            command_units = len(subprocess.list2cmdline(args).encode("utf-16-le")) // 2 + 1
            if command_units > 32_767:
                raise OSError(206, "modeled Windows command-line limit exceeded")
            if failure is not None:
                raise failure
            if "--stdin" in args:
                payload = kwargs.get("input")
                self.assertIsInstance(payload, bytes)
                self.assertIn("-z", args)
                self.assertFalse(kwargs.get("text", False))
                fields = payload.split(b"\0")
                self.assertEqual(fields.pop(), b"")
                requested = [field.decode("utf-8") for field in fields]
            else:
                requested = args[args.index("--") + 1 :]
            self.assertEqual(requested, paths)
            if raw_output is not None:
                stdout = raw_output
            elif "-z" in args:
                stdout = b"".join(
                    path.encode("utf-8") + b"\0eol\0"
                    + attributes.get(path, "lf").encode("utf-8") + b"\0"
                    for path in requested
                )
            else:
                stdout = "".join(
                    f"{path}: eol: {attributes.get(path, 'lf')}\n" for path in requested
                ).encode("utf-8")
            if kwargs.get("text", False):
                stdout = stdout.decode("utf-8", errors="replace")
            return subprocess.CompletedProcess(args, 0, stdout=stdout)

        def fake_read_bytes(path: Path) -> bytes:
            content = contents.get(path.relative_to(ROOT.resolve()).as_posix(), b"LF\n")
            if isinstance(content, OSError):
                raise content
            return content

        with (
            patch("tlcx_design_revision.PINNED_LF_FILES", tuple(paths)),
            patch("tlcx_design_revision.Path.glob", return_value=[]),
            patch("tlcx_design_revision.Path.read_bytes", autospec=True, side_effect=fake_read_bytes),
            patch("tlcx_design_revision.subprocess.run", side_effect=fake_run) as launch,
        ):
            errors = validate_checkout_eol(ROOT)
        launch.assert_called_once()
        return errors

    def test_checkout_eol_streams_many_paths_without_windows_argv_overflow(self) -> None:
        paths = [
            f"validation/tlcx/登録 文書 {index:04d} {'x' * 48}.json"
            for index in range(600)
        ]
        legacy_command = subprocess.list2cmdline(["git", "check-attr", "eol", "--", *paths])
        self.assertGreater(len(legacy_command.encode("utf-16-le")) // 2 + 1, 32_767)
        self.assertEqual(self._validate_with_fake_git(paths), [])

    def test_checkout_eol_roundtrips_unicode_and_delimiter_paths(self) -> None:
        # These are mocked protocol paths, including characters Windows files cannot use.
        paths = ["docs/日本語 space é.md", "docs/colon : tab\tline\nname.md"]
        self.assertEqual(self._validate_with_fake_git(paths), [])

    def test_checkout_eol_preserves_policy_and_checkout_diagnostics(self) -> None:
        crlf_path = "docs/日本語 space.md"
        unreadable_path = "docs/unreadable.md"
        self.assertEqual(
            self._validate_with_fake_git(
                ["docs/valid.md", crlf_path, unreadable_path],
                attributes={crlf_path: "crlf"},
                contents={crlf_path: b"CRLF\r\n", unreadable_path: OSError("unreadable")},
            ),
            sorted([
                f"pinned_eol_policy_missing:{crlf_path}",
                f"checkout_crlf_detected:{crlf_path}",
                f"pinned_file_unreadable:{unreadable_path}",
                "git_eol_attribute_result_mismatch",
            ]),
        )

    def test_checkout_eol_preserves_unavailable_diagnostic(self) -> None:
        for failure in (OSError("missing Git"), subprocess.CalledProcessError(1, "git")):
            with self.subTest(failure=type(failure).__name__):
                self.assertEqual(
                    self._validate_with_fake_git(["docs/pinned.md"], failure=failure),
                    ["git_eol_attribute_check_unavailable"],
                )

    def test_checkout_eol_rejects_malformed_or_unrequested_nul_output(self) -> None:
        good = b"docs/pinned.md\0eol\0lf\0"
        for output in (
            good[:-1],
            good + b"unfinished\0",
            good + b"\0",
            good + b"docs/unrequested.md\0eol\0lf\0",
            b"docs/pinned.md\0text\0lf\0",
        ):
            with self.subTest(output=output):
                self.assertIn(
                    "git_eol_attribute_result_mismatch",
                    self._validate_with_fake_git(["docs/pinned.md"], raw_output=output),
                )

    def test_checkout_eol_retains_set_semantics_for_duplicate_rows(self) -> None:
        self.assertEqual(
            self._validate_with_fake_git(["docs/pinned.md", "docs/pinned.md"]), []
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
