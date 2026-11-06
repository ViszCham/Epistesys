//! 日本語: 実source/query/leaseとcurrent ownerのlifecycle fixture。測定・class判定を代用しない。
//! English: Actual source/query/lease and current-consumer lifecycle fixtures. No replacement engine.
use lc631_analysis::creativity_measurement::{
    ComponentMetric, ComponentState, IndependenceStatus, InputBasis, RationalInterval,
    SemanticValidation,
};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::CandidateId;
use lc631_runtime::formal_boolean::UnconstrainedPolicy;
use lc631_runtime::joint_selection::{AxisKind, AxisValue};
use lc631_runtime::redundancy_measurement::*;
use lc631_runtime::resource::*;
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{sync::Arc, time::Duration};
use SemanticRole::Premise;

pub fn digest(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn scope() -> ContextScope {
    ContextScope::declared(digest(1), digest(2), digest(3), digest(4))
}
pub fn variable(n: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(n))
}
pub fn not(value: BoolExpr) -> BoolExpr {
    BoolExpr::Not(Box::new(value))
}
pub fn theory(ids: &[u32], assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        ids.iter().map(|&id| BoolSymbolId::new(id)).collect(),
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
pub fn map(ids: &[u32]) -> Vec<AtomMapEntry> {
    ids.iter()
        .enumerate()
        .map(|(index, &id)| AtomMapEntry {
            local: BoolSymbolId::new(id),
            meaning: RootAtom(index as u8),
        })
        .collect()
}
pub fn meanings(source: &SourceStore, count: usize) -> Vec<AtomMeaningInput> {
    (0..count)
        .map(|index| AtomMeaningInput {
            atom: RootAtom(index as u8),
            grounding: SourceSpan::checked(source, index * 2, index * 2 + 1).unwrap(),
        })
        .collect()
}
pub fn slots(roles: &[SemanticRole]) -> Vec<SlotSpec> {
    roles
        .iter()
        .enumerate()
        .map(|(index, &role)| SlotSpec {
            slot: SlotId(index as u16),
            role,
            protection: Protection::Mutable,
        })
        .collect()
}
pub fn units(expressions: &[BoolExpr]) -> Vec<UnitInput<'_>> {
    expressions
        .iter()
        .enumerate()
        .map(|(index, formula)| UnitInput {
            slot: SlotId(index as u16),
            formula: Some(formula),
        })
        .collect()
}
pub fn artifact<'a>(
    source: &'a SourceStore,
    context: ContextRevision,
    units: &'a [UnitInput<'a>],
) -> SemanticArtifactInput<'a> {
    SemanticArtifactInput {
        source,
        context,
        units,
    }
}
pub fn model<'a>(
    source: &'a SourceStore,
    context: ContextRevision,
    theory: &'a SmtStatusQuery,
    atom_map: &'a [AtomMapEntry],
) -> ModelInput<'a> {
    ModelInput {
        source,
        context,
        theory,
        atom_map,
        unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
    }
}
pub fn root_input<'a>(
    baseline: &'a SemanticArtifactInput<'a>,
    slots: &'a [SlotSpec],
    atom_meanings: &'a [AtomMeaningInput],
    model: &'a ModelInput<'a>,
) -> RootInput<'a> {
    RootInput {
        scope: scope(),
        baseline,
        slots,
        atom_meanings,
        model,
    }
}
pub fn assert_measured(state: &RedundancyState, numerator: i64, denominator: i64) {
    assert_eq!(
        state,
        &RedundancyState::Measured(RationalInterval::exact_ratio(numerator, denominator).unwrap())
    );
}

/// Thin orchestration of actual future APIs, used only to compare budget/error
/// outcomes without prescribing which admission stage detects exhaustion.
/// It constructs no reports, classes, truth vectors or success statuses.
pub fn actual_state<'i, 'v>(
    input: &'i RootInput<'i>,
    limits: Limits,
    candidate: Option<&'i SemanticArtifactInput<'i>>,
    changes: &'i [MutableChangeInput<'i>],
    view: &'v ResourceReservationView<'_>,
) -> Result<RedundancyState, Error> {
    let root = RootRedundancyPlan::capture_closed(input, limits, view)?;
    let bound = BoundCandidateUnits::checked(&root, CandidateId(1), candidate, changes, view)?;
    let pairs = PairPlan::prepare(&root, &bound, view)?;
    let owner = OwnedRedundancy::produce(&pairs, view)?;
    Ok(owner.state_current(view)?.clone())
}

pub fn fixture(
    body: impl FnOnce(
        &SourceStore,
        ContextRevision,
        &ResourceReservationView<'_>,
        &ResourceReservationView<'_>,
        &Cancellation,
    ),
) {
    fixture_ram(8 * 1024 * 1024, body);
}
pub fn fixture_ram(
    ram: u64,
    body: impl FnOnce(
        &SourceStore,
        ContextRevision,
        &ResourceReservationView<'_>,
        &ResourceReservationView<'_>,
        &Cancellation,
    ),
) {
    let source =
        SourceStore::new(Arc::from("p q r s t u v w x finite role inventory"), 1024).unwrap();
    let context = ContextRevision::original(SourceRevision::of(source.text()));
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            22,
            source.id().digest(),
            limits,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let requests = [
        ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(ram).unwrap()),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(65536).unwrap(),
        ),
    ];
    let mut own = run.reserve(&requests).unwrap();
    let mut foreign = run.reserve(&requests).unwrap();
    {
        let view = own.lease();
        let foreign_view = foreign.lease();
        body(&source, context, &view, &foreign_view, &cancel);
    }
    own.release_confirmed().unwrap();
    foreign.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(broker.used(ResourceKind::Output).unwrap(), 0);
}

/// AST input generator from a four-row hand oracle. Expected results below
/// compare these independent bits, not production equivalence/grouping output.
pub fn function(bits: u8, ids: [u32; 2]) -> BoolExpr {
    if bits == 0 {
        return BoolExpr::Constant(false);
    }
    let mut terms = Vec::new();
    for row in 0..4 {
        if bits & (1 << row) != 0 {
            terms.push(BoolExpr::And(
                ids.iter()
                    .enumerate()
                    .map(|(bit, &id)| {
                        if row & (1 << bit) != 0 {
                            variable(id)
                        } else {
                            not(variable(id))
                        }
                    })
                    .collect(),
            ));
        }
    }
    BoolExpr::Or(terms)
}

// Actual-owner lifecycle exercises belong with their owned lease fixture.
pub fn exercise_all_pairs_and_metadata() {
    fixture_ram(
        PROFILE_LIMITS.max_retained_ram + PROFILE_LIMITS.max_scratch_ram,
        |source, context, view, _, _| {
            let gamma = theory(&[0], vec![]);
            let am = map(&[0]);
            let ms = meanings(source, 1);
            let m = model(source, context, &gamma, &am);
            let expressions = vec![variable(0); 16];
            let us = units(&expressions);
            let art = artifact(source, context, &us);
            let specs = slots(&[Premise; 16]);
            let input = root_input(&art, &specs, &ms, &m);
            let root = RootRedundancyPlan::capture_closed(&input, PROFILE_LIMITS, view).unwrap();
            let bound =
                BoundCandidateUnits::checked(&root, CandidateId(1), Some(&art), &[], view).unwrap();
            let pairs = PairPlan::prepare(&root, &bound, view).unwrap();
            let owner = OwnedRedundancy::produce(&pairs, view).unwrap();
            assert_measured(owner.state_current(view).unwrap(), 15, 16);
            let counts = owner.counts_current(view).unwrap();
            assert_eq!(
                (
                    counts.consistency_calls,
                    counts.pair_calls,
                    counts.pairs_required
                ),
                (1, 120, 120)
            );
            assert!(counts.ast_visits > 0);
            assert!(counts.charged_work <= PROFILE_LIMITS.max_total_work);
            let report = owner.report_for_input_current(&bound, view).unwrap();
            assert_eq!(
                (
                    report.metric,
                    report.basis,
                    report.validation,
                    report.independence
                ),
                (
                    ComponentMetric::Redundancy,
                    InputBasis::Given,
                    SemanticValidation::Unassessed,
                    IndependenceStatus::SharedDependency
                )
            );
            assert_eq!(
                report.state,
                ComponentState::Measured(RationalInterval::exact_ratio(15, 16).unwrap())
            );
            let axis = owner.axis_current(&bound, view).unwrap();
            assert_eq!(axis.kind, AxisKind::RedundancyCost);
            assert!(std::ptr::eq(axis.report, report));
            assert!(matches!(axis.value, AxisValue::Exact(_)));
        },
    );
}

pub fn exercise_mutable_cost_consumer() {
    fixture(|source, context, view, _, _| {
        // Γ is EMPTY: hard p is a protected payload, not an assumption forcing q/p equivalence.
        let gamma = theory(&[0, 1], vec![]);
        let am = map(&[0, 1]);
        let ms = meanings(source, 2);
        let m = model(source, context, &gamma, &am);
        let p = variable(0);
        let q = variable(1);
        let base_units = [
            UnitInput {
                slot: SlotId(0),
                formula: Some(&p),
            },
            UnitInput {
                slot: SlotId(1),
                formula: Some(&p),
            },
        ];
        let base = artifact(source, context, &base_units);
        let mut specs = slots(&[Premise; 2]);
        specs[0].protection = Protection::HardUnchanged;
        let input = root_input(&base, &specs, &ms, &m);
        let root = RootRedundancyPlan::capture_closed(&input, PROFILE_LIMITS, view).unwrap();
        let a =
            BoundCandidateUnits::checked(&root, CandidateId(1), Some(&base), &[], view).unwrap();
        let changed_units = [
            base_units[0],
            UnitInput {
                slot: SlotId(1),
                formula: Some(&q),
            },
        ];
        let changed = artifact(source, context, &changed_units);
        let delta = [MutableChangeInput {
            slot: SlotId(1),
            before: &p,
            after: &q,
        }];
        let b = BoundCandidateUnits::checked(&root, CandidateId(2), Some(&changed), &delta, view)
            .unwrap();
        let ap = PairPlan::prepare(&root, &a, view).unwrap();
        let bp = PairPlan::prepare(&root, &b, view).unwrap();
        let ao = OwnedRedundancy::produce(&ap, view).unwrap();
        let bo = OwnedRedundancy::produce(&bp, view).unwrap();
        assert_measured(ao.state_current(view).unwrap(), 1, 2);
        assert_measured(bo.state_current(view).unwrap(), 0, 1);
        assert_eq!(
            compare_current(&a, &ao, &b, &bo, view).unwrap(),
            Comparison::Higher
        );
        assert_eq!(
            compare_current(&b, &bo, &a, &ao, view).unwrap(),
            Comparison::Lower
        );
        // Other G_struct/F2 leaves are absent, not actual zero-valued producers.
        let absent = BoundCandidateUnits::checked(&root, CandidateId(3), None, &[], view).unwrap();
        let xp = PairPlan::prepare(&root, &absent, view).unwrap();
        let xo = OwnedRedundancy::produce(&xp, view).unwrap();
        assert_eq!(
            compare_current(&a, &ao, &absent, &xo, view).unwrap(),
            Comparison::NoRecommendation
        );
    });
}

pub fn exercise_current_binding_and_cancel() {
    fixture(|source, context, view, foreign, cancel| {
        let gamma = theory(&[0, 1], vec![]);
        let am = map(&[0, 1]);
        let ms = meanings(source, 2);
        let m = model(source, context, &gamma, &am);
        let p = variable(0);
        let q = variable(1);
        let au = [
            UnitInput {
                slot: SlotId(0),
                formula: Some(&p),
            },
            UnitInput {
                slot: SlotId(1),
                formula: Some(&p),
            },
        ];
        let bu = [
            UnitInput {
                slot: SlotId(0),
                formula: Some(&q),
            },
            UnitInput {
                slot: SlotId(1),
                formula: Some(&q),
            },
        ];
        let aa = artifact(source, context, &au);
        let ba = artifact(source, context, &bu);
        let specs = slots(&[Premise; 2]);
        let input = root_input(&aa, &specs, &ms, &m);
        let root = RootRedundancyPlan::capture_closed(&input, PROFILE_LIMITS, view).unwrap();
        let changes = [
            MutableChangeInput {
                slot: SlotId(0),
                before: &p,
                after: &q,
            },
            MutableChangeInput {
                slot: SlotId(1),
                before: &p,
                after: &q,
            },
        ];
        let a = BoundCandidateUnits::checked(&root, CandidateId(1), Some(&aa), &[], view).unwrap();
        let b =
            BoundCandidateUnits::checked(&root, CandidateId(1), Some(&ba), &changes, view).unwrap();
        let ap = PairPlan::prepare(&root, &a, view).unwrap();
        let bp = PairPlan::prepare(&root, &b, view).unwrap();
        let ao = OwnedRedundancy::produce(&ap, view).unwrap();
        let bo = OwnedRedundancy::produce(&bp, view).unwrap();
        assert_eq!(
            compare_current(&a, &ao, &b, &bo, view).unwrap(),
            Comparison::Equal
        );
        let ar = ao.report_for_input_current(&a, view).unwrap();
        let br = bo.report_for_input_current(&b, view).unwrap();
        assert_ne!(ar.binding.target, br.binding.target);
        assert_ne!(ar.binding.result, br.binding.result);
        assert!(matches!(
            ao.report_for_input_current(&b, view),
            Err(Error::BindingMismatch)
        ));
        assert!(matches!(
            compare_current(&b, &ao, &b, &bo, view),
            Err(Error::BindingMismatch)
        ));
        assert!(matches!(
            ao.axis_current(&a, foreign),
            Err(Error::ForeignReservation)
        ));
        assert!(matches!(
            compare_current(&a, &ao, &b, &bo, foreign),
            Err(Error::ForeignReservation)
        ));
        let historical = ao.historical_report().clone();
        cancel.cancel();
        assert_eq!(view.check(), Err(ResourceError::Cancelled));
        assert!(matches!(
            ao.report_current(view),
            Err(Error::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            ao.counts_current(view),
            Err(Error::Resource(ResourceError::Cancelled))
        ));
        assert!(matches!(
            compare_current(&a, &ao, &b, &bo, view),
            Err(Error::Resource(ResourceError::Cancelled))
        ));
        assert_eq!(ao.historical_report(), &historical);
    });
}

/// Same native root/run across fresh plans: successful work is not refundable
/// when an ephemeral pair plan/owner is dropped. No production-derived budget.
pub fn exercise_root_budget_reuse() {
    fixture(|source, context, view, _, _| {
        let ids = [0, 1, 2, 3, 4, 5, 6, 7];
        let gamma = theory(&ids, vec![]);
        let am = map(&ids);
        let ms = meanings(source, 8);
        let m = model(source, context, &gamma, &am);
        // Constants avoid per-variable map lookup: 2*64*(1+256)=32896 visits/pair.
        let expressions = [
            BoolExpr::And(vec![BoolExpr::Constant(true); 63]),
            BoolExpr::And(vec![BoolExpr::Constant(true); 63]),
        ];
        let us = units(&expressions);
        let art = artifact(source, context, &us);
        let specs = slots(&[Premise; 2]);
        let input = root_input(&art, &specs, &ms, &m);
        let limits = Limits {
            max_total_work: 100_000,
            ..PROFILE_LIMITS
        };
        let root = RootRedundancyPlan::capture_closed(&input, limits, view).unwrap();
        let bound =
            BoundCandidateUnits::checked(&root, CandidateId(1), Some(&art), &[], view).unwrap();
        let mut completed = 0;
        let mut exhausted = false;
        for _ in 0..4 {
            let pairs = match PairPlan::prepare(&root, &bound, view) {
                Ok(pairs) => pairs,
                Err(Error::TotalWorkBudget) => {
                    exhausted = true;
                    break;
                }
                Err(error) => panic!("unexpected preparation failure: {error:?}"),
            };
            let owner = match OwnedRedundancy::produce(&pairs, view) {
                Ok(owner) => owner,
                Err(Error::TotalWorkBudget) => {
                    exhausted = true;
                    break;
                }
                Err(error) => panic!("unexpected evaluation failure: {error:?}"),
            };
            match owner.state_current(view).unwrap() {
                RedundancyState::Measured(_) => {
                    assert_measured(owner.state_current(view).unwrap(), 1, 2);
                    let counts = owner.counts_current(view).unwrap();
                    assert_eq!(counts.pair_calls, 1);
                    assert!(counts.ast_visits >= 32_896);
                    completed += 1;
                }
                RedundancyState::ResourceExhausted => {
                    exhausted = true;
                    break;
                }
                other => panic!("unexpected state: {other:?}"),
            }
        }
        assert!(
            completed > 0,
            "the first individually bounded call must succeed"
        );
        assert!(
            exhausted,
            "four actual pairs cannot fit the same 100000-work root budget"
        );
    });
}
