//! TEST-FIRST/API freeze; no producer implementation or successful fallback.
mod tlcx22_explain_transfer_support;
#[path = "tlcx22_explain_transfer_support/transfer_admission.rs"]
mod transfer_admission;
#[path = "tlcx22_explain_transfer_support/transfer_fixture.rs"]
mod transfer_fixture;
#[path = "tlcx22_explain_transfer_support/transfer_selection.rs"]
mod transfer_selection;
use lc631_analysis::creativity_measurement::{
    ComponentState, IndependenceStatus, InputBasis, SemanticValidation,
};
use lc631_core::{context::SourceRevision, measurement_numeric::RationalInterval};
use lc631_runtime::{explanation_measurement as e, joint_selection::*, transfer_measurement as t};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::resource::{Cancellation, Deadline, ResourceError};
use std::time::Duration;
use tlcx22_explain_transfer_support::{oracle as o, *};
use transfer_fixture as tf;

#[test]
fn tlcx22_transfer_negative_family_content_atomrename_clone_and_holdout_leakage() {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let original = tf::entries(&f);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    for cut in 0..4 {
        let mut entries = original.to_vec();
        let mut duplicate = original[0];
        duplicate.key = b"new-id";
        duplicate.role = e::DatasetRole::TransferValidation;
        if cut == 3 {
            duplicate.role = e::DatasetRole::Training;
        }
        if cut == 1 {
            duplicate.family = b"new-family-label";
        }
        if cut == 2 {
            duplicate.family = b"new-family-label";
            duplicate.theory = &f.gamma;
            duplicate.goal = &f.qr;
            duplicate.source_payload = b"different bytes but known source clone";
        }
        entries.push(duplicate);
        assert!(matches!(
            t::capture_transfer(tf::root(&cases, &entries), &view),
            Err(t::Error::SplitLeakage(_))
        ));
    }
    // Independent literal rename: source p->5,q->3,r->4, plus And/premise reorder.
    let renamed = SmtStatusQuery::checked(
        vec![
            BoolSymbolId::new(3),
            BoolSymbolId::new(4),
            BoolSymbolId::new(5),
        ],
        vec![],
        vec![atom(4), BoolExpr::And(vec![atom(3), atom(5)])],
        SmtEmitLimits::default(),
    )
    .unwrap();
    let goal = atom(5);
    let mut entries = original.to_vec();
    entries.push(t::SplitEntry {
        key: b"renamed",
        family: b"concealed-family",
        clone_family: b"concealed-clone",
        role: e::DatasetRole::TransferValidation,
        source_payload: b"new label is not new semantics",
        theory: &renamed,
        goal: &goal,
    });
    assert!(matches!(
        t::capture_transfer(tf::root(&cases, &entries), &view),
        Err(t::Error::SplitLeakage(t::SplitLeakage::CanonicalContent))
    ));
    let mut heldout = original;
    heldout[2].role = e::DatasetRole::FinalResearchHoldout;
    assert!(matches!(
        t::capture_transfer(tf::root(&cases, &heldout), &view),
        Err(t::Error::UnsupportedRole)
    ));
}

#[test]
fn tlcx22_transfer_negative_shot_update_normalizer_goal_and_budget_mismatch() {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let entries = tf::entries(&f);
    let ri = tf::root(&cases, &entries);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    for cut in 0..3 {
        let mut wrong = ri;
        if cut == 0 {
            wrong.protocol.demonstrations = 1;
        }
        if cut == 1 {
            wrong.protocol.adaptive_updates = 1;
        }
        if cut == 2 {
            wrong.normalizer_revision = d(199);
        }
        assert!(matches!(
            t::capture_transfer(wrong, &view),
            Err(t::Error::UnsupportedProtocol | t::Error::NormalizerMismatch)
        ));
    }
    let root = t::capture_transfer(ri, &view).unwrap();
    let mut wrong = tf::input(1, t::Program::ComposeV1);
    wrong.protocol.demonstrations = 1;
    assert!(matches!(
        t::admit_transfer(&root, wrong, &view),
        Err(t::Error::ProtocolMismatch)
    ));
    let mut wrong = ri;
    wrong.binding.selection.scope =
        lc631_core::context::ContextScope::declared(d(1), d(201), d(3), d(4));
    let candidate = t::admit_transfer(&root, tf::input(1, t::Program::ComposeV1), &view).unwrap();
    let gain = t::execute_transfer(&candidate, &view).unwrap();
    assert!(matches!(
        gain.report_current(
            &candidate,
            t::Expected {
                root: wrong,
                candidate: tf::input(1, t::Program::ComposeV1)
            },
            &view
        ),
        Err(t::Error::ExpectedInputMismatch)
    ));
}

#[test]
fn tlcx22_transfer_positive_actual_program_certificate_backward_handoracle_three_quarters() {
    assert!(o::literal_entails(o::T_GAMMA, o::T_QR));
    assert!(o::check(&[o::pq(), o::r()], &o::qr(), &o::t_trace()).unwrap());
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let entries = tf::entries(&f);
    let ri = tf::root(&cases, &entries);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = t::capture_transfer(ri, &view).unwrap();
    assert_eq!(root.baseline_state(0), Some(t::CaseState::Pass));
    assert_eq!(
        root.baseline_state(1),
        Some(t::CaseState::NoDerivationInThisProgram)
    );
    let ci = tf::input(1, t::Program::ComposeV1);
    let candidate = t::admit_transfer(&root, ci, &view).unwrap();
    let gain = t::execute_transfer(&candidate, &view).unwrap();
    let expected = t::Expected {
        root: ri,
        candidate: ci,
    };
    let report = gain.report_current(&candidate, expected, &view).unwrap();
    assert_eq!(
        report.state,
        ComponentState::Measured(RationalInterval::exact_ratio(3, 4).unwrap())
    );
    assert_eq!(
        (report.basis, report.validation, report.independence),
        (
            InputBasis::Given,
            SemanticValidation::Unassessed,
            IndependenceStatus::SharedDependency
        )
    );
    let trace = gain
        .candidate_certificate_current(&candidate, expected, 1, &view)
        .unwrap()
        .unwrap();
    assert_eq!(trace.len(), 4);
    assert_eq!(&trace[0].conclusion, &f.pq);
    assert!(matches!(
        &trace[1].rule,
        e::OwnedRule::AndElim {
            prior: e::StepId(0),
            child: 1
        }
    ));
    assert_eq!(&trace[2].conclusion, &f.r);
    assert_eq!(&trace[3].conclusion, &f.qr);
    assert_eq!(
        trace[3].rule,
        e::OwnedRule::AndIntro {
            priors: vec![e::StepId(1), e::StepId(2)]
        }
    );
    assert_eq!(gain.fixed_weight(), &q(4, 1));
    assert_eq!(gain.candidate_state(1), Some(t::CaseState::Pass));
}

#[test]
fn tlcx22_transfer_same_program_new_execution_is_not_same_observation_node() {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let entries = tf::entries(&f);
    let mut ri = tf::root(&cases, &entries);
    let (_br, _run, res) = run(Cancellation::default());
    let view = res.lease();
    let root = t::capture_transfer(ri, &view).unwrap();
    let ci = tf::input(1, t::Program::ExtractV1);
    let candidate = t::admit_transfer(&root, ci, &view).unwrap();
    let identity = t::execute_transfer(&candidate, &view).unwrap();
    assert_eq!(
        identity.historical_report().state,
        ComponentState::Measured(RationalInterval::exact_ratio(0, 1).unwrap())
    );
    assert!(!std::ptr::eq(
        root.baseline_node(0).unwrap(),
        identity.candidate_node(0).unwrap()
    ));
    ri.limits.max_case_work = 0;
    let unfinished_root = t::capture_transfer(ri, &view).unwrap();
    let unfinished_candidate = t::admit_transfer(&unfinished_root, ci, &view).unwrap();
    let unfinished = t::execute_transfer(&unfinished_candidate, &view).unwrap();
    assert_eq!(
        unfinished.candidate_state(0),
        Some(t::CaseState::Unknown(t::Gap::WorkLimit))
    );
    // Equal unknown intervals on distinct actual nodes use [-1,1], not Exact0.
    assert_eq!(
        unfinished.historical_report().state,
        ComponentState::Measured(RationalInterval::from_ratio(-1, 1, 1, 1).unwrap())
    );
    assert!(!std::ptr::eq(
        unfinished_root.baseline_node(0).unwrap(),
        unfinished.candidate_node(0).unwrap()
    ));
}

#[test]
fn tlcx22_transfer_current_expected_program_split_source_gamma_goal_and_nonrefund() {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let entries = tf::entries(&f);
    let mut ri = tf::root(&cases, &entries);
    ri.limits.max_run_work = 4_000_000;
    ri.limits.max_dispatches = 4;
    let ci = tf::input(1, t::Program::ComposeV1);
    let cancel = Cancellation::default();
    let (_br, _run, res) = run(cancel.clone());
    let view = res.lease();
    let root = t::capture_transfer(ri, &view).unwrap();
    let candidate = t::admit_transfer(&root, ci, &view).unwrap();
    let gain = t::execute_transfer(&candidate, &view).unwrap();
    let expected = t::Expected {
        root: ri,
        candidate: ci,
    };
    for cut in 0..6 {
        let mut wrong = expected;
        let alternate_gamma = theory(vec![f.q.clone(), f.r.clone()], 3);
        let changed_gamma = [
            t::TransferCase {
                theory: &alternate_gamma,
                ..cases[0]
            },
            cases[1],
        ];
        let changed_goal = [
            t::TransferCase {
                goal: &f.qr,
                ..cases[0]
            },
            cases[1],
        ];
        match cut {
            0 => wrong.root.binding.selection.source = SourceRevision::of("foreign source"),
            1 => wrong.root.cases = &changed_gamma,
            2 => wrong.root.cases = &changed_goal,
            3 => wrong.root.split_revision = d(202),
            4 => wrong.candidate.program = t::Program::ExtractV1,
            5 => wrong.root.limits.max_case_work = 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            gain.report_current(&candidate, wrong, &view),
            Err(t::Error::ExpectedInputMismatch)
        ));
    }
    let (_other, _other_run, other_res) = run(Cancellation::default());
    assert!(matches!(
        gain.report_current(&candidate, expected, &other_res.lease()),
        Err(t::Error::ForeignReservation)
    ));
    assert_eq!(root.work_charged_current(&view).unwrap(), 4_000_000);
    drop(gain);
    assert_eq!(root.work_charged_current(&view).unwrap(), 4_000_000);
    assert!(matches!(
        t::execute_transfer(&candidate, &view),
        Err(t::Error::WorkAdmissionExhausted)
    ));
    cancel.cancel();
    assert!(matches!(
        t::execute_transfer(&candidate, &view),
        Err(t::Error::Resource(ResourceError::Cancelled))
    ));
    let (_fresh, _fresh_run, fresh_res) = run(Cancellation::default());
    let fresh_view = fresh_res.lease();
    let fresh_root = t::capture_transfer(ri, &fresh_view).unwrap();
    let fresh_candidate = t::admit_transfer(&fresh_root, ci, &fresh_view).unwrap();
    let restored = t::execute_transfer(&fresh_candidate, &fresh_view).unwrap();
    assert!(restored
        .report_current(&fresh_candidate, expected, &fresh_view)
        .is_ok());
    assert!(matches!(
        restored.report_current(&candidate, expected, &fresh_view),
        Err(t::Error::CandidateMismatch)
    ));
}

#[test]
fn tlcx22_transfer_resource_before_traversal_and_owned_expiry() {
    let f = Fixture::transfer();
    let cases = tf::cases(&f);
    let entries = tf::entries(&f);
    let ri = tf::root(&cases, &entries);
    let (_br, _run, tiny) = run_sized(
        ri.binding.selection.source.digest(),
        Cancellation::default(),
        1,
        64 * 1024,
        Deadline::after(Duration::from_secs(120)).unwrap(),
    );
    assert!(matches!(
        t::capture_transfer(ri, &tiny.lease()),
        Err(t::Error::Resource(ResourceError::Exhausted(_)))
    ));
    let deadline = Deadline::after(Duration::from_secs(2)).unwrap();
    let (_br, _run, res) = run_sized(
        ri.binding.selection.source.digest(),
        Cancellation::default(),
        64 * 1024 * 1024,
        1024 * 1024,
        deadline,
    );
    let view = res.lease();
    let root = t::capture_transfer(ri, &view).unwrap();
    let ci = tf::input(1, t::Program::ComposeV1);
    let candidate = t::admit_transfer(&root, ci, &view).unwrap();
    let gain = t::execute_transfer(&candidate, &view).unwrap();
    std::thread::sleep(deadline.remaining().unwrap_or(Duration::ZERO) + Duration::from_millis(2));
    assert!(matches!(
        gain.report_current(
            &candidate,
            t::Expected {
                root: ri,
                candidate: ci
            },
            &view
        ),
        Err(t::Error::Resource(ResourceError::DeadlineExceeded))
    ));
}
