//! Every semantic scalar/operator/child-count position is changed with the SAME
//! actual owners/plan. Invalid product syntax is counted separately from native
//! correspondence rejection: codec failure alone cannot satisfy this contract.
use lc631_runtime::product_correspondence::{inspect_native_current,CaseDeclaration,IdentityDeclaration,
    NativeOwnerRefs,PlanSpec,ProductInput,ProductLimits,ProductPlan,ReportedProduct,TlProfile};
use lc631_runtime::{description_measurement as desc,facet_ledger::*,reconstruction_measurement as recon};
use lc631_analysis::creativity_measurement::Rational;
use lc631_substrate::{resource::ResourceReservationView,semantic_term::{self,Term,TermLimits}};

fn variants(term:&Term)->Vec<Term> {
    let mut out=Vec::new();
    match term {
        Term::Bool(value)=>out.push(Term::Bool(!value)),
        Term::Integer(value)=>out.push(Term::Integer(value+1)),
        Term::Symbol(value)=>out.push(Term::Symbol(format!("{value}-mutant"))),
        Term::Apply(name,children)=>{
            out.push(Term::Apply(format!("{name}-mutant"),children.clone()));
            let mut extra=children.clone(); extra.push(Term::Bool(false));
            out.push(Term::Apply(name.clone(),extra));
            for (index,child) in children.iter().enumerate() {
                for changed in variants(child) {
                    let mut new_children=children.clone(); new_children[index]=changed;
                    out.push(Term::Apply(name.clone(),new_children));
                }
            }
            if children.len()>1 {
                let mut reordered=children.clone(); reordered.swap(0,1);
                if &reordered!=children {out.push(Term::Apply(name.clone(),reordered));}
            }
        }
        Term::Binder(..)|Term::BoundVar(_)=>panic!("initial product fixture uses no binders"),
    }
    out
}

pub fn all_fields(
    plan:&ProductPlan,
    baseline:&ReportedProduct,
    target:&ReportedProduct,
    owners:NativeOwnerRefs,
    resources:&ResourceReservationView<'_>,
) {
    for mutate_source in [true,false] {
        let original=if mutate_source {baseline}else{target};
        let term=semantic_term::decode(original.semantic_bytes(),TermLimits::default()).unwrap();
        let mutants=variants(&term);
        assert!(mutants.len()>30,"fixture must cover a nonempty complete three-part product");
        let(mut codec_rejections,mut native_rejections)=(0,0);
        for term in mutants {
            let bytes=semantic_term::encode(&term,TermLimits::default()).unwrap();
            match plan.decode_reported(&bytes,original.identity_envelope_bytes(),resources) {
                Err(_)=>codec_rejections+=1,
                Ok(changed)=>{
                    if mutate_source {
                        rejects_with_matching_descriptions(plan,&changed,target,owners,resources);
                    }else{
                        rejects_with_matching_descriptions(plan,baseline,&changed,owners,resources);
                    }
                    native_rejections+=1;
                }
            }
        }
        assert!(codec_rejections>0);
        assert!(native_rejections>0,"valid alternative payloads must reach the actual-owner validator");
        // Raw external identity bytes stay outside semantic K, but still bind
        // native inspection. Mutating ANY envelope byte cannot reuse old owners.
        let original_envelope=original.identity_envelope_bytes();
        assert!(!original_envelope.is_empty());
        for index in 0..original_envelope.len() {
            let mut envelope=original_envelope.to_vec(); envelope[index]^=1;
            if let Ok(changed)=plan.decode_reported(original.semantic_bytes(),&envelope,resources) {
                let result=if mutate_source {inspect_native_current(plan,&changed,target,owners,resources)}
                    else {inspect_native_current(plan,baseline,&changed,owners,resources)};
                assert!(result.is_err(),"a raw identity mutation requires fresh matching native owners");
            }
        }
        // The actual originals still inspect after every rejected mutation.
        assert!(inspect_native_current(plan,baseline,target,owners,resources).is_ok());
    }
}

pub(super) fn rejects_with_matching_descriptions(
    plan:&ProductPlan,baseline:&ReportedProduct,target:&ReportedProduct,
    owners:NativeOwnerRefs,resources:&ResourceReservationView<'_>,
) {
    // Supply VALID actual legacy Description owners for BOTH offered byte
    // streams, including a fresh matching baseline/source root. Keep the SAME
    // real CounterRisk and TL owners. A Description target-hash rejection alone
    // can no longer make every native payload mutation appear correctly cut.
    let context=super::models::context();
    let scope=ComparisonScope{id:ScopeId([1;32]),baseline:Revision([5;32]),root_goal:Revision([2;32]),
        probes:Revision([3;32]),policy:Revision([4;32])};
    let probes=[recon::ProbeSpec{path:vec![],kind:recon::FeatureKind::NodeKind,
        weight:Rational::from_integer(1.into())}];
    let features=recon::ProtectedFeatureSet::capture(baseline.semantic_bytes(),&probes,context,scope,resources).unwrap();
    let base_recon=recon::produce(&features,baseline.semantic_bytes(),resources).unwrap();
    let target_recon=recon::produce(&features,target.semantic_bytes(),resources).unwrap();
    let base_input=desc::ReconstructionInput{owner:&base_recon,plan:&features,target:baseline.semantic_bytes(),
        context,transform:TransformId([90;32])};
    let root=desc::RootPlan::capture(base_input,16,resources).unwrap();
    let base_description=desc::produce(&root,base_input,resources).unwrap();
    let target_description=desc::produce(&root,desc::ReconstructionInput{owner:&target_recon,plan:&features,
        target:target.semantic_bytes(),context,transform:TransformId([91;32])},resources).unwrap();
    assert!(base_description.report_current(resources).is_ok());
    assert!(target_description.report_current(resources).is_ok());
    let matching=NativeOwnerRefs{baseline_description:&base_description,target_description:&target_description,..owners};
    assert!(inspect_native_current(plan,baseline,target,matching,resources).is_err(),
        "matching Description bytes do not excuse a different actual Risk/TL payload");
}

pub fn frozen_plan_fields(
    cases:&[CaseDeclaration],identities:&[IdentityDeclaration],meanings:&[&[u8]],
    source:&ProductInput,target:&ProductInput,owners:NativeOwnerRefs,resources:&ResourceReservationView<'_>,
) {
    let two=Rational::from_integer(2.into());
    let false_reference=super::models::encoded(Term::Bool(false));
    let mut changed_spec=semantic_term::decode(cases[0].semantic_spec,TermLimits::default()).unwrap();
    if let Term::Apply(_,fields)=&mut changed_spec {
        if let Term::Apply(_,assertions)=&mut fields[2] { assertions.push(Term::Bool(true)); }
    }
    let changed_spec=semantic_term::encode(&changed_spec,TermLimits::default()).unwrap();
    let mut extra_atom_spec=semantic_term::decode(cases[0].semantic_spec,TermLimits::default()).unwrap();
    if let Term::Apply(_,fields)=&mut extra_atom_spec {
        if let Term::Apply(_,atoms)=&mut fields[1] { atoms.push(Term::Symbol("p".into())); }
    }
    let extra_atom_spec=semantic_term::encode(&extra_atom_spec,TermLimits::default()).unwrap();
    let extra_atoms=[lc631_runtime::adversarial_measurement::SemanticAtom{
        symbol:lc631_solvers::typed_query::BoolSymbolId::new(17),meaning:b"p"}];
    for field in 0..5 {
        let old=&cases[0];
        let changed=[CaseDeclaration{native_key:old.native_key,native_atoms:if field==4 {&extra_atoms}else{old.native_atoms},
            semantic_spec:if field==3 {&changed_spec}else if field==4 {&extra_atom_spec}else{old.semantic_spec},
            reference_contract:if field==1 {&false_reference}else{old.reference_contract},
            oracle:if field==0 {super::pin(99)}else{old.oracle},weight:if field==2 {&two}else{old.weight}}];
        // Fresh reported products belong to the changed declaration plan. Their
        // real matching Description owners are also reconstructed below. The
        // original actual catalog/theory/reference/weights/Risk/TL stay fixed.
        let plan=ProductPlan::declared(PlanSpec{context:super::models::context(),scope:super::models::scope(),
            goal_profile:None,tl_profile:TlProfile::TemporalSpatial1To3,cases:&changed,identities,
            temporal_atom_meanings:meanings,limits:ProductLimits::default()},resources).unwrap();
        let baseline=plan.project_reported(ProductInput{responses:source.responses,temporal:source.temporal,spatial:source.spatial},resources).unwrap();
        let target=plan.project_reported(ProductInput{responses:target.responses,temporal:target.temporal,spatial:target.spatial},resources).unwrap();
        rejects_with_matching_descriptions(&plan,&baseline,&target,owners,resources);
    }
}
