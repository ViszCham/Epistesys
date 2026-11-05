//! Test-owned native inputs and literal oracles; no measurement implementation.
#![allow(dead_code)]
use super::*;
pub const SOURCE: &str = "p and q\np";
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn q(n: i64, den: i64) -> Rational {
    Rational::new(n.into(), den.into())
}
pub fn var(n: u32) -> BoolExpr {
    BoolExpr::Variable(BoolSymbolId::new(n))
}
pub fn query(a: u32, b: u32, assertions: Vec<BoolExpr>) -> SmtStatusQuery {
    SmtStatusQuery::checked(
        vec![BoolSymbolId::new(a), BoolSymbolId::new(b)],
        vec![],
        assertions,
        SmtEmitLimits::default(),
    )
    .unwrap()
}
pub fn dm(a: u32, b: u32) -> BoolExpr {
    BoolExpr::Not(Box::new(BoolExpr::Or(vec![
        BoolExpr::Not(Box::new(var(a))),
        BoolExpr::Not(Box::new(var(b))),
    ])))
}
pub fn weak(a: u32, b: u32) -> BoolExpr {
    BoolExpr::Or(vec![BoolExpr::And(vec![var(a), var(b)]), var(a)])
}
pub fn original(source: &SourceStore) -> OriginalContext {
    OriginalContext::checked(
        SourceRevision::of(source.text()),
        ContextScope::declared(d(1), d(2), d(3), d(4)),
        AuthorityRevision(7),
        vec![
            ContextPremise::from_source(
                d(5),
                source,
                SourceSpan::checked(source, 0, 7).unwrap(),
                PremiseBasis::Given,
                true,
            )
            .unwrap(),
            ContextPremise::from_source(
                d(6),
                source,
                SourceSpan::checked(source, 8, 9).unwrap(),
                PremiseBasis::Given,
                false,
            )
            .unwrap(),
        ],
        ContextLimits {
            max_contexts: 17,
            max_premises: 2,
            max_edits: 1,
            max_statement_bytes: 8192,
        },
    )
    .unwrap()
}
pub fn bridge_limits() -> BridgeLimits {
    BridgeLimits {
        max_candidates: 16,
        max_ast_nodes: 128,
        max_validation_checks: 100_000,
        max_payload_bytes: 8192,
    }
}
pub fn limits() -> n::Limits {
    n::Limits {
        max_atoms: 8,
        max_assertions: 16,
        max_features: 256,
        max_archive_entries: 64,
        max_ast_nodes: 512,
        max_depth: 32,
        max_payload_bytes: 1_048_576,
        max_call_work: 1_000_000,
        max_total_work: 8_000_000,
        max_archive_comparisons: 64,
        max_weight_bits: 1024,
        max_intermediate_bits: 4096,
        retained_ram: 4 * 1024 * 1024,
        scratch_ram: 8 * 1024 * 1024,
        output_bytes: 65536,
    }
}
pub fn resource(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
pub fn assert_point(report: &ComponentReport, num: i64, den: i64) {
    assert_eq!(
        report.state,
        ComponentState::Measured(RationalInterval::exact_ratio(num, den).unwrap())
    );
    assert_eq!(report.basis, InputBasis::Given);
    assert_eq!(report.validation, SemanticValidation::Unassessed);
    assert_eq!(report.independence, IndependenceStatus::SharedDependency);
}
#[derive(Clone, Copy)]
pub enum ArchiveMode {
    Baseline,
    ExactWeak,
    CommuteDeMorgan,
    Empty,
    Unread,
    Unavailable,
    Unsupported,
}

// Literal independent two-atom truth tables in row order (p,q)=00,01,10,11.
// Neither the production evaluator nor candidate mutable Γ computes this oracle.
pub const AND_TABLE: [bool; 4] = [false, false, false, true];
pub const P_TABLE: [bool; 4] = [false, false, true, true];

#[macro_export]
macro_rules! ctx_fixture {
    ($name:ident,$mode:expr,$limits:expr,$rename:expr,$body:block) => {
        ctx_fixture!(
            $name,
            $mode,
            $limits,
            $rename,
            Duration::from_secs(60),
            $body
        );
    };
    ($name:ident,$mode:expr,$limits:expr,$rename:expr,$duration:expr,$body:block) => {
        let source = SourceStore::new(Arc::from(SOURCE), 256 * 1024).unwrap();
        let (a, b) = if $rename { (7, 9) } else { (0, 1) };
        let theory = query(a, b, vec![BoolExpr::And(vec![var(a), var(b)]), var(a)]);
        let goal = query(a, b, vec![var(a)]);
        let premises = [
            GivenPremise {
                id: d(5),
                span: SourceSpan::checked(&source, 0, 7).unwrap(),
                formula: &theory.assertions()[0],
            },
            GivenPremise {
                id: d(6),
                span: SourceSpan::checked(&source, 8, 9).unwrap(),
                formula: &theory.assertions()[1],
            },
        ];
        let atoms = [
            n::AtomMap {
                local: BoolSymbolId::new(a),
                meaning: 0,
                grounding: SourceSpan::checked(&source, 0, 1).unwrap(),
            },
            n::AtomMap {
                local: BoolSymbolId::new(b),
                meaning: 1,
                grounding: SourceSpan::checked(&source, 6, 7).unwrap(),
            },
        ];
        let one = q(1, 1);
        let p = var(a);
        let weak_formula = weak(a, b);
        let dm_formula = dm(a, b);
        let facts = [
            n::FactFeature {
                formula: &theory.assertions()[0],
                weight: &one,
            },
            n::FactFeature {
                formula: &p,
                weight: &one,
            },
        ];
        let constructions = [
            n::ConstructionFeature {
                formula: &theory.assertions()[0],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &p,
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &weak_formula,
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &dm_formula,
                weight: &one,
            },
        ];
        let weak_archive = query(a, b, vec![p.clone(), weak_formula.clone()]);
        let dm_archive = query(a, b, vec![p.clone(), dm(b, a)]);
        let unsupported_archive = query(
            a,
            b,
            vec![BoolExpr::Equal(IntExpr::Constant(0), IntExpr::Constant(0))],
        );
        let mode = $mode;
        let context = original(&source).state().revision();
        let entries = [
            n::ArchiveEntry {
                source: &source,
                context,
                query: match mode {
                    ArchiveMode::CommuteDeMorgan => &dm_archive,
                    ArchiveMode::Unsupported => &unsupported_archive,
                    ArchiveMode::ExactWeak => &weak_archive,
                    _ => &theory,
                },
            },
            n::ArchiveEntry {
                source: &source,
                context,
                query: if matches!(mode, ArchiveMode::ExactWeak) {
                    &theory
                } else {
                    &weak_archive
                },
            },
        ];
        let archive = match mode {
            ArchiveMode::Empty => n::ArchiveInput::Read(&entries[..0]),
            ArchiveMode::Unread => n::ArchiveInput::Unread,
            ArchiveMode::Unavailable => n::ArchiveInput::Unavailable,
            ArchiveMode::ExactWeak => n::ArchiveInput::Read(&entries),
            _ => n::ArchiveInput::Read(&entries[..1]),
        };
        let broker =
            ResourceBroker::new(ResourceLimits::uniform(128 * 1024 * 1024).unwrap()).unwrap();
        let cancel = Cancellation::default();
        let deadline = Deadline::after($duration).unwrap();
        let run = broker
            .begin_run(
                1,
                source.id().digest(),
                ResourceLimits::uniform(128 * 1024 * 1024).unwrap(),
                deadline,
                cancel.clone(),
            )
            .unwrap();
        let reservation = run
            .reserve(&[
                resource(ResourceKind::Ram, 96 * 1024 * 1024),
                resource(ResourceKind::Output, 4 * 1024 * 1024),
            ])
            .unwrap();
        let view = reservation.lease();
        let input = n::RootInput {
            source: &source,
            original: original(&source),
            source_query: &theory,
            premises: &premises,
            target: d(5),
            goal: GoalPolicy {
                revision: d(2),
                query: &goal,
            },
            bridge_limits: bridge_limits(),
        };
        let declaration = n::Declaration {
            atoms: &atoms,
            facts: &facts,
            constructions: &constructions,
            archive,
            corpus_revision: d(70),
            oracle_revision: d(71),
            policy_revision: d(72),
            limits: $limits,
        };
        let mut root = n::RootPlan::capture_current(input, declaration, &view).unwrap();
        let demorgan = root
            .generate_current(Operator::ReframeDeMorgan, &view)
            .unwrap();
        let weakened = root
            .generate_current(
                Operator::Recombine {
                    partner: d(6),
                    connective: Connective::Or,
                },
                &view,
            )
            .unwrap();
        let $name = (
            &source,
            &theory,
            &goal,
            &premises,
            &atoms,
            &facts,
            &constructions,
            archive,
            &root,
            &demorgan,
            &weakened,
            &view,
            &cancel,
            &run,
        );
        $body
    };
}
