//! Actual finite owners, not reported-codec rename or spoofed source rebinding.
#[allow(dead_code)]
#[path = "../tlcx22_product_current_correspondence_stage_b_support/models.rs"]
mod models;
#[path = "../tlcx22_product_current_correspondence_stage_b_support/resources.rs"]
mod resources;
mod atoms;
use lc631_analysis::creativity_measurement::{ComponentState, InputBasis, Rational, RationalInterval, SemanticValidation};
use lc631_core::CandidateId;
use lc631_runtime::{
    adversarial_measurement as risk, checked_tl_reduction as tl,
    description_measurement as desc, facet_ledger::*, joint_selection::SelectionPins,
    product_correspondence::{inspect_native_current, CaseDeclaration, DeferredReason,
        IdentityDeclaration, IdentityKind, NativeOwnerRefs, PlanSpec, ProductError,
        ProductInput, ProductLimits, ProductPlan, TlProfile},
    reconstruction_measurement as recon, resource::*,
    spatiotemporal_selection::{SpatialOwner, TemporalOwner},
};
use lc631_solvers::temporal::{EvaluationBudget, Truth};
use lc631_substrate::Digest32;
use std::time::Duration;

#[derive(Clone,Copy)]
pub enum Mode { Normal, Renamed }
#[derive(Clone,Copy,Debug)]
pub enum Variant { Normal, RenamedBoth, SymbolOnly, MeaningOnly, UnusedMeaning,
    ExtraUnusedAtom, TheoryOnly, ResponseOnly }
impl Variant {
    fn accepts(self) -> bool { matches!(self, Self::Normal | Self::RenamedBoth) }
}
fn pin(n: u8) -> Digest32 { Digest32::from_bytes([n;32]) }
pub struct Snapshot { pub baseline: Vec<u8>, pub target: Vec<u8> }

/// The full real Description/TL/Risk stack is reconstructed for every cut.
/// Same truth/loss is not accepted as correspondence of a different catalog.
pub fn exercise(variant: Variant, single: bool, expiry: bool) -> Snapshot {
    atoms::hand_oracle();
    let model_mode = if matches!(variant, Variant::RenamedBoth) { Mode::Renamed } else { Mode::Normal };
    let context = models::context();
    let scope = models::scope();
    let cancellation = Cancellation::default();
    let (_broker,_run,reservation) = resources::run(cancellation.clone(),expiry);
    let view = reservation.lease();
    let input = atoms::input(variant,single);
    let spec = atoms::spec(single);
    let reference = atoms::reference_contract();
    let weight = Rational::from_integer(1.into());
    let key = b"nonzero-case";
    let cases = [CaseDeclaration { native_key:key, native_atoms:&input.declared,
        semantic_spec:&spec, reference_contract:&reference, oracle:pin(11), weight:&weight }];
    let clock = models::clock_meaning("primary");
    let origin = models::meaning("Origin/primary",models::symbol("epoch-zero"));
    let reference_time = models::meaning("Reference/primary",models::symbol("first-event"));
    let frame = models::frame_meaning();
    let identities = [
        IdentityDeclaration{kind:IdentityKind::Clock,native_id:models::identity(model_mode,40),local_role:0,meaning:&clock},
        IdentityDeclaration{kind:IdentityKind::Origin,native_id:models::identity(model_mode,41),local_role:0,meaning:&origin},
        IdentityDeclaration{kind:IdentityKind::Reference,native_id:models::identity(model_mode,42),local_role:0,meaning:&reference_time},
        IdentityDeclaration{kind:IdentityKind::Frame,native_id:models::identity(model_mode,60),local_role:0,meaning:&frame},
    ];
    let time_atom = models::encoded(models::symbol("observed-p"));
    let time_atoms:[&[u8];1] = [&time_atom];
    let plan = ProductPlan::declared(PlanSpec{context,scope,goal_profile:None,
        tl_profile:TlProfile::TemporalSpatial1To3,cases:&cases,identities:&identities,
        temporal_atom_meanings:&time_atoms,limits:ProductLimits::default()},&view).unwrap();
    assert_eq!(plan.fixed_slots()[0].semantic_spec(),spec);
    assert_eq!(plan.fixed_slots()[0].reference_contract(),reference);
    let source_query = models::query();
    let target_query = models::query();
    let source_trace = models::trace(model_mode,false,Truth::True);
    let target_trace = models::trace(model_mode,false,Truth::False);
    let source_time = models::temporal(&source_query,&source_trace);
    let target_time = models::temporal(&target_query,&target_trace);
    let source_space = models::spatial(model_mode,false);
    let target_space = models::spatial(model_mode,true);
    let reported_base = [risk::Response{case:key,term:&input.reported_baseline}];
    let reported_target = [risk::Response{case:key,term:&input.reported_target}];
    let baseline = plan.project_reported(ProductInput{responses:&reported_base,temporal:source_time,spatial:source_space},&view).unwrap();
    let target = plan.project_reported(ProductInput{responses:&reported_target,temporal:target_time,spatial:target_space},&view).unwrap();
    let snapshot = Snapshot{baseline:baseline.semantic_bytes().to_vec(),target:target.semantic_bytes().to_vec()};

    let pins = SelectionPins{source:context.source(),scope,baseline:pin(5),hard_invariants:pin(6),
        policy:pin(7),goal_profile:None,evidence_revision:pin(9)};
    let attacks = [risk::AttackCase{key,atoms:&input.actual,theory:&input.theory,
        reference:&input.reference,weight:&weight}];
    let catalog = risk::register(risk::CatalogBinding{selection:pins,context,suite:pin(10),attack_scope:pin(12),
        oracle:pin(11),definition:risk::definition_revision(),probe_set:scope.probes(),dependency_group:pin(13),
        purpose:risk::CasePurpose::SoftFiniteValidation,quantification:risk::Quantification::AllAdmissibleAssignments},
        &attacks,risk::AttackLimits{registration_ast_visits:100_000,max_ast_visits:100_000,max_trials:2},&view).unwrap();
    let binding = |n| risk::CandidateBinding{id:CandidateId(n),candidate_digest:pin(20+n as u8),context,
        target:pin(30+n as u8),transform:pin(40+n as u8)};
    let actual_base = [risk::Response{case:key,term:&input.baseline}];
    let actual_target = [risk::Response{case:key,term:&input.target}];
    let frozen_base = risk::bind_candidate(&catalog,binding(0),&actual_base,&view).unwrap();
    let base_risk = risk::execute(&frozen_base,&view).unwrap();
    let frozen_target = risk::bind_candidate(&catalog,binding(1),&actual_target,&view).unwrap();
    let target_risk = risk::execute(&frozen_target,&view).unwrap();
    // Hand oracle: Not(alpha) always disagrees; alpha / NotNot(alpha) never do.
    // The finite evaluator and witness checker are actual production owners.
    assert_eq!(base_risk.historical_trials(),[risk::TrialState::Failure]);
    assert_eq!(target_risk.historical_trials(),[risk::TrialState::Pass]);
    assert!(base_risk.counterrow(0).is_some());
    assert!(base_risk.witness_query(0).is_some());
    assert!(base_risk.witness_model(0).is_some());
    assert!(target_risk.counterrow(0).is_none());
    assert_eq!(base_risk.historical_report().state,ComponentState::Measured(RationalInterval::exact_ratio(1,1).unwrap()));
    assert_eq!(target_risk.historical_report().state,ComponentState::Measured(RationalInterval::exact_ratio(0,1).unwrap()));

    let source_tp = models::time_policy(source_time,source_time);
    let source_sp = models::space_policy(source_space,source_space);
    let tl_root = tl::RootPlan::capture(source_time,source_space,&source_tp,&source_sp,&view).unwrap();
    let tuple = tl::TupleCandidate::capture(&tl_root,target_time,target_space).unwrap();
    let target_tp = models::time_policy(source_time,target_time);
    let target_sp = models::space_policy(source_space,target_space);
    let time = TemporalOwner::produce(&target_tp,source_time,target_time,&view,&view,EvaluationBudget::default()).unwrap();
    let space = SpatialOwner::produce(&target_sp,source_space,target_space,&view,&view).unwrap();
    let reduction = tl::reduce_current(&tl_root,&tuple,tl::Owners{temporal:Some(&time),spatial:Some(&space)},&view).unwrap();
    let desc_scope = ComparisonScope{id:ScopeId([1;32]),baseline:Revision([5;32]),root_goal:Revision([2;32]),
        probes:Revision([3;32]),policy:Revision([4;32])};
    let probes = [recon::ProbeSpec{path:vec![],kind:recon::FeatureKind::NodeKind,weight:weight.clone()}];
    let features = recon::ProtectedFeatureSet::capture(baseline.semantic_bytes(),&probes,context,desc_scope,&view).unwrap();
    let base_recon = recon::produce(&features,baseline.semantic_bytes(),&view).unwrap();
    let target_recon = recon::produce(&features,target.semantic_bytes(),&view).unwrap();
    let base_input = desc::ReconstructionInput{owner:&base_recon,plan:&features,target:baseline.semantic_bytes(),context,transform:TransformId([80;32])};
    let desc_root = desc::RootPlan::capture(base_input,16,&view).unwrap();
    let base_desc = desc::produce(&desc_root,base_input,&view).unwrap();
    let target_desc = desc::produce(&desc_root,desc::ReconstructionInput{owner:&target_recon,plan:&features,
        target:target.semantic_bytes(),context,transform:TransformId([81;32])},&view).unwrap();
    let owners = NativeOwnerRefs{baseline_description:&base_desc,target_description:&target_desc,
        baseline_risk:&base_risk,baseline_responses:&frozen_base,target_risk:&target_risk,target_responses:&frozen_target,
        tl_root:&tl_root,tl_candidate:&tuple,tl_reduction:&reduction};
    let saved_base = base_risk.historical_report().clone();
    let saved_target = target_risk.historical_report().clone();
    let inspection = inspect_native_current(&plan,&baseline,&target,owners,&view);
    if !variant.accepts() {
        // Matching real Description and TL owners reach actual risk-catalog
        // correspondence. Same finite pass/failure alone cannot admit a cut.
        assert!(matches!(inspection,Err(ProductError::CorrespondenceMismatch)),"{variant:?} accepted");
        assert_eq!(base_risk.historical_report(),&saved_base);
        assert_eq!(target_risk.historical_report(),&saved_target);
        return snapshot;
    }
    let inspection = inspection.unwrap();
    inspection.with_current(&view,|current| {
        assert!(std::ptr::eq(current.baseline_risk_report(),base_risk.report_current(&frozen_base,&view).unwrap()));
        assert!(std::ptr::eq(current.target_risk_report(),target_risk.report_current(&frozen_target,&view).unwrap()));
        assert!(std::ptr::eq(current.target_description_report(),target_desc.report_current(&view).unwrap()));
        assert!(std::ptr::eq(current.translation_report(),reduction.current(&tl_root,&tuple,&view).unwrap()));
        assert_eq!(current.target_risk_report().basis,InputBasis::Given);
        assert_eq!(current.target_risk_report().validation,SemanticValidation::Unassessed);
        assert_eq!(current.target_description_report().validation,SemanticValidation::Unassessed);
        assert_eq!(current.scalar_status(),DeferredReason::FullCostOwnerMissing);
        assert!(current.scalar().is_none());
        assert!(!current.formula_evaluated());
    }).unwrap();
    let (_fb,_fr,foreign_res) = resources::run(Cancellation::default(),false);
    let foreign = foreign_res.lease();
    foreign.check().unwrap();
    assert!(inspection.with_current(&foreign,|_| ()).is_err());
    if expiry {
        let remaining = view.remaining().expect("actual inspection must first be live");
        assert!(remaining<=Duration::from_secs(2));
        std::thread::sleep(remaining+Duration::from_millis(5));
        assert!(inspection.with_current(&view,|_| ()).is_err());
    } else {
        assert!(inspection.with_current(&view,|_| cancellation.cancel()).is_err());
        assert!(inspection.with_current(&view,|_| ()).is_err());
    }
    assert_eq!(base_risk.historical_report(),&saved_base);
    assert_eq!(target_risk.historical_report(),&saved_target);
    snapshot
}
