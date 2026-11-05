//! Test-first conditional artifact quality, not source-level C1/verified31 success.
mod tlcx22_goal_artifact_support;
use tlcx22_goal_artifact_support::*;
use lc631_analysis::creativity_measurement::{ComponentMetric, InputBasis,
    SemanticValidation, IndependenceStatus};
use lc631_runtime::{goal_suite_measurement as g, joint_selection::{self, AxisKind}};
use lc631_substrate::resource::ResourceError;

#[test]
fn tlcx22_artifact_actual_outputs_hand_oracle_absolute_and_two_deltas_share_dependency() {
    fixture(|h, current, view| {
        assert_eq!(h.baseline.actual_outputs(), &BASE);
        assert_eq!(h.candidate.actual_outputs(), &HAND);
        assert_eq!((h.baseline.correct_probes(), h.candidate.correct_probes()), (3, 4));
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        owner.with_current(&root, h.pair(), current, view, |v| {
            // Literal expected values, never production parser/count/gain-derived gold.
            assert_eq!(v.baseline_outputs(), &BASE); assert_eq!(v.candidate_outputs(), &HAND);
            assert_eq!(v.baseline_successes(), &[true, true, false, true]);
            assert_eq!(v.candidate_successes(), &[true; 4]);
            assert_eq!(v.baseline_artifact_goal(), &ratio(3, 4));
            assert_eq!(v.candidate_artifact_goal(), &ratio(1, 1));
            for report in [v.coverage(), v.solve()] {
                assert_eq!(value(report), &ratio(1, 4));
                assert_eq!(report.basis, InputBasis::Given);
                assert_eq!(report.validation, SemanticValidation::Unassessed);
                assert_eq!(report.independence, IndependenceStatus::SharedDependency);
                assert_eq!(report.dependency_group, v.goal_dependency());
            }
            assert_eq!(v.coverage().metric, ComponentMetric::Coverage);
            assert_eq!(v.solve().metric, ComponentMetric::Solve);
            assert_ne!(v.coverage().definition, v.solve().definition);
            assert_eq!(v.requested_source_goal(), None);
            assert_eq!(v.source_correspondence(), g::SourceCorrespondence::UnverifiedNeedsEvidence);
            assert_eq!(v.source_eligibility(), g::SourceEligibility::Pending);
            assert_eq!(v.source_recommendation(), None);
            assert!(matches!(v.require_overlap(g::DeclaredOverlap::Absent), Err(g::Error::UndeclaredOverlap)));
            v.require_overlap(g::DeclaredOverlap::SameArtifactPrimitive).unwrap();
            // Real existing reported projector, called ONLY while actual owner view is current.
            assert_eq!(joint_selection::project_component_reported(v.coverage()).unwrap().kind, AxisKind::CoverageGain);
            assert_eq!(joint_selection::project_component_reported(v.solve()).unwrap().kind, AxisKind::SolveGain);
        }).unwrap();
    });
}

#[test]
fn tlcx22_artifact_baseline_origin_is_required_and_never_promotes_to_source_origin() {
    fixture(|h, current, view| {
        let mut input = h.root_input(); input.baseline_before = None;
        assert!(matches!(g::ArtifactPairRoot::given(input, view), Err(g::Error::MissingBaselineOrigin)));
        // Source-level C1 constructor requires actual existing validated origins,
        // not Option presence or conditional capture owners.
        let source_scope = lc631_core::context::ContextScope::declared(d(61), d(62), d(63), d(64));
        let source_plan = lc631_runtime::contextual_repair::measurement::CodingProbePlan::checked_add(
            source_scope.goal(), source_scope.semantics()).unwrap();
        assert!(matches!(g::SourceCodingRoot::given(g::SourceCodingRootInput {
            plan: &source_plan, pins: g::SourcePins { context: pins().context, scope: source_scope,
                definition: d(0xe1), oracle: d(0xe2), hard: d(65), evidence: d(66) },
            baseline_expected: None, candidate_expected: None,
            limits: limits(),
        }, view), Err(g::Error::MissingVerifiedOrigin)));
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        owner.with_current(&root, h.pair(), current, view, |v| {
            assert_eq!(v.requested_source_goal(), None);
            assert_eq!(v.source_eligibility(), g::SourceEligibility::Pending);
        }).unwrap();
    });
}

#[test]
fn tlcx22_artifact_root_query_definition_budget_and_owner_swap_are_not_scores() {
    fixture(|h, current, view| {
        let mut bad = h.root_input(); bad.pins.oracle = d(99);
        assert!(matches!(g::ArtifactPairRoot::given(bad, view), Err(g::Error::OracleMismatch)));
        let mut bad = h.root_input(); bad.limits.max_queries = 3;
        assert!(matches!(g::ArtifactPairRoot::given(bad, view), Err(g::Error::InvalidLimits)));
        let mut bad = h.root_input(); bad.pins.scope = lc631_core::context::ContextScope::declared(
            d(21), d(99), d(23), d(24));
        assert!(matches!(g::ArtifactPairRoot::given(bad, view), Err(g::Error::GoalOrProbeMismatch)));
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let foreign = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        assert!(matches!(owner.with_current(&foreign, h.pair(), current, view, |_| ()), Err(g::Error::BindingMismatch)));
        let swapped = g::CodingPair { baseline: h.candidate, candidate: h.baseline };
        assert!(matches!(owner.with_current(&root, swapped, current, view, |_| ()), Err(g::Error::RoleOrOriginMismatch)));
    });
}

#[test]
fn tlcx22_artifact_manifest_drift_with_identical_target_and_native_withholds_current_view() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        let native_before = native::file_hash(h.candidate_binary);
        let target = h.candidate_root.join("src/lib.rs"); let bytes = std::fs::read(&target).unwrap();
        let manifest = h.candidate_root.join("Cargo.toml");
        let old = std::fs::read(&manifest).unwrap(); std::fs::write(&manifest, [old.as_slice(), b"\n"].concat()).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), bytes);
        assert_eq!(native::file_hash(h.candidate_binary), native_before);
        assert!(matches!(owner.with_current(&root, h.pair(), current, view, |_| ()), Err(g::Error::BuildDrift)));
    });
}

#[test]
fn tlcx22_artifact_baseline_build_drift_and_native_swap_are_not_fresh_baselines() {
    for native_cut in [false, true] { fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        if native_cut {
            // Replace only the test-owned retained artifact, never a shared executable.
            std::fs::copy(h.candidate_binary, h.baseline_binary).unwrap();
        } else {
            std::fs::write(h.baseline_root.join("src/extra.rs"), b"pub const CHANGED:u8=1;\n").unwrap();
        }
        assert!(owner.with_current(&root, h.pair(), current, view, |_| ()).is_err());
    }); }
}

#[test]
fn tlcx22_artifact_cancel_and_current_test_revocation_cut_cached_reports() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        let revoked = vec![current.candidate_invocation.permit.digest().to_owned()];
        let old = current.candidate_invocation.authority;
        let authority = lc631_core::ExecutionPermitContext { revoked_permit_digests: &revoked,
            revocation_revision: old.revocation_revision + 1, principal: old.principal,
            action: old.action, scope: old.scope, source_revision: old.source_revision,
            authority_revision: old.authority_revision, now_epoch: old.now_epoch,
            caller_origin: old.caller_origin, trusted_host_fingerprint: old.trusted_host_fingerprint };
        let baseline = invocation(&current.baseline_invocation, current.baseline_invocation.authority);
        let candidate = invocation(&current.candidate_invocation, &authority);
        let mut cut = g::ArtifactCurrent { baseline_invocation: baseline, candidate_invocation: candidate,
            baseline_build: &mut *current.baseline_build, candidate_build: &mut *current.candidate_build,
            baseline_authority: &mut *current.baseline_authority,
            candidate_authority: &mut *current.candidate_authority };
        assert!(matches!(owner.with_current(&root, h.pair(), &mut cut, view, |_| ()), Err(g::Error::Unauthorized)));
    });
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        h.cancel.cancel();
        assert!(matches!(owner.with_current(&root, h.pair(), current, view, |_| ()),
            Err(g::Error::Resource(ResourceError::Cancelled))));
    });
}

#[test]
fn tlcx22_artifact_wrong_canonical_four_lines_is_failure_not_user_declared_success() {
    fixture_with_output(native::BEFORE, |h, current, view| {
        // All-none is well-formed but wrong on three cases. Baseline is still3/4.
        assert_eq!(h.candidate.actual_outputs(), &[None; 4]);
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let owner = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        owner.with_current(&root, h.pair(), current, view, |v| {
            assert_eq!(v.candidate_successes(), &[false, false, true, false]);
            assert_eq!(v.candidate_artifact_goal(), &ratio(1, 4));
            assert_eq!(value(v.solve()), &ratio(-1, 2));
            assert_eq!(value(v.coverage()), &ratio(-1, 2));
        }).unwrap();
    });
}

#[test]
fn tlcx22_artifact_equal_root_pins_do_not_mix_distinct_actual_baseline_occurrences() {
    fixture(|h, current, view| {
        let root = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let other = g::ArtifactPairRoot::given(h.root_input(), view).unwrap();
        let first = g::measure_artifact_pair(&root, h.pair(), current, view).unwrap();
        let bi = invocation(&current.baseline_invocation, current.baseline_invocation.authority);
        let second_baseline = lc631_runtime::contextual_repair::execution::observe_current(h.plan, bi, view).unwrap();
        assert!(!std::ptr::eq(h.baseline, &second_baseline));
        assert_eq!(h.baseline.actual_outputs(), second_baseline.actual_outputs());
        let second = g::measure_artifact_pair(&other,
            g::CodingPair { baseline: &second_baseline, candidate: h.candidate }, current, view).unwrap();
        first.require_same_baseline(&first, view).unwrap();
        assert!(matches!(first.require_same_baseline(&second, view), Err(g::Error::BaselineMismatch)));
        assert!(matches!(second.require_same_baseline(&first, view), Err(g::Error::BaselineMismatch)));
    });
}
