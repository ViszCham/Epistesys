//! Planned API construction RED until central registers composite_original.
//! Given membership never becomes semantic applicability, full K/TL or original success.
#[path = "tlcx22_composite_original_support/mod.rs"]
mod support;
use lc631_analysis::creativity_measurement::{formula::OriginalFormula, ComponentMetric as M};
use lc631_runtime::{
    composite_original::*,
    resource::{Cancellation, ResourceError},
};
use support::*;

#[test]
fn c01_missing_actual_source_is_rejected_before_conditional_capture() {
    let source = source();
    let decl = declarations();
    let apps = applicability();
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    assert!(matches!(
        ConditionalRootPlan::capture(spec(None, &decl, &apps), &view),
        Err(Error::SourceMissing)
    ));
}
#[test]
fn c02_membership_keeps_no_k_no_tl_no_hard_and_all_strict_leaves() {
    let source = source();
    let decl = declarations();
    let apps = applicability();
    let coords = coordinates();
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    let root = ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    assert_eq!(root.provenance(), ConditionalProvenance::GivenUnvalidated);
    let candidate = ConditionalCandidate::admit(
        &root,
        CandidateSpec {
            coordinates: &coords,
            baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1,
        },
        &view,
    )
    .unwrap();
    let pending = PendingComposite::capture(&root, &candidate, &view).unwrap();
    for (formula, missing) in [(OriginalFormula::F1V1, F1), (OriginalFormula::F2V1, F2)] {
        pending
            .with_current(
                formula,
                ExpectedPending {
                    root: &root,
                    candidate: &candidate,
                },
                &view,
                |current| {
                    assert_eq!(current.missing_leaves(), missing);
                    for gap in [
                        MissingDependency::FullCompositeCost,
                        MissingDependency::CompositeTotalTl,
                        MissingDependency::ActualHardEvidence,
                        MissingDependency::NativeOwnerCorrespondence,
                        MissingDependency::SemanticApplicability,
                    ] {
                        assert!(current.gaps().contains(&gap));
                    }
                    assert_eq!(
                        current.provenance(),
                        ConditionalProvenance::GivenUnvalidated
                    );
                    assert_eq!(current.causal_validation(), CausalValidation::Unassessed);
                    assert_eq!(current.hard_status(), HardStatus::NeedsEvidence);
                    assert!(current.scalar().is_none());
                    assert!(!current.formula_evaluated());
                    assert_eq!(current.recommendation(), None);
                    assert!(std::ptr::eq(current.source(), &source));
                },
            )
            .unwrap();
    }
}
#[test]
fn c03_unsupported_root_relation_survives_complete_coordinate_membership() {
    let source = source();
    let decl = declarations();
    let mut apps = applicability();
    let coords = coordinates();
    apps.iter_mut()
        .find(|a| a.metric == M::Design)
        .unwrap()
        .claim = ApplicabilityClaim::Unsupported(ApplicabilityReason::UnregisteredRootRelation);
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    let root = ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    let candidate = ConditionalCandidate::admit(
        &root,
        CandidateSpec {
            coordinates: &coords,
            baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1,
        },
        &view,
    )
    .unwrap();
    let pending = PendingComposite::capture(&root, &candidate, &view).unwrap();
    pending
        .with_current(
            OriginalFormula::F2V1,
            ExpectedPending {
                root: &root,
                candidate: &candidate,
            },
            &view,
            |current| {
                assert_eq!(current.unsupported_leaves(), &[M::Design]);
                assert!(current.missing_leaves().contains(&M::Design));
                assert!(current.scalar().is_none());
                assert_eq!(current.recommendation(), None);
            },
        )
        .unwrap();
}
#[test]
fn c04_missing_applicability_is_not_inferred_from_package_membership() {
    let source = source();
    let decl = declarations();
    let mut apps = applicability();
    apps.retain(|a| a.metric != M::Explain);
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    assert!(matches!(
        ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view),
        Err(Error::MissingApplicability(M::Explain))
    ));
}
#[test]
fn c05_hard_coordinate_cut_and_retagged_schema_are_rejected() {
    let source = source();
    let decl = declarations();
    let apps = applicability();
    let mut coords = coordinates();
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    let root = ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    coords.retain(|c| c.coordinate != Coordinate::Graph);
    assert!(matches!(
        ConditionalCandidate::admit(
            &root,
            CandidateSpec {
                coordinates: &coords,
                baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1
            },
            &view
        ),
        Err(Error::MissingCoordinate(Coordinate::Graph))
    ));
    let mut wrong = coordinates();
    wrong[0].schema = d(99);
    assert!(matches!(
        ConditionalCandidate::admit(
            &root,
            CandidateSpec {
                coordinates: &wrong,
                baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1
            },
            &view
        ),
        Err(Error::CoordinateSchemaMismatch(Coordinate::Hypothesis))
    ));
    // This cuts only mandatory DECLARED membership, not actual semantic hard evidence.
}
#[test]
fn c06_equal_content_foreign_root_candidate_or_reservation_cannot_refresh_pending() {
    let source = source();
    let decl = declarations();
    let apps = applicability();
    let coords = coordinates();
    let cancel = Cancellation::default();
    let (_b, _r, res) = run(&source, cancel.clone());
    let view = res.lease();
    let root = ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    let other_root =
        ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    let input = || CandidateSpec {
        coordinates: &coords,
        baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1,
    };
    let candidate = ConditionalCandidate::admit(&root, input(), &view).unwrap();
    let other = ConditionalCandidate::admit(&root, input(), &view).unwrap();
    let pending = PendingComposite::capture(&root, &candidate, &view).unwrap();
    assert!(matches!(
        pending.with_current(
            OriginalFormula::F2V1,
            ExpectedPending {
                root: &other_root,
                candidate: &candidate
            },
            &view,
            |_| ()
        ),
        Err(Error::ForeignRoot)
    ));
    assert!(matches!(
        pending.with_current(
            OriginalFormula::F2V1,
            ExpectedPending {
                root: &root,
                candidate: &other
            },
            &view,
            |_| ()
        ),
        Err(Error::ForeignCandidate)
    ));
    let (_b2, _r2, res2) = run(&source, Cancellation::default());
    let view2 = res2.lease();
    assert!(matches!(
        pending.with_current(
            OriginalFormula::F2V1,
            ExpectedPending {
                root: &root,
                candidate: &candidate
            },
            &view2,
            |_| ()
        ),
        Err(Error::ForeignReservation)
    ));
    cancel.cancel();
    assert!(matches!(
        pending.with_current(
            OriginalFormula::F2V1,
            ExpectedPending {
                root: &root,
                candidate: &candidate
            },
            &view,
            |_| ()
        ),
        Err(Error::Resource(ResourceError::Cancelled))
    ));
    assert!(view2.check().is_ok());
}
#[test]
fn c07_coding_baseline_profiles_cannot_be_mixed() {
    let source = source();
    let decl = declarations();
    let apps = applicability();
    let coords = coordinates();
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    let root = ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    assert!(matches!(
        ConditionalCandidate::admit(
            &root,
            CandidateSpec {
                coordinates: &coords,
                baseline_profile: CodingBaselineProfile::WrappingAddFourV1
            },
            &view
        ),
        Err(Error::BaselineProfileMismatch)
    ));
}
#[test]
fn c08_foreign_goal_applicability_is_rejected_before_candidates() {
    let source = source();
    let decl = declarations();
    let mut apps = applicability();
    apps.iter_mut()
        .find(|a| a.metric == M::Explain)
        .unwrap()
        .root_goal = d(77);
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    assert!(matches!(
        ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view),
        Err(Error::ApplicabilityGoalMismatch(M::Explain))
    ));
}
#[test]
fn c09_callback_cancellation_rejects_even_a_pending_snapshot() {
    let source = source();
    let decl = declarations();
    let apps = applicability();
    let coords = coordinates();
    let cancel = Cancellation::default();
    let (_b, _r, res) = run(&source, cancel.clone());
    let view = res.lease();
    let root = ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    let candidate = ConditionalCandidate::admit(
        &root,
        CandidateSpec {
            coordinates: &coords,
            baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1,
        },
        &view,
    )
    .unwrap();
    let pending = PendingComposite::capture(&root, &candidate, &view).unwrap();
    let outcome = pending.with_current(
        OriginalFormula::F2V1,
        ExpectedPending {
            root: &root,
            candidate: &candidate,
        },
        &view,
        |current| {
            assert!(current.scalar().is_none());
            cancel.cancel();
        },
    );
    assert!(matches!(
        outcome,
        Err(Error::Resource(ResourceError::Cancelled))
    ));
}
#[test]
fn c10_restoring_same_declaration_requires_fresh_current_owners_and_stays_no_scalar() {
    let source = source();
    let decl = declarations();
    let apps = applicability();
    let coords = coordinates();
    let (_b, _r, res) = run(&source, Cancellation::default());
    let view = res.lease();
    let root = ConditionalRootPlan::capture(spec(Some(&source), &decl, &apps), &view).unwrap();
    let mut cut = coordinates();
    cut.retain(|c| c.coordinate != Coordinate::Coding);
    assert!(matches!(
        ConditionalCandidate::admit(
            &root,
            CandidateSpec {
                coordinates: &cut,
                baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1
            },
            &view
        ),
        Err(Error::MissingCoordinate(Coordinate::Coding))
    ));
    let restored = ConditionalCandidate::admit(
        &root,
        CandidateSpec {
            coordinates: &coords,
            baseline_profile: CodingBaselineProfile::AlwaysNoneFourV1,
        },
        &view,
    )
    .unwrap();
    let pending = PendingComposite::capture(&root, &restored, &view).unwrap();
    pending
        .with_current(
            OriginalFormula::F2V1,
            ExpectedPending {
                root: &root,
                candidate: &restored,
            },
            &view,
            |current| {
                assert_eq!(current.missing_leaves(), F2);
                assert!(current.scalar().is_none());
                assert_eq!(current.recommendation(), None);
            },
        )
        .unwrap();
}
