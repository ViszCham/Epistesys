//! Actual core context/path fixtures plus future parser/lowering/eval/map calls.
use super::support::*;
use crate::exact_paid;

#[derive(Clone, Copy, PartialEq)]
pub enum Case {
    Valid, MacroTextInDirectFrame, RenameOnly, SameFrameAnswerRepair, UnusedMacro,
    RegistryOnlyInDirectFrame, IdentityFrame, WrongDeltaDeclaration, CopiedDelta,
    MissingBaselinePaid, MissingCandidatePaid, AllPassMissingCandidate,
    TinyMapBudget, TinyOracleBudget, SourceUnobserved, UnsupportedProfile,
    ForeignRootPlan, RootGoalSwap, ForeignSource, ForeignPathState, MissingPath,
    WrongPathEndpoint, HardErase, AuthoritySwap, CancelAfterCallback,
    ExpireBeforeCurrent, DuplicateProbe, ZeroWeight, LeadingZeroDeclaration,
    OmittedPayloadMacro, ReverseProbeArrival, DetachedInterval, CopiedReport,
    CompareActualRows, ContextScopeSwap, UnregisteredFrameRule,
}
pub fn scene(case: Case) -> Result<Observation, f::Error> {
    let baseline_is_macro = matches!(case, Case::RenameOnly | Case::SameFrameAnswerRepair);
    let baseline_declaration = if baseline_is_macro { MACRO_BASE } else { DIRECT_BASE };
    let text = format!("{baseline_declaration}\nimmutable base; root goal is a typed Given input");
    let source = SourceStore::new(Arc::from(text), 4096).unwrap();
    let foreign_source = SourceStore::new(Arc::from(source.text()), 4096).unwrap();
    let scope = ContextScope::declared(d(1), d(2), d(3), d(4));
    let original = OriginalContext::checked(SourceRevision::of(source.text()), scope, AuthorityRevision(7),
        vec![
            ContextPremise::from_source(d(10), &source,
                SourceSpan::checked(&source, 0, baseline_declaration.len()).unwrap(),
                PremiseBasis::Given, true).unwrap(),
            ContextPremise::from_source(d(11), &source,
                SourceSpan::checked(&source, baseline_declaration.len()+1, source.text().len()).unwrap(),
                PremiseBasis::Given, false).unwrap(),
        ], ContextLimits { max_contexts:4, max_premises:2, max_edits:1, max_statement_bytes:4096 }).unwrap();
    let parent = original.state();
    let edges = [Edge(Vertex(0), Vertex(1)), Edge(Vertex(1), Vertex(2))];
    let base = ClosedGraph::checked(&source, parent.revision(), 3, &edges).unwrap();
    let cap = ResourceLimits::uniform(16*1024*1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let ttl = if case==Case::ExpireBeforeCurrent { Duration::from_secs(2) } else { Duration::from_secs(60) };
    let run = broker.begin_run(93, source.id().digest(), cap, Deadline::after(ttl).unwrap(), cancel.clone()).unwrap();
    let reservation = run.reserve(&[resource(ResourceKind::Ram, 8*1024*1024)]).unwrap();
    let view = reservation.lease();
    let profiles = [f::FrameProfile::DirectEdgeV1, f::FrameProfile::CheckedPathMacroV1];
    let registered_profiles = if case == Case::UnregisteredFrameRule { &profiles[..1] } else { &profiles[..] };
    let family = f::FrameFamily::register_exact(&base, registered_profiles,
        f::OracleId::ExpandOneOuterStepToBasePathV1, &view)?;
    // Both states are actual complete basis owners. The second is a foreign
    // owner despite equal payloads; it must not substitute for the expected one.
    let state = state(parent.revision(), scope);
    let other_state = super::support::state(parent.revision(), scope);
    let actual_path = path(&state);
    let foreign_path = path(&other_state);
    let macro_edge = if case==Case::WrongPathEndpoint { edges[0] } else { ROOT_GOAL };
    let macro_input = [f::MacroInput { edge:macro_edge,
        path:if case==Case::MissingPath {None}
            else if case==Case::ForeignPathState {Some(&foreign_path)} else {Some(&actual_path)} }];
    let baseline_frame = f::FrameSpec {
        profile:if baseline_is_macro {profiles[1]} else {profiles[0]},
        declaration:baseline_declaration,
        macros:if baseline_is_macro {&macro_input[..]} else {&[]},
    };
    let mut perspectives = [
        f::PerspectiveQuery { id:f::ProbeId(0), edge:edges[0], weight:1 },
        f::PerspectiveQuery { id:f::ProbeId(1), edge:ROOT_GOAL, weight:3 },
    ];
    if case==Case::DuplicateProbe { perspectives[1].id=perspectives[0].id; }
    if case==Case::ZeroWeight { perspectives[1].weight=0; }
    if case==Case::ReverseProbeArrival { perspectives.reverse(); }
    let baseline_terms = [
        f::QueryText { query:f::ProbeId(0), text:EDGE_01 },
        f::QueryText { query:f::ProbeId(1), text:EDGE_02 },
    ];
    let active_queries = if case==Case::AllPassMissingCandidate {&perspectives[..1]} else {&perspectives[..]};
    let active_baseline_terms = if case==Case::AllPassMissingCandidate {&baseline_terms[..1]} else {&baseline_terms[..]};
    let root_goal = f::RootGoal::Reachability(ROOT_GOAL);
    let other_goal = f::RootGoal::Reachability(Edge(Vertex(1), Vertex(2)));
    let pins = SelectionPins { source:parent.revision().source(), scope, baseline:d(5),
        hard_invariants:d(6), policy:d(7), goal_profile:None, evidence_revision:d(9) };
    let mut limits = f::ExactLimits::finite_path_v2();
    if case==Case::TinyMapBudget { limits.max_map_work=1; }
    if case==Case::TinyOracleBudget { limits.max_query_work=1; }
    let input = f::ExactRootInput {
        source:if case==Case::ForeignSource {&foreign_source} else {&source},
        original:&original, base:&base, family:&family, state:&state,
        root_goal:&root_goal, frame_premise:d(10), baseline_frame:&baseline_frame,
        baseline_terms:active_baseline_terms, perspectives:active_queries, pins,
        source_basis:if case==Case::SourceUnobserved {f::SourceBasis::Unobserved} else {f::SourceBasis::Given},
    };
    let plan = f::ReframePlan::capture_exact(input, limits, &view)?;
    let foreign_plan = f::ReframePlan::capture_exact(f::ExactRootInput {
        root_goal:if case==Case::RootGoalSwap {&other_goal} else {&root_goal}, ..input
    }, limits, &view)?;
    let candidate_plan = if matches!(case, Case::ForeignRootPlan | Case::RootGoalSwap) {&foreign_plan} else {&plan};
    let direct_candidate = matches!(case, Case::IdentityFrame | Case::MacroTextInDirectFrame | Case::RegistryOnlyInDirectFrame);
    let candidate_declaration = if case==Case::SameFrameAnswerRepair {MACRO_BASE}
        else if case==Case::LeadingZeroDeclaration {"frame-v2:path;name=candidate;00,2=0,1,2"}
        else if case==Case::WrongPathEndpoint {"frame-v2:path;name=candidate;0,1=0,1,2"}
        else if case==Case::OmittedPayloadMacro {"frame-v2:path;name=candidate"}
        else if direct_candidate {DIRECT_CANDIDATE} else {MACRO_CANDIDATE};
    let candidate_frame = f::FrameSpec {
        profile:if case==Case::UnsupportedProfile {f::FrameProfile::Unregistered(90)}
            else if direct_candidate {profiles[0]} else {profiles[1]},
        declaration:candidate_declaration,
        macros:if direct_candidate && case!=Case::RegistryOnlyInDirectFrame {&[]}
            else {&macro_input[..]},
    };
    let delta_statement = if case==Case::WrongDeltaDeclaration {DIRECT_CANDIDATE} else {candidate_declaration};
    let edits = if case==Case::HardErase {vec![PremiseEdit::Retract {id:d(11)}]}
        else {vec![PremiseEdit::Replace {id:d(10),statement:Arc::from(delta_statement),basis:PremiseBasis::Proposed}]};
    let candidate_scope = if case == Case::ContextScopeSwap {
        ContextScope::declared(d(1), d(99), d(3), d(4))
    } else { scope };
    let delta = ContextDelta::proposed(parent.revision(), candidate_scope,
        if case==Case::AuthoritySwap {AuthorityRevision(8)} else {AuthorityRevision(7)},
        ContextApplicability::Unconditional, edits);
    let zero_delta = ContextDelta::proposed(parent.revision(), scope, AuthorityRevision(7),
        ContextApplicability::Unconditional,
        vec![PremiseEdit::Replace {id:d(10),statement:Arc::from(DIRECT_CANDIDATE),basis:PremiseBasis::Proposed}]);
    let mut contexts = ContextConsumer::new(original.clone()).unwrap();
    let admitted = contexts.admit(delta).map_err(f::Error::Context)?;
    let child_revision = match admitted {ContextAdmission::Inserted(r)|ContextAdmission::Duplicate(r)=>r};
    let zero_admitted = contexts.admit(zero_delta).map_err(f::Error::Context)?;
    let zero_revision = match zero_admitted {ContextAdmission::Inserted(r)|ContextAdmission::Duplicate(r)=>r};
    let child = contexts.get(child_revision).unwrap();
    let zero_child = contexts.get(zero_revision).unwrap();
    let copied_delta = child.declared_delta().unwrap().clone();
    let baseline_binding = f::CheckedFrameMap::baseline_current(&plan, &view)?;
    let candidate_binding = f::CheckedFrameMap::produce_exact(candidate_plan, f::FrameTransitionInput {
        parent, child, delta:if case==Case::CopiedDelta {&copied_delta} else {child.declared_delta().unwrap()},
        frame:&candidate_frame, state:&state,
    }, &view)?;
    let candidate_terms = [
        f::QueryText {query:f::ProbeId(0),text:EDGE_01},
        f::QueryText {query:f::ProbeId(1),text:if case==Case::UnusedMacro
            || (direct_candidate && case!=Case::MacroTextInDirectFrame) {EDGE_02} else {MACRO_02}},
    ];
    let active_candidate_terms = if case==Case::AllPassMissingCandidate {&candidate_terms[..1]} else {&candidate_terms[..]};
    let baseline_paid = if case==Case::MissingBaselinePaid {None} else {
        Some(exact_paid!(&plan,&baseline_binding,active_baseline_terms,CandidateId(1),&view)?)
    };
    let candidate_paid = if matches!(case, Case::MissingCandidatePaid | Case::AllPassMissingCandidate) {None} else {
        Some(exact_paid!(candidate_plan,&candidate_binding,active_candidate_terms,CandidateId(2),&view)?)
    };
    let owner = f::OwnedReframe::measure_pair_current(&plan,
        f::PairInput {baseline:baseline_paid.as_ref(),candidate:candidate_paid.as_ref()}, &view)?;
    if case==Case::ExpireBeforeCurrent {
        let start=Instant::now();
        while view.check().is_ok() {
            assert!(start.elapsed()<Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    let mut observed = owner.with_current(&view, |current| {
        let mut comparison=None; let mut frontier=Vec::new();
        let candidate= candidate_paid.as_ref().unwrap();
        let baseline= baseline_paid.as_ref().unwrap();
        if matches!(case,Case::DetachedInterval|Case::CopiedReport) {
            let actual=project_component_reported(current.report()).unwrap();
            if case==Case::DetachedInterval {
                let equal=ratio(3,4);
                let detached=ReportedComponentAxis {kind:actual.kind,value:AxisValue::Exact(&equal),report:current.report()};
                current.bind_axis(&plan,baseline,candidate,&detached)?;
            } else {
                let copied=current.report().clone();
                let detached=project_component_reported(&copied).unwrap();
                current.bind_axis(&plan,baseline,candidate,&detached)?;
            }
        }
        if case==Case::CompareActualRows {
            let zero_frame=f::FrameSpec {profile:profiles[0],declaration:DIRECT_CANDIDATE,macros:&[]};
            let zero_binding=f::CheckedFrameMap::produce_exact(&plan,f::FrameTransitionInput {
                parent,child:zero_child,delta:zero_child.declared_delta().unwrap(),frame:&zero_frame,state:&state,
            },&view)?;
            let zero_paid=exact_paid!(&plan,&zero_binding,active_baseline_terms,CandidateId(3),&view)?;
            let zero=f::OwnedReframe::measure_pair_current(&plan,
                f::PairInput {baseline:Some(baseline),candidate:Some(&zero_paid)},&view)?;
            zero.with_current(&view,|zc| {
                let za=project_component_reported(zc.report()).unwrap();
                let pa=project_component_reported(current.report()).unwrap();
                let zr=zc.bind_axis(&plan,baseline,&zero_paid,&za)?;
                let pr=current.bind_axis(&plan,baseline,candidate,&pa)?;
                comparison=Some(f::compare_current(&zr,&pr,&view)?);
                let selected=f::select_current(&[zr,pr],&view)?;
                frontier=selected.frontier().to_vec();
                assert_eq!(selected.recommendation(),None);
                Ok::<(),f::Error>(())
            })??;
        }
        let trace=candidate.trace_for(f::ProbeId(1));
        let observation=Observation {
            report:current.report().clone(),baseline:current.baseline_results().to_vec(),
            candidate:current.candidate_results().to_vec(),baseline_profile:baseline.profile(),
            candidate_profile:candidate.profile(),
            candidate_rules:trace.map(|t|(t.parse_rule(),t.lower_rule(),t.eval_rule())),
            candidate_path:current.used_base_path(f::ProbeId(1)).map(|p|p.to_vec()),
            candidate_map_work:candidate.mapping_work_spent(),semantic_frame_changed:current.semantic_frame_changed(),
            same_oracle_and_budget:current.same_oracle_and_budget(),
            root_before:current.root_reachability_before(),root_after:current.root_reachability_after(),
            denominator:plan.denominator(),has_gaps:current.has_gaps(),
            causes:current.causes().iter().map(|c|(c.stage(),c.cause())).collect(),
            work_before_owner_drop:0,work_after_owner_drop:0,comparison,frontier,
            recommendation:current.recommendation(),
        };
        if case==Case::CancelAfterCallback {cancel.cancel();}
        Ok::<Observation,f::Error>(observation)
    })??;
    observed.work_before_owner_drop=plan.work_spent();
    drop(owner);
    observed.work_after_owner_drop=plan.work_spent();
    Ok(observed)
}
