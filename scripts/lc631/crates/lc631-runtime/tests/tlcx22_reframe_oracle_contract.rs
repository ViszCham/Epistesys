//! Separate planned-RED target. Reframe oracle/body still require central review.
#![forbid(unsafe_code)]
#[path = "tlcx22_reorganization_support/frame_fixture.rs"]
mod frame_fixture;
use frame_fixture::*;

#[test]
fn tlcx22_reframe_direct_failure_to_checked_macro_explanation() {
    let actual=scenario(Case::CheckedMacro).unwrap();
    assert_eq!(actual.report.state,ComponentState::Measured(ratio(3,4)));
    assert_eq!(actual.baseline,vec![f::PerspectiveOutcome::KnownPass,f::PerspectiveOutcome::KnownFailure]);
    assert_eq!(actual.candidate,vec![f::PerspectiveOutcome::KnownPass,f::PerspectiveOutcome::KnownPass]);
    assert_eq!(actual.used_base_paths,vec![vec![0,1,2]]);
    assert_eq!(actual.denominator,4);
    assert_eq!(actual.report.basis,InputBasis::Given);
    assert_eq!(actual.report.validation,SemanticValidation::Unassessed);
    assert_eq!(actual.report.independence,IndependenceStatus::SharedDependency);
    assert_eq!(actual.report.metric,ComponentMetric::Reframe);
    assert!(actual.same_oracle_and_budget);
    assert!(actual.root_reachability_before && actual.root_reachability_after);
    assert!(matches!(scenario(Case::MissingPath),Err(f::Error::MissingPathOwner)));
    assert!(matches!(scenario(Case::WrongEndpoint),Err(f::Error::MapBackEndpoints)));
}

#[test]
fn tlcx22_reframe_baseline_unknown_not_failure() {
    let actual=scenario(Case::TinyOracleBudget).unwrap();
    assert_eq!(actual.baseline[1],f::PerspectiveOutcome::Unknown);
    assert_ne!(actual.report.state,ComponentState::Measured(ratio(3,4)));
    assert_ne!(actual.report.state,ComponentState::Measured(ratio(0,1)));
    assert!(actual.has_gaps);
    assert_eq!(actual.denominator,4);
    assert!(matches!(scenario(Case::MissingBaselineOwner),Err(f::Error::MissingPaidMapOwner)));
}

#[test]
fn tlcx22_reframe_frame_label_or_unused_macro_not_gain() {
    let unused=scenario(Case::UnusedMacro).unwrap();
    assert_eq!(unused.report.state,ComponentState::Measured(ratio(0,1)));
    assert!(unused.used_base_paths.is_empty());
    let identity=scenario(Case::IdentityFrame).unwrap();
    assert_eq!(identity.report.state,ComponentState::Measured(ratio(0,1)));
    assert!(matches!(scenario(Case::LabelOnlyDelta),Err(f::Error::FrameDeltaMismatch)));
    assert!(matches!(scenario(Case::UnregisteredMacroRule),Err(f::Error::Unsupported(f::Cause::UnsupportedProfile))));
    assert!(matches!(scenario(Case::OracleSwap),Err(f::Error::NoComparable(f::Cause::ForeignPlan))));
    assert!(matches!(scenario(Case::HardErase),Err(f::Error::Context(ContextError::HardInvariant))));
    assert!(matches!(scenario(Case::AuthoritySwap),Err(f::Error::Context(ContextError::AuthorityMismatch))));
    assert!(matches!(scenario(Case::GoalScopeSwap),Err(f::Error::Context(ContextError::ScopeMismatch))));
    // Macro content cannot be a new edge in the immutable base registry.
    // Both frames use the SAME registered expansion oracle and fixed queries.
}
