//! TEST-FIRST/API: construction RED is pending centrally; no producer stub exists.
use lc631_analysis::creativity_measurement::{formula::OriginalFormula, ComponentMetric as M, *};
use lc631_core::{
    context::{
        ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis, SourceRevision,
    },
    AuthorityRevision,
};
use lc631_runtime::{
    context_novelty_measurement as n,
    creative_candidate_bridge::*,
    resource::{ResourceBroker, ResourceLimits},
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, IntExpr, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    resource::*,
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{cell::Cell, sync::Arc, time::Duration};
mod tlcx22_context_novelty_support;
use tlcx22_context_novelty_support::*;

#[test]
fn tlcx22_ctx_p01_actual_demorgan_semantic_zero_is_not_construction_identity() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (_, _, _, _, _, _, _, _, root, dm, _, view, _, _) = f;
        let expected = root.expected_inputs_current(dm, view).unwrap();
        let owner = root.measure_current(expected, view).unwrap();
        owner
            .with_current(expected, view, |current| {
                assert_point(current.report(M::ContextDistance).unwrap(), 0, 1);
                assert_point(current.report(M::FrameworkDistance).unwrap(), 0, 1);
                assert_point(current.report(M::Knownness).unwrap(), 1, 3);
                assert_eq!(current.archive_status(), n::ArchiveStatus::Complete);
            })
            .unwrap();
    });
}
#[test]
fn tlcx22_ctx_p02_fixed_global_domain_does_not_make_each_mutable_gamma_tautological() {
    assert_ne!(AND_TABLE, P_TABLE); // Independent literal rows, not source-Γ entailment.
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (_, _, _, _, _, _, _, _, root, _, weak, view, _, _) = f;
        let expected = root.expected_inputs_current(weak, view).unwrap();
        let owner = root.measure_current(expected, view).unwrap();
        owner
            .with_current(expected, view, |c| {
                assert_point(c.report(M::ContextDistance).unwrap(), 1, 1);
                assert_eq!(c.dctx_union_weight(), &q(2, 1));
                assert_point(c.report(M::Knownness).unwrap(), 1, 3);
            })
            .unwrap();
    });
}
#[test]
fn tlcx22_ctx_p03_actual_all_entry_max_scan_includes_exact_construction() {
    ctx_fixture!(f, ArchiveMode::ExactWeak, limits(), false, {
        let (_, _, _, _, _, _, _, _, root, _, weak, view, _, _) = f;
        let expected = root.expected_inputs_current(weak, view).unwrap();
        let owner = root.measure_current(expected, view).unwrap();
        owner
            .with_current(expected, view, |c| {
                assert_point(c.report(M::Knownness).unwrap(), 1, 1);
                assert_eq!(c.counts().archive_comparisons, 2);
            })
            .unwrap();
    });
}
#[test]
fn tlcx22_ctx_p04_registered_commutativity_and_global_alpha_map_are_invariant() {
    for rename in [false, true] {
        ctx_fixture!(f, ArchiveMode::CommuteDeMorgan, limits(), rename, {
            let (_, _, _, _, _, _, _, _, root, dm, _, view, _, _) = f;
            let e = root.expected_inputs_current(dm, view).unwrap();
            let owner = root.measure_current(e, view).unwrap();
            owner
                .with_current(e, view, |c| {
                    assert_point(c.report(M::Knownness).unwrap(), 1, 1);
                    assert_point(c.report(M::ContextDistance).unwrap(), 0, 1);
                })
                .unwrap();
        });
    }
}
#[test]
fn tlcx22_ctx_n01_empty_unread_unavailable_and_unsupported_archive_are_not_zero() {
    for mode in [
        ArchiveMode::Empty,
        ArchiveMode::Unread,
        ArchiveMode::Unavailable,
        ArchiveMode::Unsupported,
    ] {
        ctx_fixture!(f, mode, limits(), false, {
            let (_, _, _, _, _, _, _, _, root, dm, _, view, _, _) = f;
            let e = root.expected_inputs_current(dm, view).unwrap();
            let owner = root.measure_current(e, view).unwrap();
            owner
                .with_current(e, view, |c| {
                    let known = c.report(M::Knownness).unwrap();
                    match mode {
                        ArchiveMode::Empty => {
                            assert_eq!(c.archive_status(), n::ArchiveStatus::Empty);
                            assert_eq!(
                                known.state,
                                ComponentState::Undefined(MeasurementError::EmptyCorpus)
                            );
                        }
                        ArchiveMode::Unread => {
                            assert_eq!(c.archive_status(), n::ArchiveStatus::Unread);
                            assert!(matches!(known.state, ComponentState::Unknown(_)));
                        }
                        ArchiveMode::Unavailable => {
                            assert_eq!(c.archive_status(), n::ArchiveStatus::Unavailable);
                            assert!(matches!(known.state, ComponentState::Unknown(_)));
                        }
                        ArchiveMode::Unsupported => {
                            assert_eq!(c.archive_status(), n::ArchiveStatus::Unsupported);
                            assert_eq!(
                                known.state,
                                ComponentState::Unknown(MeasurementGap::Unsupported)
                            );
                        }
                        _ => unreachable!(),
                    }
                })
                .unwrap();
        });
    }
}
#[test]
fn tlcx22_ctx_n02_same_signature_zero_does_not_rescue_undefined_tnc_or_original_score() {
    ctx_fixture!(f, ArchiveMode::Empty, limits(), false, {
        let (_, _, _, _, _, _, _, _, root, dm, _, view, _, _) = f;
        let e = root.expected_inputs_current(dm, view).unwrap();
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |c| {
                assert_point(c.report(M::FrameworkDistance).unwrap(), 0, 1);
                assert!(c.report(M::NoveltyCompatibility).is_none());
                for formula in [OriginalFormula::F1V1, OriginalFormula::F2V1] {
                    let n::OriginalStatus::NoScalar { missing } = c.original_status(formula);
                    assert!(missing.contains(&M::SemanticConfigurationDistance));
                    assert!(missing.contains(&M::Compatibility));
                }
                for exit in [
                    n::PendingExit::NonzeroFramework,
                    n::PendingExit::CrossFrameworkSemantic,
                    n::PendingExit::CrossFrameworkCompatibility,
                    n::PendingExit::NormativeTnc,
                    n::PendingExit::OriginalScoreSelection,
                ] {
                    assert!(c.pending_exits().contains(&exit));
                }
            })
            .unwrap();
    });
}
#[test]
fn tlcx22_ctx_n03_not_polarity_is_not_removed_by_commutative_normalization() {
    ctx_fixture!(f, ArchiveMode::Baseline, limits(), false, {
        let (_, _, _, _, _, _, _, _, root, dm, _, view, _, _) = f;
        let e = root.expected_inputs_current(dm, view).unwrap();
        let owner = root.measure_current(e, view).unwrap();
        owner
            .with_current(e, view, |c| {
                assert_point(c.report(M::Knownness).unwrap(), 1, 3)
            })
            .unwrap();
    });
}
#[test]
fn tlcx22_ctx_n04_no_clone_on_actual_owners_deserialize_negative_is_pending() {
    macro_rules! not_impl {
        ($ty:ty:$bound:path) => {{
            trait Ambiguous<A> {
                fn check() {}
            }
            impl<T: ?Sized> Ambiguous<()> for T {}
            struct IfImpl;
            impl<T: ?Sized + $bound> Ambiguous<IfImpl> for T {}
            let _ = <$ty as Ambiguous<_>>::check;
        }};
    }
    not_impl!(n::RootPlan<'static,'static>:Clone);
    not_impl!(n::OwnedAssessment<'static,'static,'static>:Clone);
    not_impl!(n::CurrentAssessment<'static>:Clone);
    // No direct serde dependency is added. Deserialize stays forbidden; the
    // serde_json::from_str::<OwnedAssessment> compile_fail packet awaits BODY.
}
#[path = "tlcx22_context_novelty_support/admission.rs"]
mod admission;
#[path = "tlcx22_context_novelty_support/current.rs"]
mod current;
