//! 日本語: 原式preflightの欠測・binding・実lease境界。式評価の成功ではない。
//! English: Missing-leaf, binding and actual-lease cuts, not successful formula evaluation.
mod tlcx19_checked_tl_support;

use lc631_analysis::creativity_measurement::{formula::*, ComponentMetric as M, Rational};
use lc631_runtime::{
    checked_tl_reduction as tl, original_score_pending as original,
    resource::*,
    spatiotemporal_selection::{SpatialOwner, TemporalOwner},
};
use lc631_solvers::temporal::{EvaluationBudget, Truth};
use std::time::{Duration, Instant};
use tlcx19_checked_tl_support::*;

const F1_LEAVES: &[M] = &[
    M::DescriptionBits,
    M::ContextDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Knownness,
];
const F2_LEAVES: &[M] = &[
    M::DescriptionBits,
    M::FrameworkDistance,
    M::SemanticConfigurationDistance,
    M::Compatibility,
    M::Coverage,
    M::Compression,
    M::Bridge,
    M::Prediction,
    M::Redundancy,
    M::Preserve,
    M::Rebind,
    M::Reframe,
    M::BreakCost,
    M::Explain,
    M::Design,
    M::Transfer,
    M::Solve,
    M::CounterexampleRisk,
    M::BridgeCost,
];

fn binding(root: &tl::RootPlan<'_, '_, '_>) -> FormulaBinding {
    FormulaBinding {
        comparison_scope: root.scope().comparison(),
        baseline_revision: root.digest(),
        baseline_candidate: d(81),
        encoding_revision: d(82),
        tl_reduction_policy: root.digest(),
    }
}

// All nested coefficients are zero. The selected formula is either entirely
// degenerate, or has just one nonzero outer coefficient. Neither permits
// unavailable leaves to be replaced by zero.
fn profile(binding: FormulaBinding, all_zero: bool) -> FormulaProfile {
    let zero = || Rational::from_integer(0.into());
    let one = || Rational::from_integer(1.into());
    let mut coefficients = CoefficientSet {
        f1: std::array::from_fn(|_| zero()),
        f2: std::array::from_fn(|_| zero()),
        structural: std::array::from_fn(|_| zero()),
        reorganization: std::array::from_fn(|_| zero()),
        utility: std::array::from_fn(|_| zero()),
    };
    if !all_zero {
        coefficients.f1[0] = one();
        coefficients.f2[0] = one();
    }
    FormulaProfile::checked(
        binding,
        coefficients,
        ScaleParameters {
            baseline_bits: one(),
            sigma_bits: one(),
            match_scale_bits: one(),
        },
        &[(M::DescriptionBits, d(83))],
        64,
    )
    .unwrap()
}

macro_rules! fixture {
    ($resources:expr; $root:ident, $view:ident, $a:ident, $s:ident, $tp:ident, $sp:ident) => {
        let query = query();
        let trace = trace(Truth::True);
        let $a = temporal(&query, &trace);
        let $s = spatial(false);
        let $tp = time_policy($a, $a);
        let $sp = space_policy($s, $s, false);
        let (_broker, _run, reservation) = $resources;
        let $view = reservation.lease();
        let $root = tl::RootPlan::capture($a, $s, &$tp, &$sp, &$view).unwrap();
    };
}

#[test]
fn tlcx22_zero_coefficients_never_erase_complete_ordered_missing_leaves() {
    fixture!(run(Cancellation::default()); root, view, a, s, tp, sp);
    for (formula, expected) in [
        (OriginalFormula::F1V1, F1_LEAVES),
        (OriginalFormula::F2V1, F2_LEAVES),
    ] {
        let zero = profile(binding(&root), true);
        assert!(zero.ranking_allowed(formula).is_err());
        assert!(matches!(
            original::RootPlan::capture(&root, &zero, formula, &view),
            Err(original::Error::ProfileMismatch)
        ));
        let partial = profile(binding(&root), false);
        let plan = original::RootPlan::capture(&root, &partial, formula, &view).unwrap();
        let candidate = tl::TupleCandidate::capture(&root, a, s).unwrap();
        let reduction = tl::reduce_current(
            &root,
            &candidate,
            tl::Owners {
                temporal: None,
                spatial: None,
            },
            &view,
        )
        .unwrap();
        let pending = original::capture_current(&plan, &candidate, &reduction, &view).unwrap();
        let observed = pending.current(&candidate, &view).unwrap();
        assert_eq!(observed.missing_leaves(), expected);
        assert_eq!(observed.translation_loss(), None);
        assert_eq!(
            observed.translation_gaps(),
            [Some(tl::NoScalar::MissingOwner); 2]
        );
        assert_eq!(observed.scalar(), None);
        assert!(!observed.formula_evaluated());
        let selection = original::select_current(&plan, &[&pending], &view).unwrap();
        assert_eq!(selection.pending_count(), 1);
        assert_eq!(selection.recommendation(), None);
    }
}

#[test]
fn tlcx22_profile_scope_baseline_and_reduction_policy_swaps_are_rejected() {
    fixture!(run(Cancellation::default()); root, view, a, s, tp, sp);
    for field in 0..3 {
        let mut changed = binding(&root);
        match field {
            0 => changed.comparison_scope = d(90),
            1 => changed.baseline_revision = d(91),
            _ => changed.tl_reduction_policy = d(92),
        }
        let profile = profile(changed, false);
        for formula in [OriginalFormula::F1V1, OriginalFormula::F2V1] {
            assert!(matches!(
                original::RootPlan::capture(&root, &profile, formula, &view),
                Err(original::Error::ProfileMismatch)
            ));
        }
    }
}

#[test]
fn tlcx22_equal_content_foreign_root_cannot_supply_an_actual_reduction() {
    fixture!(run(Cancellation::default()); root, view, a, s, tp, sp);
    let profile = profile(binding(&root), false);
    let plan = original::RootPlan::capture(&root, &profile, OriginalFormula::F1V1, &view).unwrap();
    let other_root = tl::RootPlan::capture(a, s, &tp, &sp, &view).unwrap();
    assert_eq!(root.digest(), other_root.digest());
    let other_candidate = tl::TupleCandidate::capture(&other_root, a, s).unwrap();
    let reduction = tl::reduce_current(
        &other_root,
        &other_candidate,
        tl::Owners {
            temporal: None,
            spatial: None,
        },
        &view,
    )
    .unwrap();
    assert!(matches!(
        original::capture_current(&plan, &other_candidate, &reduction, &view),
        Err(original::Error::Translation(tl::Error::BindingMismatch))
    ));
}

#[test]
fn tlcx22_duplicate_pending_entries_count_rows_without_recommending() {
    fixture!(run(Cancellation::default()); root, view, a, s, tp, sp);
    let profile = profile(binding(&root), false);
    let plan = original::RootPlan::capture(&root, &profile, OriginalFormula::F1V1, &view).unwrap();
    let candidate = tl::TupleCandidate::capture(&root, a, s).unwrap();
    let reduction = tl::reduce_current(
        &root,
        &candidate,
        tl::Owners {
            temporal: None,
            spatial: None,
        },
        &view,
    )
    .unwrap();
    let pending = original::capture_current(&plan, &candidate, &reduction, &view).unwrap();
    let selection = original::select_current(&plan, &[&pending, &pending], &view).unwrap();
    // This is deliberately a row count, not two unique candidates.
    assert_eq!(selection.pending_count(), 2);
    assert_eq!(selection.recommendation(), None);
}

#[test]
fn tlcx22_cancelled_root_is_not_renewed_by_a_fresh_same_source_lease() {
    let cancel = Cancellation::default();
    fixture!(run(cancel.clone()); root, view, a, s, tp, sp);
    let profile = profile(binding(&root), false);
    let plan = original::RootPlan::capture(&root, &profile, OriginalFormula::F1V1, &view).unwrap();
    let candidate = tl::TupleCandidate::capture(&root, a, s).unwrap();
    let reduction = tl::reduce_current(
        &root,
        &candidate,
        tl::Owners {
            temporal: None,
            spatial: None,
        },
        &view,
    )
    .unwrap();
    let pending = original::capture_current(&plan, &candidate, &reduction, &view).unwrap();
    cancel.cancel();
    let (_b, _r, fresh) = run(Cancellation::default());
    let fresh_view = fresh.lease();
    assert!(fresh_view.check().is_ok());
    assert_eq!(view.source_revision(), fresh_view.source_revision());
    // Plan capture alone does not certify the borrowed TL root as current.
    let fresh_plan =
        original::RootPlan::capture(&root, &profile, OriginalFormula::F1V1, &fresh_view).unwrap();
    assert!(matches!(
        original::capture_current(&fresh_plan, &candidate, &reduction, &fresh_view),
        Err(original::Error::Translation(tl::Error::Resource(
            ResourceError::Cancelled
        )))
    ));
    assert!(pending.current(&candidate, &view).is_err());
    assert!(original::select_current(&plan, &[&pending], &view).is_err());
}

#[test]
fn tlcx22_independently_cancelled_actual_owner_cannot_reuse_cached_zero_tl() {
    fixture!(run(Cancellation::default()); root, view, a, s, tp, sp);
    let source_cancel = Cancellation::default();
    let (_b, _r, source) = run(source_cancel.clone());
    let source_view = source.lease();
    let temporal =
        TemporalOwner::produce(&tp, a, a, &source_view, &view, EvaluationBudget::default())
            .unwrap();
    let spatial = SpatialOwner::produce(&sp, s, s, &view, &view).unwrap();
    let profile = profile(binding(&root), false);
    let plan = original::RootPlan::capture(&root, &profile, OriginalFormula::F1V1, &view).unwrap();
    let candidate = tl::TupleCandidate::capture(&root, a, s).unwrap();
    let reduction = tl::reduce_current(
        &root,
        &candidate,
        tl::Owners {
            temporal: Some(&temporal),
            spatial: Some(&spatial),
        },
        &view,
    )
    .unwrap();
    let pending = original::capture_current(&plan, &candidate, &reduction, &view).unwrap();
    assert_eq!(
        pending
            .current(&candidate, &view)
            .unwrap()
            .translation_loss(),
        Some(&bounds(0, 1, 0, 1))
    );
    source_cancel.cancel();
    assert!(view.check().is_ok());
    assert!(matches!(
        pending.current(&candidate, &view),
        Err(original::Error::Translation(tl::Error::Producer(
            lc631_runtime::spatiotemporal_selection::Error::Resource(ResourceError::Cancelled)
        )))
    ));
    assert!(original::select_current(&plan, &[&pending], &view).is_err());
}

fn deadline_run(deadline: Deadline) -> (ResourceBroker, RunResources, Reservation) {
    let cap = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker
        .begin_run(
            19,
            context().source().digest(),
            cap,
            deadline,
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(24 * 1024 * 1024).unwrap(),
        )])
        .unwrap();
    (broker, run, reservation)
}

#[test]
fn tlcx22_real_deadline_expiry_rejects_current_pending_without_a_scalar() {
    let expiry = Instant::now() + Duration::from_secs(2);
    fixture!(deadline_run(Deadline::at(expiry)); root, view, a, s, tp, sp);
    let profile = profile(binding(&root), false);
    let plan = original::RootPlan::capture(&root, &profile, OriginalFormula::F2V1, &view).unwrap();
    let candidate = tl::TupleCandidate::capture(&root, a, s).unwrap();
    let reduction = tl::reduce_current(
        &root,
        &candidate,
        tl::Owners {
            temporal: None,
            spatial: None,
        },
        &view,
    )
    .unwrap();
    let pending = original::capture_current(&plan, &candidate, &reduction, &view).unwrap();
    assert!(!pending
        .current(&candidate, &view)
        .unwrap()
        .formula_evaluated());
    // Wait for the real monotonic deadline, never mutate a reported status flag.
    while let Some(remaining) = expiry.checked_duration_since(Instant::now()) {
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
    assert_eq!(view.check(), Err(ResourceError::DeadlineExceeded));
    assert!(matches!(
        pending.current(&candidate, &view),
        Err(original::Error::Resource(ResourceError::DeadlineExceeded))
    ));
    assert!(matches!(
        original::capture_current(&plan, &candidate, &reduction, &view),
        Err(original::Error::Resource(ResourceError::DeadlineExceeded))
    ));
    assert!(original::select_current(&plan, &[&pending], &view).is_err());
}
