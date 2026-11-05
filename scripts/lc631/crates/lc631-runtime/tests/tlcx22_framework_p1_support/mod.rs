//! Given inputs and literal finite oracles only; no framework implementation.
#![allow(dead_code)]
use super::f;
// P1 admission target uses this metric; the current-only target shares this support.
#[allow(unused_imports)]
pub use lc631_analysis::creativity_measurement::ComponentMetric;
pub use lc631_analysis::creativity_measurement::{
    ComponentReport, ComponentState, IndependenceStatus, InputBasis, Rational, RationalInterval,
    SemanticValidation,
};
pub use lc631_core::context::{
    ContextLimits, ContextPremise, ContextScope, OriginalContext, PremiseBasis, SourceRevision,
};
pub use lc631_core::AuthorityRevision;
pub use lc631_runtime::{
    context_novelty_measurement as n,
    creative_candidate_bridge::{BridgeLimits, Connective, GivenPremise, GoalPolicy, Operator},
    resource::*,
};
pub use lc631_solvers::typed_query::{BoolExpr, BoolSymbolId, SmtEmitLimits, SmtStatusQuery};
pub use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};
pub use std::{cell::Cell, sync::Arc, time::Duration};
#[path = "../tlcx22_context_novelty_support/mod.rs"]
mod base;
pub use base::{bridge_limits, d, limits as context_limits, original, q, resource, var, weak};
pub const SOURCE: &str = base::SOURCE;

pub fn scope() -> ContextScope {
    ContextScope::declared(d(1), d(2), d(3), d(4))
}
pub fn operator() -> Operator {
    Operator::Recombine {
        partner: d(6),
        connective: Connective::Or,
    }
}
pub fn limits() -> f::Limits {
    f::Limits {
        max_atoms: 3,
        max_models: 64,
        max_cases: 4,
        max_queries: 32,
        max_lowered_nodes: 128,
        max_native_dispatches_per_case: 2048,
        max_current_checks: 16,
        plan_ram: 2 * 1024 * 1024,
        case_ram: 2 * 1024 * 1024,
        scratch_ram: 8 * 1024 * 1024,
        output_bytes: 65536,
    }
}
pub const PROFILES: [f::ProfileRegistration<'static>; 2] = [
    f::ProfileRegistration {
        kernel: f::Kernel::ClassicalPointV1,
        contract: f::Contract::ClassicalPointV1,
        display_name: "classical",
    },
    f::ProfileRegistration {
        kernel: f::Kernel::PersistentChain2V1,
        contract: f::Contract::PersistentChain2V1,
        display_name: "persistent",
    },
];
pub const OPERATORS: [f::OperatorMap; 5] = [
    f::OperatorMap {
        source: f::GrammarOperator::Constant,
        target: f::NativeOperator::Constant,
    },
    f::OperatorMap {
        source: f::GrammarOperator::Variable,
        target: f::NativeOperator::Atom,
    },
    f::OperatorMap {
        source: f::GrammarOperator::Not,
        target: f::NativeOperator::ProfiledNot,
    },
    f::OperatorMap {
        source: f::GrammarOperator::And,
        target: f::NativeOperator::And,
    },
    f::OperatorMap {
        source: f::GrammarOperator::Or,
        target: f::NativeOperator::Or,
    },
];
pub const ROLES: [f::RoleBinding; 20] = [
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::AtomForcing,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::Bottom,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::Top,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::And,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::Or,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::PositiveRules,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::Domain,
        package: f::Package::Kc,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::Negation,
        package: f::Package::Kc,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::Consequence,
        package: f::Package::Kc,
    },
    f::RoleBinding {
        kernel: f::Kernel::ClassicalPointV1,
        role: f::SignatureRole::AxiomSuite,
        package: f::Package::Kc,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::AtomForcing,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::Bottom,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::Top,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::And,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::Or,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::PositiveRules,
        package: f::Package::C,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::Domain,
        package: f::Package::Ki,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::Negation,
        package: f::Package::Ki,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::Consequence,
        package: f::Package::Ki,
    },
    f::RoleBinding {
        kernel: f::Kernel::PersistentChain2V1,
        role: f::SignatureRole::AxiomSuite,
        package: f::Package::Ki,
    },
];

pub struct Inputs<'a> {
    pub source: &'a SourceStore,
    pub theory: &'a SmtStatusQuery,
    pub goal: &'a SmtStatusQuery,
    pub premises: &'a [GivenPremise<'a>],
    pub atoms: &'a [n::AtomMap],
    pub facts: &'a [n::FactFeature<'a>],
    pub constructions: &'a [n::ConstructionFeature<'a>],
    pub framework_atoms: &'a [f::AtomCorrespondence],
    pub probes: &'a [f::Probe<'a>],
    pub universe: &'a [f::PackageWeight<'a>],
}
impl<'a> Inputs<'a> {
    pub fn native<'v>(&self, view: &'v ResourceReservationView<'_>) -> n::RootPlan<'a, 'v> {
        n::RootPlan::capture_current(
            n::RootInput {
                source: self.source,
                original: original(self.source),
                source_query: self.theory,
                premises: self.premises,
                target: d(5),
                goal: GoalPolicy {
                    revision: d(2),
                    query: self.goal,
                },
                bridge_limits: bridge_limits(),
            },
            n::Declaration {
                atoms: self.atoms,
                facts: self.facts,
                constructions: self.constructions,
                archive: n::ArchiveInput::Unread,
                corpus_revision: d(70),
                oracle_revision: d(71),
                policy_revision: d(72),
                limits: context_limits(),
            },
            view,
        )
        .unwrap()
    }
    pub fn declaration(&self) -> f::Declaration<'a> {
        f::Declaration {
            root_scope: scope(),
            root_goal_revision: d(2),
            authority_revision: AuthorityRevision(7),
            profiles: &PROFILES,
            partition: &ROLES,
            universe: self.universe,
            partition_policy: f::PartitionPolicy::GivenTestOnly6_1_1,
            map: f::MapDeclaration {
                atoms: self.framework_atoms,
                operators: &OPERATORS,
                point_world: 0,
                chain_root: 0,
                chain_future: 1,
                rule: f::MapRule::ConstantEmbeddingRootRetractionV1,
            },
            probes: self.probes,
            allowed_operator: operator(),
            comparison: f::CellPair {
                source: f::CellKey {
                    kernel: f::Kernel::ClassicalPointV1,
                    subject: f::SubjectKind::Original,
                },
                target: f::CellKey {
                    kernel: f::Kernel::PersistentChain2V1,
                    subject: f::SubjectKind::Generated,
                },
                attribution: f::Attribution::JointContextAndFramework,
            },
            definition_revision: d(80),
            map_revision: d(81),
            limits: limits(),
        }
    }
}

// Independent literal rows: q-state00,01,11 => q bits0,2,3; not-q3,0,0; LEM3,2,3.
pub const CHAIN_Q: [u64; 3] = [0, 2, 3];
pub const CHAIN_NOT_Q: [u64; 3] = [3, 0, 0];
pub const CHAIN_LEM: [u64; 3] = [3, 2, 3];
pub const CLASSICAL_LEM: [u64; 2] = [1, 1];
pub const ADMITTED: [&[usize]; 4] = [&[3], &[2, 3], &[8], &[6, 7, 8]];
pub const CELLS: [f::CellKey; 4] = [
    f::CellKey {
        kernel: f::Kernel::ClassicalPointV1,
        subject: f::SubjectKind::Original,
    },
    f::CellKey {
        kernel: f::Kernel::ClassicalPointV1,
        subject: f::SubjectKind::Generated,
    },
    f::CellKey {
        kernel: f::Kernel::PersistentChain2V1,
        subject: f::SubjectKind::Original,
    },
    f::CellKey {
        kernel: f::Kernel::PersistentChain2V1,
        subject: f::SubjectKind::Generated,
    },
];

#[macro_export]
macro_rules! framework_fixture {
    ($name:ident,$count:expr,$rename:expr,$duration:expr,$body:block) => {
        let source = SourceStore::new(Arc::from(SOURCE), 262144).unwrap();
        let ids: Vec<_> = (if $rename {
            [7, 9, 11, 13]
        } else {
            [0, 1, 2, 3]
        })
        .into_iter()
        .take($count)
        .map(BoolSymbolId::new)
        .collect();
        let (a, b) = (ids[0].get(), ids[1].get());
        let theory = SmtStatusQuery::checked(
            ids.clone(),
            vec![],
            vec![BoolExpr::And(vec![var(a), var(b)]), var(a)],
            SmtEmitLimits::default(),
        )
        .unwrap();
        let goal =
            SmtStatusQuery::checked(ids.clone(), vec![], vec![var(a)], SmtEmitLimits::default())
                .unwrap();
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
        let atoms: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| n::AtomMap {
                local: *id,
                meaning: i as u8,
                grounding: SourceSpan::checked(&source, 0, 1).unwrap(),
            })
            .collect();
        // Extra atoms use explicit Given grounding; this is not NLP extraction truth.
        let framework_atoms: Vec<_> = (0..ids.len())
            .map(|i| f::AtomCorrespondence {
                meaning: i as u8,
                point_slot: i as u8,
                chain_slot: i as u8,
            })
            .collect();
        let one = q(1, 1);
        let six = q(6, 1);
        let weak_formula = weak(a, b);
        let facts = [
            n::FactFeature {
                formula: &theory.assertions()[0],
                weight: &one,
            },
            n::FactFeature {
                formula: &goal.assertions()[0],
                weight: &one,
            },
        ];
        let constructions = [
            n::ConstructionFeature {
                formula: &theory.assertions()[0],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &goal.assertions()[0],
                weight: &one,
            },
            n::ConstructionFeature {
                formula: &weak_formula,
                weight: &one,
            },
        ];
        let q_expr = var(b);
        let lem = BoolExpr::Or(vec![var(b), BoolExpr::Not(Box::new(var(b)))]);
        let probes = [
            f::Probe {
                role: f::QueryRole::GoalEcho,
                formula: &goal.assertions()[0],
                duty: f::Duty::Optional,
            },
            f::Probe {
                role: f::QueryRole::Placement,
                formula: &q_expr,
                duty: f::Duty::Optional,
            },
            f::Probe {
                role: f::QueryRole::BridgeLaw,
                formula: &lem,
                duty: f::Duty::Optional,
            },
        ];
        let universe = [
            f::PackageWeight {
                package: f::Package::C,
                weight: &six,
            },
            f::PackageWeight {
                package: f::Package::Kc,
                weight: &one,
            },
            f::PackageWeight {
                package: f::Package::Ki,
                weight: &one,
            },
        ];
        let broker =
            ResourceBroker::new(ResourceLimits::uniform(128 * 1024 * 1024).unwrap()).unwrap();
        let cancel = Cancellation::default();
        let run = broker
            .begin_run(
                2212,
                source.id().digest(),
                ResourceLimits::uniform(128 * 1024 * 1024).unwrap(),
                Deadline::after($duration).unwrap(),
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
        let $name = (
            Inputs {
                source: &source,
                theory: &theory,
                goal: &goal,
                premises: &premises,
                atoms: &atoms,
                facts: &facts,
                constructions: &constructions,
                framework_atoms: &framework_atoms,
                probes: &probes,
                universe: &universe,
            },
            &view,
            &cancel,
            &run,
        );
        $body
    };
}
