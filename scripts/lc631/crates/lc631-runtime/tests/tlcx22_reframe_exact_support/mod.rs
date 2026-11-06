//! Given finite fixtures/literal gold and resource scaffolding; no local oracle.
// Shared fixture surface: individual integration targets use different subsets.
#![allow(dead_code)]
pub use lc631_analysis::creativity_measurement::{
    ComponentReport, ComponentState, IndependenceStatus, InputBasis, RationalInterval,
    SemanticValidation,
};
pub use lc631_core::{context::*, AuthorityRevision, CandidateId};
pub use lc631_runtime::{
    contextual_distillation::{consumer::*, *},
    joint_selection::{
        project_component_reported, AxisValue, PairRelation, ReportedComponentAxis, SelectionPins,
    },
    reorganization_measurement::reframe as f,
    resource::*,
    structural_gain_measurement::{ClosedGraph, Edge, Vertex},
};
pub use lc631_substrate::{
    source::{SourceSpan, SourceStore},
    Digest32,
};
pub use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub const DIRECT_BASE: &str = "frame-v2:direct;name=base";
pub const DIRECT_CANDIDATE: &str = "frame-v2:direct;name=candidate";
pub const MACRO_BASE: &str = "frame-v2:path;name=base;0,2=0,1,2";
pub const MACRO_CANDIDATE: &str = "frame-v2:path;name=candidate;0,2=0,1,2";
// These bytes are authored independently, not produced by canonical_statement.
pub const EDGE_01: &str = "edge(0,1)";
pub const EDGE_02: &str = "edge(0,2)";
pub const MACRO_02: &str = "macro(0,2)";
pub const ROOT_GOAL: Edge = Edge(Vertex(0), Vertex(2));
pub fn d(n: u8) -> Digest32 {
    Digest32::from_bytes([n; 32])
}
pub fn ratio(n: i64, den: i64) -> RationalInterval {
    RationalInterval::exact_ratio(n, den).unwrap()
}
pub fn relation(a: u8, b: u8) -> RelationEdit {
    RelationEdit {
        from: RelationNode(a),
        to: RelationNode(b),
    }
}
pub fn state(context: ContextRevision, scope: ContextScope) -> ContextualDistillation {
    let basis = ImmutableRelationBasis::checked(
        context,
        scope,
        3,
        &[relation(0, 1), relation(1, 2)],
        BasisAvailability::Given,
    )
    .unwrap();
    let mut state = ContextualDistillation::new(
        basis,
        DistillationLimits {
            max_epochs: 2,
            max_proposals: 2,
            max_relations: 4,
        },
    )
    .unwrap();
    state
        .epoch(&[RelationProposal {
            context,
            scope,
            edit: relation(0, 2),
        }])
        .unwrap();
    state
}
pub fn path(state: &ContextualDistillation) -> DependencyPath<'_> {
    let request = NextRelationQuery {
        context: state.basis().context(),
        scope: state.basis().scope(),
        state_revision: state.state_revision(),
        basis_digest: state.basis().digest(),
        relation: relation(0, 2),
        basis: ValidationBasis::GivenFiniteReachability,
    };
    consume_dependency_path(revalidate_next_query(state, request, 64).unwrap()).unwrap()
}
pub fn resource(kind: ResourceKind, bytes: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(bytes).unwrap())
}
pub struct Observation {
    pub report: ComponentReport,
    pub baseline: Vec<f::PerspectiveOutcome>,
    pub candidate: Vec<f::PerspectiveOutcome>,
    pub baseline_profile: f::FrameProfile,
    pub candidate_profile: f::FrameProfile,
    pub candidate_rules: Option<(f::ParseRule, f::LowerRule, f::EvalRule)>,
    pub candidate_path: Option<Vec<u16>>,
    pub candidate_map_work: u64,
    pub semantic_frame_changed: bool,
    pub same_oracle_and_budget: bool,
    pub root_before: bool,
    pub root_after: bool,
    pub denominator: u64,
    pub has_gaps: bool,
    pub causes: Vec<(f::Stage, f::Cause)>,
    pub work_before_owner_drop: u64,
    pub work_after_owner_drop: u64,
    pub comparison: Option<PairRelation>,
    pub frontier: Vec<CandidateId>,
    pub recommendation: Option<CandidateId>,
}
// Fixture orchestration only: every stage is the future real API. No AST,
// evaluator, paid-map receipt or measurement is implemented in this macro.
#[macro_export]
macro_rules! exact_paid {
    ($plan:expr, $binding:expr, $terms:expr, $id:expr, $view:expr) => {{
        let parsed = f::ParsedFrameProgram::parse_exact($plan, $binding, $terms, $id, $view)?;
        let lowered = f::LoweredFrameProgram::lower_current(parsed, $view)?;
        let evaluated = f::EvaluatedFrameProgram::evaluate_current(lowered, $view)?;
        f::PaidFrameMap::execute_current(evaluated, $view)
    }};
}
