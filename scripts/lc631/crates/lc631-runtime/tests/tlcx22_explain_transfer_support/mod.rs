//! Test-owned literal fixtures only. No production checker/program/score shim.
#![allow(dead_code)]
pub mod oracle;
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    measurement_numeric::Rational,
    CandidateId,
};
use lc631_runtime::{
    explanation_measurement as e,
    joint_selection::SelectionPins,
    resource::{Reservation, ResourceBroker, ResourceLimits, RunResources},
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    resource::{Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest},
    Digest32,
};
use std::time::Duration;
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn q(n: i64, den: i64) -> Rational {
    Rational::new(n.into(), den.into())
}
pub fn atom(n: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(n))
}
pub fn theory(assertions: Vec<BoolExpr>, atoms: u32) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        (0..atoms).map(BoolSymbolId::new).collect(),
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
pub fn binding(role: e::DatasetRole) -> e::Binding {
    let source = SourceRevision::of("Given ConjunctionCertificateV1 / transfer-v1 fixed source");
    e::Binding {
        selection: SelectionPins {
            source,
            scope: ContextScope::declared(d(1), d(2), d(3), d(4)),
            baseline: d(5),
            hard_invariants: d(6),
            policy: d(7),
            goal_profile: Some(d(8)),
            evidence_revision: d(9),
        },
        context: ContextRevision::original(source),
        suite: d(10),
        oracle: d(11),
        definition: e::definition_revision(),
        probe_set: d(3),
        dependency_group: d(12),
        role,
    }
}
pub fn candidate(n: u8) -> e::CandidateBinding {
    e::CandidateBinding {
        id: CandidateId(u64::from(n)),
        candidate_digest: d(20 + n),
        context: binding(e::DatasetRole::ExplanationValidation).context,
        target: d(40 + n),
        transform: d(60 + n),
    }
}
pub fn limits() -> e::Limits {
    e::Limits {
        max_case_work: 1_000_000,
        max_run_work: 16_000_000,
        max_dispatches: 16,
        max_split_work: 8_000_000,
    }
}
pub struct Fixture {
    pub gamma: SmtStatusQuery,
    pub p: BoolExpr,
    pub q: BoolExpr,
    pub r: BoolExpr,
    pub pq: BoolExpr,
    pub qr: BoolExpr,
    pub one: Rational,
    pub three: Rational,
}
impl Fixture {
    pub fn explain() -> Self {
        Self::make(false)
    }
    pub fn transfer() -> Self {
        Self::make(true)
    }
    fn make(transfer: bool) -> Self {
        let pq = BoolExpr::And(vec![atom(0), atom(1)]);
        let mut assertions = vec![pq.clone()];
        if transfer {
            assertions.push(atom(2));
        }
        Self {
            gamma: theory(assertions, 3),
            p: atom(0),
            q: atom(1),
            r: atom(2),
            pq,
            qr: BoolExpr::And(vec![atom(1), atom(2)]),
            one: q(1, 1),
            three: q(3, 1),
        }
    }
    pub fn projection(&self, goal_q: bool, wrong: bool) -> Vec<e::Step<'_>> {
        let correct = u8::from(goal_q);
        vec![
            e::Step {
                conclusion: &self.pq,
                rule: e::Rule::Premise { index: 0 },
            },
            e::Step {
                conclusion: if goal_q { &self.q } else { &self.p },
                rule: e::Rule::AndElim {
                    prior: e::StepId(0),
                    child: if wrong { 1 - correct } else { correct },
                },
            },
        ]
    }
    pub fn case<'a>(
        &'a self,
        key: &'a [u8],
        goal: &'a BoolExpr,
        steps: &'a [e::Step<'a>],
    ) -> e::ExplainCase<'a> {
        e::ExplainCase {
            key,
            theory: &self.gamma,
            goal,
            weight: &self.one,
            baseline: e::Submission::Certificate(e::Certificate { steps }),
        }
    }
}
pub fn root_input<'a>(cases: &'a [e::ExplainCase<'a>]) -> e::ExplainRootInput<'a> {
    e::ExplainRootInput {
        binding: binding(e::DatasetRole::ExplanationValidation),
        cases,
        limits: limits(),
    }
}
pub fn response<'a>(key: &'a [u8], steps: &'a [e::Step<'a>]) -> e::ExplainResponse<'a> {
    e::ExplainResponse {
        key,
        submission: e::Submission::Certificate(e::Certificate { steps }),
        prose: b"",
    }
}
pub fn input<'a>(n: u8, responses: &'a [e::ExplainResponse<'a>]) -> e::ExplainCandidateInput<'a> {
    e::ExplainCandidateInput {
        binding: candidate(n),
        responses,
    }
}
pub fn run(cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    run_sized(
        binding(e::DatasetRole::ExplanationValidation)
            .selection
            .source
            .digest(),
        cancel,
        64 * 1024 * 1024,
        1024 * 1024,
        Deadline::after(Duration::from_secs(120)).unwrap(),
    )
}
pub fn run_sized(
    source: Digest32,
    cancel: Cancellation,
    ram: u64,
    output: u64,
    deadline: Deadline,
) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(1, source, limits, deadline, cancel)
        .unwrap();
    let req = |kind, n| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap());
    let reservation = run
        .reserve(&[
            req(ResourceKind::Ram, ram),
            req(ResourceKind::Output, output),
        ])
        .unwrap();
    (broker, run, reservation)
}
