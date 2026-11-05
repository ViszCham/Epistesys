//! Actual owned TL is consumed, but absent original leaves never become scores.
mod tlcx19_checked_tl_support;
use lc631_analysis::creativity_measurement::{formula::*, ComponentMetric as M, Rational};
use lc631_runtime::{
    checked_tl_reduction as tl, original_score_pending as original,
    spatiotemporal_selection::{SpatialOwner, TemporalOwner},
};
use lc631_solvers::temporal::{EvaluationBudget, Truth};
use tlcx19_checked_tl_support::*;

fn profile(root: &tl::RootPlan<'_, '_, '_>) -> FormulaProfile {
    let q = || Rational::from_integer(1.into());
    FormulaProfile::checked(
        FormulaBinding {
            comparison_scope: root.scope().comparison(),
            baseline_revision: root.digest(),
            baseline_candidate: d(81),
            encoding_revision: d(82),
            tl_reduction_policy: root.digest(),
        },
        CoefficientSet {
            f1: std::array::from_fn(|_| q()),
            f2: std::array::from_fn(|_| q()),
            structural: std::array::from_fn(|_| q()),
            reorganization: std::array::from_fn(|_| q()),
            utility: std::array::from_fn(|_| q()),
        },
        ScaleParameters {
            baseline_bits: q(),
            sigma_bits: q(),
            match_scale_bits: q(),
        },
        &[(M::DescriptionBits, d(83))],
        64,
    )
    .unwrap()
}

#[test]
fn tlcx22_actual_zero_tl_still_has_missing_original_leaves_and_no_selection() {
    let q = query();
    let trace = trace(Truth::True);
    let a = temporal(&q, &trace);
    let s = spatial(false);
    let tp = time_policy(a, a);
    let sp = space_policy(s, s, false);
    let cancel = Cancellation::default();
    let (_b, _r, res) = run(cancel.clone());
    let view = res.lease();
    let root = tl::RootPlan::capture(a, s, &tp, &sp, &view).unwrap();
    let profile = profile(&root);
    let plan = original::RootPlan::capture(&root, &profile, OriginalFormula::F1V1, &view).unwrap();
    let candidate = tl::TupleCandidate::capture(&root, a, s).unwrap();
    let other = tl::TupleCandidate::capture(&root, a, s).unwrap();
    let t = TemporalOwner::produce(&tp, a, a, &view, &view, EvaluationBudget::default()).unwrap();
    let s = SpatialOwner::produce(&sp, s, s, &view, &view).unwrap();
    let reduction = tl::reduce_current(
        &root,
        &candidate,
        tl::Owners {
            temporal: Some(&t),
            spatial: Some(&s),
        },
        &view,
    )
    .unwrap();
    let receipt = original::capture_current(&plan, &candidate, &reduction, &view).unwrap();
    // No report rebinding or copied endpoint substitutes for the actual owner.
    assert!(std::ptr::eq(receipt.translation_owner(), &reduction));
    let observed = receipt.current(&candidate, &view).unwrap();
    assert_eq!(observed.translation_loss(), Some(&bounds(0, 1, 0, 1)));
    assert!(observed.missing_leaves().contains(&M::DescriptionBits));
    assert!(observed.missing_leaves().contains(&M::Knownness));
    assert!(observed.scalar().is_none());
    assert!(!observed.formula_evaluated());
    assert!(receipt.current(&other, &view).is_err());
    let selection = original::select_current(&plan, &[&receipt], &view).unwrap();
    assert_eq!(selection.recommendation(), None);
    assert_eq!(selection.pending_count(), 1);
    let other_plan =
        original::RootPlan::capture(&root, &profile, OriginalFormula::F2V1, &view).unwrap();
    assert!(original::select_current(&other_plan, &[&receipt], &view).is_err());
    assert!(original::select_current(&plan, &[], &view).is_err());
    let (_b2, _r2, res2) = run(Cancellation::default());
    assert!(receipt.current(&candidate, &res2.lease()).is_err());
    cancel.cancel();
    assert!(receipt.current(&candidate, &view).is_err());
    assert!(original::select_current(&plan, &[&receipt], &view).is_err());
}

#[test]
fn tlcx22_missing_actual_tl_is_not_zero_and_f2_remains_strictly_pending() {
    let q = query();
    let trace = trace(Truth::True);
    let a = temporal(&q, &trace);
    let s = spatial(false);
    let tp = time_policy(a, a);
    let sp = space_policy(s, s, false);
    let (_b, _r, res) = run(Cancellation::default());
    let view = res.lease();
    let root = tl::RootPlan::capture(a, s, &tp, &sp, &view).unwrap();
    let profile = profile(&root);
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
    let receipt = original::capture_current(&plan, &candidate, &reduction, &view).unwrap();
    let observed = receipt.current(&candidate, &view).unwrap();
    assert!(observed.translation_loss().is_none());
    assert_eq!(
        observed.translation_gaps(),
        [Some(tl::NoScalar::MissingOwner); 2]
    );
    for metric in [
        M::FrameworkDistance,
        M::Prediction,
        M::Preserve,
        M::Design,
        M::DescriptionBits,
        M::BridgeCost,
    ] {
        assert!(observed.missing_leaves().contains(&metric));
    }
    assert!(observed.scalar().is_none());
    assert_eq!(
        original::select_current(&plan, &[&receipt], &view)
            .unwrap()
            .recommendation(),
        None
    );
}
