//! Profile B exact-frame TEST-FIRST. No producer implementation or success stub.
#![forbid(unsafe_code)]
#[path = "tlcx22_reframe_exact_support/mod.rs"]
mod support;
#[path = "tlcx22_reframe_exact_support/scenario.rs"]
mod scenario;
use support::*;
use scenario::*;

#[test]
fn tlcx22_reframe_exact_different_parser_lowering_evaluator_paid_map() {
    let observed = scene(Case::Valid).unwrap();
    assert_eq!(observed.report.state, ComponentState::Measured(ratio(3, 4)));
    assert_eq!(observed.baseline, vec![f::PerspectiveOutcome::KnownPass, f::PerspectiveOutcome::KnownFailure]);
    assert_eq!(observed.candidate, vec![f::PerspectiveOutcome::KnownPass, f::PerspectiveOutcome::KnownPass]);
    assert_eq!(observed.candidate_path, Some(vec![0, 1, 2]));
    assert_eq!(observed.baseline_profile, f::FrameProfile::DirectEdgeV1);
    assert_eq!(observed.candidate_profile, f::FrameProfile::CheckedPathMacroV1);
    assert_eq!(observed.candidate_rules, Some((
        f::ParseRule::CheckedMacroCallV1, f::LowerRule::CheckedPathReferenceV1,
        f::EvalRule::CheckedPathMacroV1,
    )));
    assert!(observed.candidate_map_work > 0, "map-back was not executed/charged");
    assert!(observed.root_before && observed.root_after);
    assert_eq!(observed.denominator, 4);
    assert_eq!(observed.recommendation, None);
    assert_eq!(observed.report.basis, InputBasis::Given);
    assert_eq!(observed.report.validation, SemanticValidation::Unassessed);
    assert_eq!(observed.report.independence, IndependenceStatus::SharedDependency);
}

#[test]
fn tlcx22_reframe_exact_cross_language_input_is_not_a_label_check() {
    assert!(matches!(scene(Case::MacroTextInDirectFrame),
        Err(f::Error::Parse(f::ParseCause::OutOfFrameLanguage))));
    let direct = scene(Case::UnusedMacro).unwrap();
    assert_eq!(direct.candidate_rules.unwrap().0, f::ParseRule::DirectEdgeAtomV1);
    assert_eq!(direct.candidate_path, None);
    assert_eq!(direct.report.state, ComponentState::Measured(ratio(0, 1)));
}

#[test]
fn tlcx22_reframe_exact_rename_and_same_frame_answer_repair_are_zero() {
    for case in [Case::RenameOnly, Case::SameFrameAnswerRepair] {
        let observed = scene(case).unwrap();
        // A real baseline failure plus candidate success is insufficient WITHOUT
        // semantic frame change. This isolates that gate from mere program repair.
        assert_eq!(observed.baseline[1], f::PerspectiveOutcome::KnownFailure);
        assert_eq!(observed.candidate[1], f::PerspectiveOutcome::KnownPass);
        assert_eq!(observed.baseline_profile, observed.candidate_profile);
        assert!(!observed.semantic_frame_changed);
        assert_eq!(observed.report.state, ComponentState::Measured(ratio(0, 1)));
        assert!(observed.candidate_map_work > 0);
    }
}

#[test]
fn tlcx22_reframe_exact_registry_only_or_unrelated_delta_not_gain() {
    assert!(matches!(scene(Case::RegistryOnlyInDirectFrame), Err(f::Error::FramePayloadMismatch)));
    assert!(matches!(scene(Case::WrongDeltaDeclaration), Err(f::Error::FrameDeltaMismatch)));
    assert!(matches!(scene(Case::CopiedDelta), Err(f::Error::ForeignDeltaOwner)));
    let observed = scene(Case::IdentityFrame).unwrap();
    assert_eq!(observed.report.state, ComponentState::Measured(ratio(0, 1)));
}

#[test]
fn tlcx22_reframe_exact_paid_owners_required_even_all_baseline_pass() {
    for case in [Case::MissingBaselinePaid, Case::MissingCandidatePaid, Case::AllPassMissingCandidate] {
        assert!(matches!(scene(case), Err(f::Error::MissingPaidMapOwner)));
    }
}

#[test]
fn tlcx22_reframe_exact_map_cut_unknown_cause_and_nonrefund() {
    let observed = scene(Case::TinyMapBudget).unwrap();
    assert!(observed.has_gaps);
    assert!(matches!(observed.report.state,
        ComponentState::ResourceExhausted { .. } | ComponentState::Unknown(_)));
    assert!(observed.causes.contains(&(f::Stage::PaidMap, f::Cause::WorkExhausted)));
    assert_eq!(observed.denominator, 4);
    assert_eq!(observed.recommendation, None);
    assert_eq!(observed.work_before_owner_drop, observed.work_after_owner_drop);
    assert!(observed.work_after_owner_drop > 0);
}

#[test]
fn tlcx22_reframe_exact_source_unknown_unsupported_no_comparable_not_zero() {
    assert!(matches!(scene(Case::SourceUnobserved),
        Err(f::Error::NeedsEvidence(f::Cause::SourceUnobserved))));
    assert!(matches!(scene(Case::UnsupportedProfile),
        Err(f::Error::Unsupported(f::Cause::UnsupportedProfile))));
    assert!(matches!(scene(Case::ForeignRootPlan), Err(f::Error::NoComparable(_))));
    assert!(matches!(scene(Case::RootGoalSwap), Err(f::Error::NoComparable(_))));
    let unknown = scene(Case::TinyOracleBudget).unwrap();
    assert!(unknown.has_gaps);
    assert!(!matches!(unknown.report.state, ComponentState::Measured(_)));
    assert_eq!(unknown.denominator, 4);
}

#[test]
fn tlcx22_reframe_exact_actual_source_path_state_and_goal_cuts() {
    assert!(matches!(scene(Case::ForeignSource), Err(f::Error::ForeignSourceOwner)));
    assert!(matches!(scene(Case::ForeignPathState), Err(f::Error::ForeignPathState)));
    assert!(matches!(scene(Case::MissingPath), Err(f::Error::MissingPathOwner)));
    assert!(matches!(scene(Case::WrongPathEndpoint), Err(f::Error::MapBackEndpoints)));
    assert!(matches!(scene(Case::HardErase), Err(f::Error::Context(ContextError::HardInvariant))));
    assert!(matches!(scene(Case::AuthoritySwap), Err(f::Error::Context(ContextError::AuthorityMismatch))));
}

#[test]
fn tlcx22_reframe_exact_current_cancel_expiry_and_retained_cause() {
    assert!(matches!(scene(Case::CancelAfterCallback), Err(f::Error::Resource(ResourceError::Cancelled))));
    assert!(matches!(scene(Case::ExpireBeforeCurrent), Err(f::Error::Resource(ResourceError::DeadlineExceeded))));
    let observed = scene(Case::Valid).unwrap();
    assert!(observed.causes.contains(&(f::Stage::FrameEvaluate, f::Cause::NoDirectBaseEdge)));
}

#[test]
fn tlcx22_reframe_exact_fixed_weights_and_independent_literal_parse() {
    assert!(matches!(scene(Case::DuplicateProbe), Err(f::Error::InvalidProbeSet)));
    assert!(matches!(scene(Case::ZeroWeight), Err(f::Error::InvalidWeight)));
    assert!(matches!(scene(Case::LeadingZeroDeclaration), Err(f::Error::Parse(f::ParseCause::NonCanonical))));
    assert!(matches!(scene(Case::OmittedPayloadMacro), Err(f::Error::FramePayloadMismatch)));
    let observed = scene(Case::ReverseProbeArrival).unwrap();
    assert_eq!(observed.report.state, ComponentState::Measured(ratio(3, 4)));
    assert_eq!(observed.denominator, 4);
    assert_eq!(observed.candidate_path, Some(vec![0, 1, 2]));
}

#[test]
fn tlcx22_reframe_exact_consumer_uses_same_actual_plan_and_owner_axis() {
    assert!(matches!(scene(Case::DetachedInterval), Err(f::Error::ForeignInterval)));
    assert!(matches!(scene(Case::CopiedReport), Err(f::Error::ForeignReport)));
    let observed = scene(Case::CompareActualRows).unwrap();
    assert_eq!(observed.comparison, Some(PairRelation::RightDominates));
    assert_eq!(observed.frontier, vec![CandidateId(2)]);
    assert_eq!(observed.recommendation, None);
}
