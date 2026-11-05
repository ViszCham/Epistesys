//! 日本語: CR-DSEMの予定APIに対する負例・実接続受入試験。source実装はまだない。
//! English: Negative/actual-connection acceptance cases for a planned CR-DSEM API.
//! Until central registration, E0432 is construction RED, never semantic RED.
#[path = "tlcx22_semantic_configuration_support/mod.rs"]
mod support;
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentState, IndependenceStatus, InputBasis, RationalInterval, SemanticValidation,
};
use lc631_runtime::creative_candidate_bridge::semantic_configuration::*;
use std::cell::Cell;
use support::{run, Case, Cut};

fn bounds(report: &lc631_analysis::creativity_measurement::ComponentReport) -> &RationalInterval {
    match &report.state {
        ComponentState::Measured(value) => value,
        other => panic!("expected finite K1 bound, got {other:?}"),
    }
}

#[test]
fn tlcx22_dsem_rejects_required_role_and_disguised_actual_hard_predicate() {
    for cut in [Cut::RequiredRole, Cut::DisguisedHardR] {
        let calls = Cell::new(0);
        assert!(matches!(run(Case::Change, cut, &calls), Err(Error::RequiredProbe)));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn tlcx22_dsem_inconsistent_baseline_and_unsupported_probe_never_become_unknown_bounds() {
    let calls = Cell::new(0);
    assert!(matches!(run(Case::InconsistentBaseline, Cut::None, &calls),
        Err(Error::NoComparable(lc631_runtime::formal_boolean::NoComparable::SourceContextInconsistent))));
    assert_eq!(calls.get(), 0);
    assert!(matches!(run(Case::Change, Cut::UnsupportedProbe, &calls),
        Err(Error::Formal(lc631_runtime::formal_boolean::Failure::UnsupportedInteger))));
    assert_eq!(calls.get(), 0);
}

#[test]
fn tlcx22_dsem_actual_generated_permitted_change_has_hand_oracle_three_quarters() {
    let calls = Cell::new(0);
    let value = run(Case::Change, Cut::None, &calls).unwrap();
    assert_eq!(value.report.metric, ComponentMetric::SemanticConfigurationDistance);
    assert_eq!(value.report.basis, InputBasis::Given);
    assert_eq!(value.report.independence, IndependenceStatus::SharedDependency);
    assert_eq!(value.report.validation, SemanticValidation::FiniteExhaustive);
    assert_eq!(bounds(&value.report), &RationalInterval::exact_ratio(3, 4).unwrap());
    assert_eq!(value.baseline_p, CanonicalOutcome::Supported);
    assert_eq!(value.candidate_p, CanonicalOutcome::Undetermined);
    assert_eq!(value.candidate_q, CanonicalOutcome::Supported);
    // Γz has r=q=true and either p; all eight assignments were truly checked.
    assert_eq!(value.candidate_models, 2);
    assert_eq!(value.initial_queries, 12); // four hard + eight optional self-queries.
    assert_eq!(value.follow_up, Some(CanonicalOutcome::Supported));
    assert_eq!(value.follow_up_counts, Some((12, 14)));
    assert!(!value.fresh_current_rejected);
    assert_eq!(calls.get(), 1);
}

#[test]
fn tlcx22_dsem_demorgan_same_canonical_outcomes_are_zero_and_dispatch_no_follow_up() {
    let value = run(Case::Equivalent, Cut::None, &Cell::new(0)).unwrap();
    assert_eq!(bounds(&value.report), &RationalInterval::exact_ratio(0, 1).unwrap());
    assert_eq!(value.report.validation, SemanticValidation::FiniteExhaustive);
    assert_eq!(value.baseline_p, CanonicalOutcome::Supported);
    assert_eq!(value.candidate_p, CanonicalOutcome::Supported);
    assert_eq!(value.follow_up, None);
    assert_eq!(value.follow_up_counts, None);
    assert_eq!(value.initial_queries, 12);
}

#[test]
fn tlcx22_dsem_refuted_and_undetermined_are_actual_distinct_canonical_outcomes() {
    let value = run(Case::Negative, Cut::None, &Cell::new(0)).unwrap();
    assert_eq!(value.baseline_p, CanonicalOutcome::Refuted);
    assert_eq!(value.report.validation, SemanticValidation::FiniteExhaustive);
    assert_eq!(value.candidate_p, CanonicalOutcome::Undetermined);
    assert_eq!(bounds(&value.report), &RationalInterval::exact_ratio(3, 4).unwrap());
    assert_eq!(value.follow_up_counts, Some((12, 14)));
}

#[test]
fn tlcx22_dsem_missing_dispatch_preserves_denominator_and_executes_fresh_support() {
    let value = run(Case::Change, Cut::MissingCandidateP, &Cell::new(0)).unwrap();
    assert_eq!(value.baseline_p, CanonicalOutcome::Supported);
    assert_eq!(value.candidate_p, CanonicalOutcome::Unknown(OutcomeGap::MissingDispatch));
    assert_eq!(value.report.validation, SemanticValidation::Unassessed);
    assert_eq!(bounds(&value.report), &RationalInterval::from_ratio(0, 1, 3, 4).unwrap());
    assert_eq!(value.initial_queries, 10);
    assert_eq!(value.follow_up, Some(CanonicalOutcome::Undetermined));
    assert_eq!(value.follow_up_counts, Some((10, 12)));
    // Fresh observation does not rewrite the historical bounded measurement.
    assert_eq!(bounds(&value.report), &RationalInterval::from_ratio(0, 1, 3, 4).unwrap());
}

#[test]
fn tlcx22_dsem_work_unknown_is_not_semantic_undetermined_and_fresh_result_is_actual() {
    let value = run(Case::Change, Cut::UnknownCandidateP, &Cell::new(0)).unwrap();
    assert_eq!(value.candidate_p, CanonicalOutcome::Unknown(OutcomeGap::WorkExhausted));
    assert_eq!(value.report.validation, SemanticValidation::Unassessed);
    assert_eq!(bounds(&value.report), &RationalInterval::from_ratio(0, 1, 3, 4).unwrap());
    assert_eq!(value.follow_up, Some(CanonicalOutcome::Undetermined));
    assert_eq!(value.follow_up_counts, Some((12, 14)));
}

#[test]
fn tlcx22_dsem_foreign_binding_stale_plan_and_schedule_cuts_never_enter_consumer() {
    for cut in [Cut::CopySource, Cut::ForeignBridge, Cut::ForeignLease, Cut::ForeignPlan,
        Cut::StaleDomain, Cut::ForeignDispatch, Cut::DuplicateDispatch] {
        let calls = Cell::new(0);
        assert!(run(Case::Change, cut, &calls).is_err(), "cut {cut:?}");
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn tlcx22_dsem_fixed_positive_weights_duplicate_work_and_resource_cuts_fail() {
    for cut in [Cut::EmptyProbes, Cut::DuplicateProbe, Cut::ZeroWeight, Cut::NumericBudget, Cut::MalformedWeight,
        Cut::Work, Cut::Payload, Cut::Output] {
        let calls = Cell::new(0);
        assert!(run(Case::Change, cut, &calls).is_err(), "cut {cut:?}");
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn tlcx22_dsem_tied_unknown_weights_need_evidence_without_id_or_order_tiebreak() {
    for cut in [Cut::Tie, Cut::TieRenamed, Cut::TieReordered] {
        let value = run(Case::Change, cut, &Cell::new(0)).unwrap();
        assert!(value.ambiguous_follow_up);
        assert_eq!(value.initial_queries, 4);
        assert_eq!(value.follow_up, None);
        assert_eq!(value.report.validation, SemanticValidation::Unassessed);
        assert_eq!(bounds(&value.report), &RationalInterval::from_ratio(0, 1, 1, 1).unwrap());
    }
}

#[test]
fn tlcx22_dsem_follow_up_cannot_mint_a_new_root_work_allowance() {
    let value = run(Case::Change, Cut::FollowUpWork, &Cell::new(0)).unwrap();
    assert_eq!(bounds(&value.report), &RationalInterval::exact_ratio(3, 4).unwrap());
    assert_eq!(value.initial_queries, 12);
    assert_eq!(value.follow_up, None);
    assert_eq!(value.follow_up_counts, None);
}

#[test]
fn tlcx22_dsem_cancellation_rejects_current_and_fresh_results_without_rollback_claim() {
    for (cut, expected_calls) in [(Cut::CancelBefore, 0), (Cut::CancelInside, 1), (Cut::CancelFresh, 1)] {
        let calls = Cell::new(0);
        assert!(run(Case::Change, cut, &calls).is_err());
        assert_eq!(calls.get(), expected_calls);
    }
}
