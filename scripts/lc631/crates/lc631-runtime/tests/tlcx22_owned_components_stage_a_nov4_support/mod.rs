//! Actual finite producer fixtures; no detached ComponentReport constructor.
use super::{adversarial_support as a, design_support as dsg, memory_support as mem, prediction_support as pred};
use lc631_analysis::creativity_measurement::{
    formula::OriginalFormula, CandidateSubject, ComponentMetric as M, ComponentState,
    Rational, RationalInterval,
};
use lc631_core::{context::{ContextRevision, ContextScope, SourceRevision}, CandidateId};
use lc631_runtime::{
    adversarial_measurement as risk, description_measurement as desc,
    design_measurement as design, facet_ledger::*, joint_selection::SelectionPins,
    memory_measurement as memory,
    original_score_pending::owned_components::{CollectionPlan, ComponentOwnerRef, OwnedComponentCollection},
    prediction_measurement as prediction, reconstruction_measurement as recon,
    resource::*, structural_gain_measurement as structure,
};
use lc631_solvers::{temporal::Truth, typed_query::BoolExpr};
use lc631_substrate::{source::SourceStore, Digest32};
use std::{sync::Arc, time::Duration};

fn pin(n: u8) -> Digest32 { Digest32::from_bytes([n;32]) }
fn run(source: Digest32, cancel: Cancellation) -> (ResourceBroker, RunResources, Reservation) {
    // Explicit large TEST profile. Production default budget is not changed.
    let cap = ResourceLimits::uniform(320*1024*1024).unwrap();
    let broker = ResourceBroker::new(cap).unwrap();
    let run = broker.begin_run(22,source,cap,
        Deadline::after(Duration::from_secs(120)).unwrap(),cancel).unwrap();
    let reservation = run.reserve(&[
        ResourceRequest::new(ResourceKind::Ram,ResourceAmount::checked(256*1024*1024).unwrap()),
        ResourceRequest::new(ResourceKind::Output,ResourceAmount::checked(32*1024*1024).unwrap()),
    ]).unwrap();
    (broker,run,reservation)
}

pub fn exercise_six_actual_owners() {
    let source = SourceStore::new(Arc::from("Given independent finite component models"),1024).unwrap();
    let context = ContextRevision::original(SourceRevision::of(source.text()));
    let pins = SelectionPins {source:context.source(),scope:ContextScope::declared(pin(1),pin(2),pin(3),pin(4)),
        baseline:pin(5),hard_invariants:pin(6),policy:pin(7),goal_profile:None,evidence_revision:pin(9)};
    // Fixed BEFORE any candidate/measurement capture. These are Given registry
    // labels, not checked semantic correspondence between the different models.
    let plan = CollectionPlan::declared(pins,OriginalFormula::F2V1).unwrap();
    // This subset is fixed before production too. It permits a partial
    // ComponentAssessment; original F2 still independently requires19 leaves.
    let group_required = [M::Design,M::Prediction,M::CounterexampleRisk];
    let group_plan = CollectionPlan::declared_subset(pins,OriginalFormula::F2V1,&group_required).unwrap();
    let cancel = Cancellation::default();
    let (_broker,_run,reservation) = run(source.id().digest(),cancel.clone());
    let view = reservation.lease();

    let term = [3,1,b'x']; // Independent literal canonical K6 Symbol("x").
    let specs = [recon::ProbeSpec {path:vec![],kind:recon::FeatureKind::Symbol,
        weight:Rational::from_integer(1.into())}];
    let term_scope = ComparisonScope{id:ScopeId([1;32]),baseline:Revision([5;32]),
        root_goal:Revision([2;32]),probes:Revision([3;32]),policy:Revision([4;32])};
    let term_plan = recon::ProtectedFeatureSet::capture(&term,&specs,context,term_scope,&view).unwrap();
    let reconstruction = recon::produce(&term_plan,&term,&view).unwrap();
    let input = desc::ReconstructionInput {owner:&reconstruction,plan:&term_plan,
        target:&term,context,transform:TransformId([22;32])};
    let description_root = desc::RootPlan::capture(input,16,&view).unwrap();
    let description = desc::produce(&description_root,input,&view).unwrap();

    let edges = [structure::Edge(structure::Vertex(0),structure::Vertex(1))];
    let graph = structure::ClosedGraph::checked(&source,context,2,&edges).unwrap();
    let protected = [structure::ProtectedEdge{edge:edges[0],weight:1}];
    let mapping = [structure::Correspondence{source:structure::Vertex(0),target:structure::Vertex(0)},
        structure::Correspondence{source:structure::Vertex(1),target:structure::Vertex(1)}];
    let preservation_plan = structure::PreservationPlan::declared(&graph,pins.scope,2,&protected,Some(&mapping)).unwrap();
    let preservation = structure::OwnedPreservation::produce(&preservation_plan,Some(&graph),
        structure::CandidateBinding{id:CandidateId(1),identity:pin(21),context,pins},&view).unwrap();

    let f = dsg::Fixture::gold();
    let mut db = dsg::binding(); db.selection=pins; db.context=context;
    let suite = design::register(db,&[f.workload(b"case")],dsg::limits(),&view).unwrap();
    let order = dsg::improved();
    let programs = [design::PriorityProgram{case:b"case",order:&order}];
    let mut dc = dsg::candidate(1); dc.context=context;
    let frozen_design = design::bind_candidate(&suite,dc,&programs,&view).unwrap();
    let design_gain = design::execute(&frozen_design,&view).unwrap();

    let mut pb = pred::binding(1,prediction::SplitRole::Validation); pb.selection=pins; pb.context=context;
    let query = pred::query(pb); let trace = pred::trace(Truth::True);
    let p0 = pred::q(0,1); let pz = pred::q(1,2); let weight = pred::q(1,1);
    let forecasts = [prediction::Forecast{canonical_sample:b"actual-label",baseline:&p0,candidate:&pz,
        weight:&weight,query:&query,trace:&trace}];
    let frozen_prediction = prediction::freeze(pb,&forecasts,prediction::ForecastLimits{max_node_steps:1024},&view).unwrap();
    let prediction_gain = prediction::evaluate(&frozen_prediction,&view).unwrap();

    let mut rb = a::binding(); rb.selection=pins; rb.context=context;
    let theory = a::theory(&[],vec![]); let yes = BoolExpr::Constant(true);
    let cases = [risk::AttackCase{key:b"soft",atoms:&[],theory:&theory,reference:&yes,weight:&weight}];
    let catalog = risk::register(rb,&cases,a::limits(),&view).unwrap();
    let mut rc = a::candidate(1); rc.context=context;
    let responses = [risk::Response{case:b"soft",term:&yes}];
    let frozen_risk = risk::bind_candidate(&catalog,rc,&responses,&view).unwrap();
    let risk_gain = risk::execute(&frozen_risk,&view).unwrap();

    let mf = mem::Fixture::three(vec![mem::bounds(1,1,1,1)],vec![(1,3,0)]);
    let pairs = mf.pairs(); let kappa = mem::q(1,1);
    let mut mb = mem::binding(1); mb.selection=pins; mb.context=context;
    // Keep the native BridgeCost target(31), not Design/Prediction/Risk target(30).
    let memory_plan = memory::RegistryPlan::declared(mb,&mf.states,&pairs,memory::StateId(1),
        memory::StateId(3),mem::unit(),&kappa,mem::budget());
    let memory_gain = memory::execute_owned(&memory_plan,&view).unwrap();

    // Every ref carries actual owners AND each producer's original frozen input.
    let owners = [
        ComponentOwnerRef::Description(&description), ComponentOwnerRef::Preserve(&preservation),
        ComponentOwnerRef::Design{owner:&design_gain,frozen:&frozen_design},
        ComponentOwnerRef::Prediction{owner:&prediction_gain,frozen:&frozen_prediction},
        ComponentOwnerRef::CounterRisk{owner:&risk_gain,frozen:&frozen_risk},
        ComponentOwnerRef::BridgeCost{owner:&memory_gain,plan:&memory_plan},
    ];
    let originals = [description.report_current(&view).unwrap(),preservation.report_current(&view).unwrap(),
        design_gain.report_current(&frozen_design,&view).unwrap(),
        prediction_gain.report_current(&frozen_prediction,&view).unwrap(),
        risk_gain.report_current(&frozen_risk,&view).unwrap(),memory_gain.report_current(&memory_plan,&view).unwrap()];
    let snapshots:Vec<_> = originals.iter().map(|r| (*r).clone()).collect();
    let subject = CandidateSubject::from_binding(originals[2].binding);
    let collection = OwnedComponentCollection::capture(&plan,subject,&owners,&view).unwrap();
    // Same six-subject fields do not excuse a foreign native root policy.
    // Description's actual scope.probes differs from its report.probe_set;
    // native lineage scope must be checked without equating those fields.
    for axis in 0..3 {
        let scope = ContextScope::declared(pin(1),if axis==0 {pin(99)}else{pin(2)},
            if axis==1 {pin(99)}else{pin(3)},if axis==2 {pin(99)}else{pin(4)});
        let foreign = CollectionPlan::declared(SelectionPins{scope,..pins},OriginalFormula::F2V1).unwrap();
        for index in 0..2 {
            let singleton = [owners[index]];
            let native_subject = CandidateSubject::from_binding(originals[index].binding);
            assert!(OwnedComponentCollection::capture(&foreign,native_subject,&singleton,&view).is_err());
        }
    }
    for foreign_pins in [SelectionPins{hard_invariants:pin(99),..pins},
        SelectionPins{evidence_revision:pin(99),..pins},SelectionPins{policy:pin(99),..pins}] {
        let foreign = CollectionPlan::declared(foreign_pins,OriginalFormula::F2V1).unwrap();
        let singleton = [owners[1]];
        let native_subject = CandidateSubject::from_binding(originals[1].binding);
        assert!(OwnedComponentCollection::capture(&foreign,native_subject,&singleton,&view).is_err());
    }
    collection.with_current(&view,|current| {
        assert_eq!(current.native_reports().len(),6);
        assert_eq!(current.exact_reports().len(),3);
        assert_eq!(current.correspondence_required().len(),3);
        assert_eq!(current.missing_leaves().len(),16);
        assert!(current.assessment().is_none());
        assert!(current.scalar().is_none());
        assert!(!current.formula_evaluated());
        assert!(current.tl_correspondence_pending());
        for (actual,original) in current.native_reports().iter().zip(originals) {
            assert!(std::ptr::eq(*actual,original));
        }
        // Actual measured zero is present; an omitted owner is a different state.
        assert_eq!(current.exact_reports().iter().find(|r| r.metric==M::CounterexampleRisk).unwrap().state,
            ComponentState::Measured(RationalInterval::exact_ratio(0,1).unwrap()));
    }).unwrap();
    let exact_group = OwnedComponentCollection::capture(&group_plan,subject,&owners,&view).unwrap();
    exact_group.with_current(&view,|current| {
        let assessment = current.assessment().expect("fixed three-metric subset is present");
        assert_eq!(assessment.reports().len(),3);
        assert!(std::ptr::eq(assessment.get(M::Prediction).unwrap(),originals[3]));
        assert_eq!(current.missing_leaves().len(),16); // Original F2, not subset.
        assert!(current.scalar().is_none());
    }).unwrap();
    for omitted in 0..owners.len() {
        let cut:Vec<_> = owners.iter().enumerate().filter(|(i,_)| *i!=omitted).map(|(_,o)| *o).collect();
        let cut_collection = OwnedComponentCollection::capture(&plan,subject,&cut,&view).unwrap();
        cut_collection.with_current(&view,|current| {
            assert_eq!(current.native_reports().len(),5);
            let exact_cut = (2..=4).contains(&omitted);
            assert_eq!(current.exact_reports().len(),if exact_cut {2}else{3});
            assert_eq!(current.missing_leaves().len(),if exact_cut {17}else{16});
            assert_eq!(current.correspondence_required().len(),if exact_cut {3}else{2});
            assert!(current.scalar().is_none());
        }).unwrap();
        if omitted==3 {
            let missing_required = OwnedComponentCollection::capture(&group_plan,subject,&cut,&view).unwrap();
            missing_required.with_current(&view,|current| {
                assert!(current.assessment().is_none());
                assert!(current.missing_leaves().contains(&M::Prediction));
                assert!(current.scalar().is_none());
            }).unwrap();
        }
    }
    // Exact values/bytes are insufficient when a different native frozen object
    // is supplied with the old owner; the producer's pointer check must run.
    let other_frozen = prediction::freeze(pb,&forecasts,prediction::ForecastLimits{max_node_steps:1024},&view).unwrap();
    let foreign_plan = [ComponentOwnerRef::Prediction{owner:&prediction_gain,frozen:&other_frozen}];
    assert!(OwnedComponentCollection::capture(&plan,subject,&foreign_plan,&view).is_err());
    let mut wrong_subject = subject; wrong_subject.scope=pin(99);
    assert!(OwnedComponentCollection::capture(&plan,wrong_subject,&owners,&view).is_err());
    let (_foreign_broker,_foreign_run,foreign_reservation) = run(source.id().digest(),Cancellation::default());
    let foreign_view = foreign_reservation.lease();
    assert!(collection.with_current(&foreign_view,|_| ()).is_err());
    for (actual,snapshot) in originals.iter().zip(&snapshots) { assert_eq!(*actual,snapshot); }
    // Cancellation inside the diagnostic consumer must fail the post-callback
    // current check as well; the callback result cannot become a success.
    assert!(collection.with_current(&view,|_| cancel.cancel()).is_err());
    assert!(collection.with_current(&view,|_| ()).is_err());
}
