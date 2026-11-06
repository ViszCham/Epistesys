//! 日本語: 実公開APIの失敗admissionと単一dispatchの追加回帰。過去のRED観測ではない。
//! English: Additional actual-API regressions for failed admission and single dispatch;
//! their addition is not an observed RED. No replacement producer or measurement engine.
use lc631_analysis::creativity_measurement::RationalInterval;
use lc631_core::{
    context::{ContextRevision, ContextScope, SourceRevision},
    CandidateId,
};
use lc631_runtime::{
    formal_boolean::{Failure, UnconstrainedPolicy},
    redundancy_measurement::*,
    resource::{
        Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
        ResourceRequest, ResourceReservationView,
    },
};
use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, IntExpr, SmtEmitLimits, SmtStatusQuery};
use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};
use std::{sync::Arc, time::Duration};

// This fixture owns only actual source and lease setup/cleanup. Both test bodies
// call the production admission, evaluation and current-owner methods directly.
fn fixture(body: impl FnOnce(&SourceStore, ContextRevision, &ResourceReservationView<'_>)) {
    let source =
        SourceStore::new(Arc::from("p q r s t u v w finite regression source"), 1024).unwrap();
    let context = ContextRevision::original(SourceRevision::of(source.text()));
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            2202,
            source.id().digest(),
            limits,
            Deadline::after(Duration::from_secs(120)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let requests = [
        ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
        ),
        ResourceRequest::new(
            ResourceKind::Output,
            ResourceAmount::checked(64 * 1024).unwrap(),
        ),
    ];
    let mut reservation = run.reserve(&requests).unwrap();
    {
        let view = reservation.lease();
        body(&source, context, &view);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
    assert_eq!(broker.used(ResourceKind::Output).unwrap(), 0);
}

fn scope() -> ContextScope {
    let digest = |n| Digest32::from_bytes([n; 32]);
    ContextScope::declared(digest(1), digest(2), digest(3), digest(4))
}

#[test]
fn tlcx22_redundancy_late_unsupported_admissions_spend_the_same_root_budget() {
    fixture(|source, context, view| {
        let symbols: Vec<_> = (0..8).map(BoolSymbolId::new).collect();
        let gamma =
            SmtStatusQuery::checked(symbols.clone(), vec![], vec![], SmtEmitLimits::default())
                .unwrap();
        let atom_map: Vec<_> = symbols
            .iter()
            .enumerate()
            .map(|(i, &local)| AtomMapEntry {
                local,
                meaning: RootAtom(i as u8),
            })
            .collect();
        let meanings: Vec<_> = (0..8)
            .map(|i| AtomMeaningInput {
                atom: RootAtom(i as u8),
                grounding: SourceSpan::checked(source, i * 2, i * 2 + 1).unwrap(),
            })
            .collect();
        let model = ModelInput {
            source,
            context,
            theory: &gamma,
            atom_map: &atom_map,
            unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        };
        // Each complete predicate has 64 nodes at depth two. Fifteen complete
        // predicates precede the unsupported final unit on EVERY failed attempt.
        let expressions = vec![BoolExpr::And(vec![BoolExpr::Constant(true); 63]); 16];
        let units: Vec<_> = expressions
            .iter()
            .enumerate()
            .map(|(i, formula)| UnitInput {
                slot: SlotId(i as u16),
                formula: Some(formula),
            })
            .collect();
        let baseline = SemanticArtifactInput {
            source,
            context,
            units: &units,
        };
        let slots: Vec<_> = (0..16)
            .map(|i| SlotSpec {
                slot: SlotId(i),
                role: SemanticRole::Premise,
                protection: Protection::Mutable,
            })
            .collect();
        let input = RootInput {
            scope: scope(),
            baseline: &baseline,
            slots: &slots,
            atom_meanings: &meanings,
            model: &model,
        };
        // Fixed before admission: root envelope 65024 and each candidate 61440.
        // Two attempts fit at 187904; a third would require 249344. A 100000
        // total would reject before even one unsupported scan and be vacuous.
        let limits = Limits {
            max_total_work: 200_000,
            ..PROFILE_LIMITS
        };
        let root = RootRedundancyPlan::capture_closed(&input, limits, view).unwrap();
        let unsupported = BoolExpr::LessThan(IntExpr::Constant(0), IntExpr::Constant(1));
        let mut malformed_units = units.clone();
        malformed_units[15].formula = Some(&unsupported);
        let malformed = SemanticArtifactInput {
            source,
            context,
            units: &malformed_units,
        };

        for attempt in 0..2 {
            assert!(
                matches!(
                    BoundCandidateUnits::checked(
                        &root,
                        CandidateId(1),
                        Some(&malformed),
                        &[],
                        view
                    ),
                    Err(Error::Formal(Failure::UnsupportedInteger))
                ),
                "attempt {attempt} must reach the actual late unsupported unit"
            );
        }
        assert!(matches!(
            BoundCandidateUnits::checked(&root, CandidateId(1), Some(&malformed), &[], view),
            Err(Error::TotalWorkBudget)
        ));
        // A failed debit cannot revive the same root's spent work on retry.
        assert!(matches!(
            BoundCandidateUnits::checked(&root, CandidateId(1), Some(&malformed), &[], view),
            Err(Error::TotalWorkBudget)
        ));
    });
}

#[test]
fn tlcx22_redundancy_repeated_production_conflicts_without_changing_the_first_owner() {
    fixture(|source, context, view| {
        let symbol = BoolSymbolId::new(0);
        let gamma = SmtStatusQuery::checked(vec![symbol], vec![], vec![], SmtEmitLimits::default())
            .unwrap();
        let atom_map = [AtomMapEntry {
            local: symbol,
            meaning: RootAtom(0),
        }];
        let meanings = [AtomMeaningInput {
            atom: RootAtom(0),
            grounding: SourceSpan::checked(source, 0, 1).unwrap(),
        }];
        let model = ModelInput {
            source,
            context,
            theory: &gamma,
            atom_map: &atom_map,
            unconstrained: UnconstrainedPolicy::ExplicitlyGiven,
        };
        let p = BoolExpr::Variable(symbol);
        let units = [
            UnitInput {
                slot: SlotId(0),
                formula: Some(&p),
            },
            UnitInput {
                slot: SlotId(1),
                formula: Some(&p),
            },
        ];
        let artifact = SemanticArtifactInput {
            source,
            context,
            units: &units,
        };
        let slots = [
            SlotSpec {
                slot: SlotId(0),
                role: SemanticRole::Premise,
                protection: Protection::Mutable,
            },
            SlotSpec {
                slot: SlotId(1),
                role: SemanticRole::Premise,
                protection: Protection::Mutable,
            },
        ];
        let input = RootInput {
            scope: scope(),
            baseline: &artifact,
            slots: &slots,
            atom_meanings: &meanings,
            model: &model,
        };
        let root = RootRedundancyPlan::capture_closed(&input, PROFILE_LIMITS, view).unwrap();
        let bound = BoundCandidateUnits::checked(&root, CandidateId(1), Some(&artifact), &[], view)
            .unwrap();
        let pairs = PairPlan::prepare(&root, &bound, view).unwrap();
        let owner = OwnedRedundancy::produce(&pairs, view).unwrap();
        let state = owner.state_current(view).unwrap().clone();
        assert_eq!(
            state,
            RedundancyState::Measured(RationalInterval::exact_ratio(1, 2).unwrap())
        );
        let counts = *owner.counts_current(view).unwrap();
        assert_eq!(
            (
                counts.consistency_calls,
                counts.pair_calls,
                counts.pairs_required
            ),
            (1, 1, 1)
        );
        assert!(counts.ast_visits > 0);
        let historical = owner.historical_report().clone();
        assert_eq!(
            owner.report_for_input_current(&bound, view).unwrap(),
            &historical
        );

        assert!(matches!(
            OwnedRedundancy::produce(&pairs, view),
            Err(Error::Conflict)
        ));

        assert_eq!(owner.state_current(view).unwrap(), &state);
        assert_eq!(owner.counts_current(view).unwrap(), &counts);
        assert_eq!(owner.historical_report(), &historical);
        assert_eq!(owner.report_current(view).unwrap(), &historical);
        assert_eq!(
            owner.report_for_input_current(&bound, view).unwrap(),
            &historical
        );
    });
}
