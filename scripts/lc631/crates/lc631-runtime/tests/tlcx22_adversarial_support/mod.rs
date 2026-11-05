#![allow(dead_code)]
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    measurement_numeric::Rational,
    CandidateId,
};
use lc631_runtime::adversarial_measurement::*;
use lc631_runtime::joint_selection::SelectionPins;
use lc631_runtime::resource::{Reservation, ResourceBroker, ResourceLimits, RunResources};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    resource::{Cancellation, Deadline, ResourceAmount, ResourceKind, ResourceRequest},
    Digest32,
};
use std::time::Duration;
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn q(n: i64, d: i64) -> Rational {
    Rational::new(n.into(), d.into())
}
pub fn binding() -> CatalogBinding {
    let source = SourceRevision::of("fixed adversarial question");
    CatalogBinding {
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
        attack_scope: d(11),
        oracle: d(12),
        definition: definition_revision(),
        probe_set: d(3),
        dependency_group: d(13),
        purpose: CasePurpose::SoftFiniteValidation,
        quantification: Quantification::AllAdmissibleAssignments,
    }
}
pub fn candidate(id: u64) -> CandidateBinding {
    CandidateBinding {
        id: CandidateId(id),
        candidate_digest: d(20 + id as u8),
        context: binding().context,
        target: d(30),
        transform: d(31),
    }
}
pub fn v(n: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(n))
}
pub fn theory(ids: &[u32], assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        ids.iter().map(|n| BoolSymbolId::new(*n)).collect(),
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
pub fn limits() -> AttackLimits {
    AttackLimits {
        registration_ast_visits: 1_000_000,
        max_ast_visits: 1_000_000,
        max_trials: 8,
    }
}
pub fn run(cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    let br = ResourceBroker::new(ResourceLimits::uniform(256 * 1024 * 1024).unwrap()).unwrap();
    let run = br
        .begin_run(
            1,
            binding().selection.source.digest(),
            ResourceLimits::uniform(128 * 1024 * 1024).unwrap(),
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel,
        )
        .unwrap();
    let res = run
        .reserve(&[
            req(ResourceKind::Ram, 128 * 1024 * 1024),
            req(ResourceKind::Output, 16 * 1024 * 1024),
        ])
        .unwrap();
    (br, run, res)
}
pub fn req(k: ResourceKind, n: u64) -> ResourceRequest {
    ResourceRequest::new(k, ResourceAmount::checked(n).unwrap())
}
