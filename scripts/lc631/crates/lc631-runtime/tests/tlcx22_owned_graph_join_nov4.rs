//! 日本語: 実graph/decoder ownerを結ぶ負例を先に固定する。原式完成ではない。
//! English: Negative-first actual graph/decoder join; no original-score closure.
use lc631_analysis::creativity_measurement::{ComponentState, Rational, RationalInterval};
use lc631_core::{context::{ContextRevision, ContextScope, SourceRevision}, CandidateId};
use lc631_runtime::{
    description_measurement::{self as description, ReconstructionInput},
    facet_ledger::{ComparisonScope, Revision, ScopeId, TransformId},
    joint_selection::{PairRelation, SelectionPins},
    reconstruction_measurement::{self as reconstruction, FeatureKind, ProbeSpec, ProtectedFeatureSet},
    resource::*,
    structural_gain_measurement::{
        owned_graph_join::{compare_current, OwnedGraphJoin},
        CandidateBinding, ClosedGraph, Correspondence, Edge, OwnedPreservation, PreservationPlan,
        ProtectedEdge, Vertex,
    },
};
use lc631_substrate::{semantic_term::{self, Term, TermLimits}, source::SourceStore, Digest32};
use std::{sync::Arc, time::Duration};

fn d(n: u8) -> Digest32 { Digest32::from_bytes([n; 32]) }
fn edge(a: u16, b: u16) -> Edge { Edge(Vertex(a), Vertex(b)) }
// Independent fixture representation: actual typed K6 values, not reports/digests.
fn term(edges: &[(u16, u16)]) -> Vec<u8> {
    semantic_term::encode(&Term::Apply("closed-directed-graph-v1".into(), vec![
        Term::Integer(3.into()),
        Term::Apply("edges-v1".into(), edges.iter().map(|&(a,b)|
            Term::Apply("edge-v1".into(), vec![Term::Integer(a.into()), Term::Integer(b.into())])
        ).collect()),
    ]), TermLimits::default()).unwrap()
}
fn fixture(body: impl FnOnce(&SourceStore, ContextRevision, &ResourceReservationView<'_>, &Cancellation)) {
    let source = SourceStore::new(Arc::from("Given finite graph and exact K6 correspondence"), 1024).unwrap();
    let context = ContextRevision::original(SourceRevision::of(source.text()));
    let cap = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let cancel = Cancellation::default();
    let run = broker.begin_run(22, source.id().digest(), cap,
        Deadline::after(Duration::from_secs(60)).unwrap(), cancel.clone()).unwrap();
    let reservation = run.reserve(&[
        ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(112*1024*1024).unwrap()),
        ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(32*1024*1024).unwrap()),
    ]).unwrap();
    let view = reservation.lease();
    body(&source, context, &view, &cancel);
}

#[test]
fn tlcx22_owned_graph_join_nov4_rejects_same_source_wrong_term_and_retains_actual_owners() {
    fixture(|source, context, view, cancel| {
        let scope = ContextScope::declared(d(1), d(3), d(4), d(5));
        let pins = SelectionPins { source: context.source(), scope, baseline:d(2),
            hard_invariants:d(6), policy:d(7), goal_profile:None, evidence_revision:d(9) };
        // The final edge is intentionally NOT protected: correspondence must
        // inspect the full actual graph, independently of preservation probes.
        let edges = [edge(0,1),edge(1,2),edge(2,0)];
        let target_edges = [edge(0,1),edge(2,0)];
        let graph = ClosedGraph::checked(source,context,3,&edges).unwrap();
        let target = ClosedGraph::checked(source,context,3,&target_edges).unwrap();
        let probes = [ProtectedEdge{edge:edges[0],weight:1},ProtectedEdge{edge:edges[1],weight:3}];
        let mapping = [Correspondence{source:Vertex(0),target:Vertex(0)},
            Correspondence{source:Vertex(1),target:Vertex(1)},Correspondence{source:Vertex(2),target:Vertex(2)}];
        let plan = PreservationPlan::declared(&graph,scope,3,&probes,Some(&mapping)).unwrap();
        let binding = CandidateBinding{id:CandidateId(1),identity:d(21),context,pins};
        let preserve = OwnedPreservation::produce(&plan,Some(&target),binding,view).unwrap();
        let source_bytes = term(&[(0,1),(1,2),(2,0)]);
        let target_bytes = term(&[(0,1),(2,0)]);
        // Same protected answers, counts and K6 length; only an unprotected
        // actual edge differs. Probe-only and numeric-only joins must reject.
        let wrong_bytes = term(&[(0,1),(2,1)]);
        let term_scope = ComparisonScope{id:ScopeId([1;32]),baseline:Revision([2;32]),
            root_goal:Revision([3;32]),probes:Revision([4;32]),policy:Revision([5;32])};
        let specs = [ProbeSpec{path:vec![],kind:FeatureKind::Operator,weight:Rational::from_integer(1.into())}];
        let term_plan = ProtectedFeatureSet::capture(&source_bytes,&specs,context,term_scope,view).unwrap();
        let baseline = reconstruction::produce(&term_plan,&source_bytes,view).unwrap();
        let candidate = reconstruction::produce(&term_plan,&target_bytes,view).unwrap();
        let wrong = reconstruction::produce(&term_plan,&wrong_bytes,view).unwrap();
        let input = |owner,target,n| ReconstructionInput{owner,plan:&term_plan,target,context,transform:TransformId([n;32])};
        let root = description::RootPlan::capture(input(&baseline,&source_bytes,22),16,view).unwrap();
        let description = description::produce(&root,input(&candidate,&target_bytes,23),view).unwrap();
        let wrong_description = description::produce(&root,input(&wrong,&wrong_bytes,24),view).unwrap();
        assert!(OwnedGraphJoin::capture(&preserve,&wrong_description,&source_bytes,&wrong_bytes,view).is_err());
        assert!(OwnedGraphJoin::capture(&preserve,&description,&source_bytes,&wrong_bytes,view).is_err());
        let extra_edge = term(&[(0,1),(1,0),(2,0)]);
        assert!(OwnedGraphJoin::capture(&preserve,&description,&source_bytes,&extra_edge,view).is_err());
        let mut trailing = target_bytes.clone();
        trailing.push(0);
        assert!(OwnedGraphJoin::capture(&preserve,&description,&source_bytes,&trailing,view).is_err());
        let mut wrong_nodes = semantic_term::decode(&target_bytes,TermLimits::default()).unwrap();
        if let Term::Apply(_,fields) = &mut wrong_nodes { fields[0] = Term::Integer(4.into()); }
        let wrong_nodes = semantic_term::encode(&wrong_nodes,TermLimits::default()).unwrap();
        assert!(OwnedGraphJoin::capture(&preserve,&description,&source_bytes,&wrong_nodes,view).is_err());
        let original_p = preserve.report_current(view).unwrap().clone();
        let original_k = description.report_current(view).unwrap().clone();
        let joined = OwnedGraphJoin::capture(&preserve,&description,&source_bytes,&target_bytes,view).unwrap();
        let current = joined.current(view).unwrap();
        assert!(std::ptr::eq(current.preservation(),preserve.report_current(view).unwrap()));
        assert!(std::ptr::eq(current.description(),description.report_current(view).unwrap()));
        assert_ne!(current.binding(),original_p.binding.result);
        assert_ne!(current.binding(),original_k.binding.result);
        assert_eq!(current.preservation().state,ComponentState::Measured(RationalInterval::exact_ratio(1,4).unwrap()));
        assert_eq!(preserve.report_current(view).unwrap(),&original_p);
        assert_eq!(description.report_current(view).unwrap(),&original_k);
        assert_eq!(compare_current(&joined,&joined,view).unwrap(),PairRelation::EquivalentInScope);
        // Actual second candidate restores the protected edge under the SAME
        // fixed plan. Its measured Preserve rises from1/4 to1; K is retained
        // as a prerequisite/lineage component, never added as a Pareto gain.
        let full_preserve = OwnedPreservation::produce(&plan,Some(&graph),
            CandidateBinding{id:CandidateId(2),identity:d(22),context,pins},view).unwrap();
        let full_description = description::produce(&root,input(&baseline,&source_bytes,25),view).unwrap();
        let full_join = OwnedGraphJoin::capture(&full_preserve,&full_description,
            &source_bytes,&source_bytes,view).unwrap();
        assert_eq!(compare_current(&joined,&full_join,view).unwrap(),PairRelation::RightDominates);
        assert_eq!(compare_current(&full_join,&joined,view).unwrap(),PairRelation::LeftDominates);
        // Reusing valid term/owner from the other candidate is a cut, not a
        // restoration: actual target graph correspondence must fail.
        assert!(OwnedGraphJoin::capture(&preserve,&full_description,
            &source_bytes,&source_bytes,view).is_err());
        let missing = OwnedPreservation::produce(&plan,None,binding,view).unwrap();
        assert!(OwnedGraphJoin::capture(&missing,&description,&source_bytes,&target_bytes,view).is_err());
        let partial_plan = PreservationPlan::declared(&graph,scope,3,&probes,Some(&mapping[..2])).unwrap();
        let partial = OwnedPreservation::produce(&partial_plan,Some(&target),binding,view).unwrap();
        assert!(OwnedGraphJoin::capture(&partial,&description,&source_bytes,&target_bytes,view).is_err());
        // A new actual root with equal bytes and numbers can form its OWN join;
        // it cannot silently substitute for the fixed root of this comparison.
        let separate_root = description::RootPlan::capture(input(&baseline,&source_bytes,22),16,view).unwrap();
        let separate_description = description::produce(&separate_root,input(&candidate,&target_bytes,23),view).unwrap();
        let separate = OwnedGraphJoin::capture(&preserve,&separate_description,&source_bytes,&target_bytes,view).unwrap();
        assert!(compare_current(&joined,&separate,view).is_err());
        fixture(|_,same_context,foreign,_| {
            assert_eq!(same_context,context);
            assert!(joined.current(foreign).is_err());
            assert!(compare_current(&joined,&joined,foreign).is_err());
        });
        cancel.cancel();
        assert!(joined.current(view).is_err());
        assert!(compare_current(&joined,&joined,view).is_err());
    });
}
