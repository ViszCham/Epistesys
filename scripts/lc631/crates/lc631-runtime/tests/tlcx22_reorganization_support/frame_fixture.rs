//! Future-frame orchestration with actual core contexts and checked dependency paths.
//! No local explanation oracle, measurement kernel, fake certificate or success stub.
#![allow(dead_code)]
pub use lc631_analysis::creativity_measurement::{
    ComponentMetric,ComponentReport,ComponentState,InputBasis,SemanticValidation,IndependenceStatus,
    RationalInterval,
};
pub use lc631_core::context::ContextError;
use lc631_core::{context::*,AuthorityRevision,CandidateId};
use lc631_runtime::{contextual_distillation::{consumer::*,*},resource::*,
    structural_gain_measurement::{ClosedGraph,Edge,Vertex},
    joint_selection::SelectionPins};
pub use lc631_runtime::reorganization_measurement::reframe as f;
use lc631_substrate::{source::SourceStore,Digest32};
use std::{sync::Arc,time::Duration};
#[derive(Clone,Copy)]
pub enum Case { CheckedMacro,MissingPath,WrongEndpoint,TinyOracleBudget,MissingBaselineOwner,
    UnusedMacro,IdentityFrame,LabelOnlyDelta,UnregisteredMacroRule,OracleSwap,
    HardErase,AuthoritySwap,GoalScopeSwap }
pub fn ratio(n:i64,den:i64)->RationalInterval { RationalInterval::exact_ratio(n,den).unwrap() }
fn d(n:u8)->Digest32 { Digest32::from_bytes([n;32]) }
pub struct Observed {
    pub report:ComponentReport,
    pub baseline:Vec<f::PerspectiveOutcome>,
    pub candidate:Vec<f::PerspectiveOutcome>,
    pub used_base_paths:Vec<Vec<u16>>,
    pub denominator:u64,
    pub same_oracle_and_budget:bool,
    pub root_reachability_before:bool,
    pub root_reachability_after:bool,
    pub has_gaps:bool,
}
pub fn scenario(case:Case)->Result<Observed,f::Error> {
    const DIRECT:&str="frame-v1:direct";
    let text=format!("{DIRECT}\nbase edges 0->1,1->2; root reach(0,2)");
    let source=SourceStore::new(Arc::from(text),1024).unwrap();
    let scope=ContextScope::declared(d(1),d(2),d(3),d(4));
    let original=OriginalContext::checked(SourceRevision::of(source.text()),scope,
        AuthorityRevision(7),vec![
            ContextPremise::reported(d(10),Arc::from(DIRECT),PremiseBasis::Given,true),
            ContextPremise::reported(d(11),Arc::from("immutable base and root reach(0,2)"),
                PremiseBasis::Given,false),
        ],ContextLimits {max_contexts:4,max_premises:2,max_edits:1,max_statement_bytes:1024}).unwrap();
    let parent=original.state();
    let edges=[Edge(Vertex(0),Vertex(1)),Edge(Vertex(1),Vertex(2))];
    let base=ClosedGraph::checked(&source,parent.revision(),3,&edges).unwrap();
    let cap=ResourceLimits::uniform(8*1024*1024).unwrap();
    let broker=ResourceBroker::new(cap).unwrap(); let cancel=Cancellation::default();
    let run=broker.begin_run(23,source.id().digest(),cap,
        Deadline::after(Duration::from_secs(60)).unwrap(),cancel).unwrap();
    let reservation=run.reserve(&[ResourceRequest::new(ResourceKind::Ram,
        ResourceAmount::checked(4*1024*1024).unwrap())]).unwrap();
    let view=reservation.lease();
    let rules=if matches!(case,Case::UnregisteredMacroRule) {
        &[f::FrameRule::DirectEdge][..]
    } else { &[f::FrameRule::DirectEdge,f::FrameRule::ExpandCheckedBasePath][..] };
    // Family/oracle frozen BEFORE plan capture, macro creation and frame Delta.
    let family=f::FrameFamily::given(&base,rules,
        f::OracleId::ExpandOneOuterStepToBasePathV1,&view)?;
    let direct=f::FramePayload::Direct;
    let perspectives=[
        f::PerspectiveProbe {edge:edges[0],weight:1},
        f::PerspectiveProbe {edge:Edge(Vertex(0),Vertex(2)),weight:3},
    ];
    let pins=SelectionPins {source:parent.revision().source(),scope,baseline:d(5),
        hard_invariants:d(6),policy:d(7),goal_profile:None,evidence_revision:d(9)};
    let mut bounds=f::Limits::finite_path_v1();
    if matches!(case,Case::TinyOracleBudget) { bounds.max_check_work=1; }
    let plan=f::ReframePlan::capture_fixed(&source,&original,&base,&family,
        if matches!(case,Case::MissingBaselineOwner) {None} else {Some(&direct)},
        &perspectives,pins,bounds,&view)?;
    // Root captures the actual graph and fixed oracle before this candidate exists.
    let basis=ImmutableRelationBasis::checked(parent.revision(),scope,3,
        &[RelationEdit {from:RelationNode(0),to:RelationNode(1)},
          RelationEdit {from:RelationNode(1),to:RelationNode(2)}],BasisAvailability::Given).unwrap();
    let mut state=ContextualDistillation::new(basis,
        DistillationLimits {max_epochs:2,max_proposals:2,max_relations:4}).unwrap();
    let request=state.epoch(&[RelationProposal {context:parent.revision(),scope,
        edit:RelationEdit {from:RelationNode(0),to:RelationNode(2)}}])
        .unwrap().next_query.unwrap();
    let path=consume_dependency_path(revalidate_next_query(&state,request,64).unwrap()).unwrap();
    let macros=[f::MacroInput {
        edge:if matches!(case,Case::WrongEndpoint) {edges[0]} else {Edge(Vertex(0),Vertex(2))},
        path:if matches!(case,Case::MissingPath) {None} else {Some(&path)},
    }];
    let payload=if matches!(case,Case::IdentityFrame) {f::FramePayload::Direct}
        else {f::FramePayload::PathMacros(&macros)};
    let statement=if matches!(case,Case::LabelOnlyDelta) {
        Arc::from("frame-label-only")
    } else { Arc::from(payload.canonical_statement()?) };
    let edits=if matches!(case,Case::HardErase) {vec![PremiseEdit::Retract {id:d(11)}]}
        else {vec![PremiseEdit::Replace {id:d(10),statement,basis:PremiseBasis::Proposed}]};
    let delta=ContextDelta::proposed(parent.revision(),
        if matches!(case,Case::GoalScopeSwap) {ContextScope::declared(d(1),d(99),d(3),d(4))} else {scope},
        if matches!(case,Case::AuthoritySwap) {AuthorityRevision(8)} else {AuthorityRevision(7)},
        ContextApplicability::Unconditional,edits);
    let mut contexts=ContextConsumer::new(original.clone()).unwrap();
    let admitted=contexts.admit(delta).map_err(f::Error::Context)?;
    let revision=match admitted {ContextAdmission::Inserted(r)|ContextAdmission::Duplicate(r)=>r};
    let child=contexts.get(revision).unwrap();
    let checked=f::CheckedFrameMap::produce(&plan,parent,child,
        child.declared_delta().unwrap(),&payload,&state,&view)?;
    let program=f::PreparedFrameProgram::prepare(&plan,&checked,
        if matches!(case,Case::UnusedMacro) {f::ExplanationProgram::DirectOnly}
            else {f::ExplanationProgram::ResolveFrameQueries},
        if matches!(case,Case::OracleSwap) {f::OracleId::DirectEdgeOnlyV1}
            else {f::OracleId::ExpandOneOuterStepToBasePathV1},
        CandidateId(2),&view)?;
    let owner=f::OwnedReframe::evaluate(&program,&view)?;
    owner.with_current(&view,|current| Observed {
        report:current.report().clone(),baseline:current.baseline_results().to_vec(),
        candidate:current.candidate_results().to_vec(),
        used_base_paths:current.used_base_paths().map(|p|p.to_vec()).collect(),
        denominator:plan.denominator(),same_oracle_and_budget:current.same_oracle_and_budget(),
        root_reachability_before:current.root_reachability_before(),
        root_reachability_after:current.root_reachability_after(),has_gaps:current.has_gaps(),
    })
}
