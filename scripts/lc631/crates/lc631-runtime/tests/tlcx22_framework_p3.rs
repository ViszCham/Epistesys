//! P3 TEST-FIRST only. Missing semantic module is construction RED, not behavior success.
mod tlcx22_framework_p3_support;
use tlcx22_framework_p3_support::*;

#[test]
fn tlcx22_p3_n01_duplicate_roles_zero_weights_and_detached_probe_ast_are_rejected() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let zero=q(0,1);let bs=bridges(&one);
        let duplicate=[lem_probe(&h,&one),lem_probe(&h,&one)];
        let invalid=[lem_probe(&h,&zero)];
        let copied=h.probes[2].formula.clone();
        let detached=[s::SemanticProbe {role:f::QueryRole::BridgeLaw,formula:&copied,weight:&one}];
        for (probes,reason) in [(&duplicate[..],0),(&invalid[..],1),(&detached[..],2)] {
            let fd=h.declaration();let nr=registration(h.constructions,m::ArchiveInput::Unread);
            let sr=semantic_registration(probes,&bs);
            let novelty=m::Root::preregister(f::FrameworkRoot::preregister(h.native(view),&fd,view).unwrap(),&nr,view).unwrap();
            let result=s::Root::preregister(novelty,&sr,view);
            assert!(match reason {0=>matches!(result,Err(s::Error::DuplicateProbe)),
                1=>matches!(result,Err(s::Error::InvalidWeight)),
                _=>matches!(result,Err(s::Error::InvalidProbe))});
        }
    });
}

#[test]
fn tlcx22_p3_n02_known_payload_and_numeric_limits_precede_native_not_dispatch() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        for kind in 0..2 {
            let fd=h.declaration();let nr=registration(h.constructions,m::ArchiveInput::Unread);
            let mut sr=semantic_registration(&ps,&bs);
            if kind==0 {sr.limits.max_payload_bytes=1;} else {sr.limits.max_weight_bits=0;}
            let novelty=m::Root::preregister(f::FrameworkRoot::preregister(h.native(view),&fd,view).unwrap(),&nr,view).unwrap();
            let result=s::Root::preregister(novelty,&sr,view);
            assert!(if kind==0 {matches!(result,Err(s::Error::PayloadBudget))}
                else {matches!(result,Err(s::Error::NumericBudget))});
        }
    });
}

#[test]
fn tlcx22_p3_p01_future_nonforcing_is_not_negation_and_three_actual_leaves_give_quarter() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
        let e=semantic_pair(&root,&left,le,&right,re,view);
        let owner=root.measure_current(e,view).unwrap();
        owner.with_current(e,view,|v|{
            measured(&v,ComponentMetric::FrameworkDistance,1,4);
            measured(&v,ComponentMetric::SemanticConfigurationDistance,1,1);
            measured(&v,ComponentMetric::Compatibility,1,2);
            measured(&v,ComponentMetric::NoveltyCompatibility,1,4);
            assert_eq!(v.tnc().unwrap(),&RationalInterval::exact_ratio(1,4).unwrap());
            let q=v.query(s::Side::Target,f::QueryRole::Placement).unwrap();
            assert_eq!(q.status(),s::CanonicalStatus::Undetermined);
            assert_eq!((q.feasible_models(),q.forced_models(),q.negation_forced_models()),(3,1,1));
            let lem=v.query(s::Side::Target,f::QueryRole::BridgeLaw).unwrap();
            assert_eq!(lem.status(),s::CanonicalStatus::Undetermined);
            assert_eq!((lem.feasible_models(),lem.forced_models(),lem.negation_forced_models()),(3,2,0));
            for (i,row) in q.rows().iter().filter(|r|r.admitted()).enumerate() {
                assert_eq!(row.query_bits(),Q_BITS[i]);assert_eq!(row.negation_bits(),NOT_Q_BITS[i]);
            }
            for (i,row) in lem.rows().iter().filter(|r|r.admitted()).enumerate() {
                assert_eq!(row.query_bits(),LEM_BITS[i]);assert_eq!(row.negation_bits(),NOT_LEM_BITS[i]);
            }
            let middle=q.rows().iter().find(|r|r.valuation_states()==[2,1]).unwrap();
            assert_eq!((middle.query_bits()&1,middle.negation_bits()&1),(0,0));
            assert_eq!(v.counts().native_not_dispatches,39);
            assert_eq!(v.counts().native_not_store_entries,27);
            assert_eq!(v.bridge_results().iter().map(|b|b.passed()).collect::<Vec<_>>(),[true,false]);
            let witness=v.bridge_results()[1].mismatches();
            assert_eq!(witness.len(),1);assert_eq!(witness[0].target_states(),&[2,1]);
            assert_eq!((witness[0].source_root_forced(),witness[0].target_root_forced()),(true,false));
            assert_eq!(witness[0].lost_future_atoms(),&[1]);
            let d=v.report(ComponentMetric::SemanticConfigurationDistance).unwrap();
            let a=v.report(ComponentMetric::Compatibility).unwrap();
            assert_eq!(d.dependency_group,a.dependency_group);
            assert_ne!(d.binding.producer,a.binding.producer);
            no_full_f2(&v);
        }).unwrap();
    });
}

#[test]
fn tlcx22_p3_p02_same_actual_root_four_cells_control_context_and_framework_without_retagging() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();
        let c0e=root.expected_case_current(f::Subject::Original,f::Kernel::ClassicalPointV1,view).unwrap();
        let c0=root.execute_case_current(c0e,view).unwrap();
        let c1e=root.expected_case_current(f::Subject::Generated(&generated),f::Kernel::ClassicalPointV1,view).unwrap();
        let c1=root.execute_case_current(c1e,view).unwrap();
        let i0e=root.expected_case_current(f::Subject::Original,f::Kernel::PersistentChain2V1,view).unwrap();
        let i0=root.execute_case_current(i0e,view).unwrap();
        let i1e=root.expected_case_current(f::Subject::Generated(&generated),f::Kernel::PersistentChain2V1,view).unwrap();
        let i1=root.execute_case_current(i1e,view).unwrap();
        let expected_q:[&[u64];4]=[&[1],&[0,1],&[3],&[0,2,3]];
        let expected_lem:[&[u64];4]=[&[1],&[1,1],&[3],&[3,2,3]];
        for (i,(owner,e)) in [(&c0,c0e),(&c1,c1e),(&i0,i0e),(&i1,i1e)].into_iter().enumerate() {
            owner.with_current(e,view,|v|{
                assert_eq!(v.rows().iter().enumerate().filter_map(|(i,r)|r.admitted().then_some(i)).collect::<Vec<_>>(),ADMITTED[i]);
                assert_eq!(v.rows().iter().filter(|r|r.admitted()).map(|r|r.query_bits()[1]).collect::<Vec<_>>(),expected_q[i]);
                assert_eq!(v.rows().iter().filter(|r|r.admitted()).map(|r|r.query_bits()[2]).collect::<Vec<_>>(),expected_lem[i]);
            }).unwrap();
        }
        let e=semantic_pair(&root,&c0,c0e,&i1,i1e,view);
        let owner=root.measure_current(e,view).unwrap();
        owner.with_current(e,view,|v|{
            assert_eq!(v.query(s::Side::Source,f::QueryRole::Placement).unwrap().status(),Q_STATUS[0]);
            assert_eq!(v.query(s::Side::Target,f::QueryRole::Placement).unwrap().status(),Q_STATUS[3]);
            assert_eq!(v.query(s::Side::Source,f::QueryRole::BridgeLaw).unwrap().status(),LEM_STATUS[0]);
            assert_eq!(v.query(s::Side::Target,f::QueryRole::BridgeLaw).unwrap().status(),LEM_STATUS[3]);
            assert!(std::ptr::eq(v.source_query(),c0e.subject_query));
            assert!(std::ptr::eq(v.target_query(),i1e.subject_query));
        }).unwrap();
    });
}

#[test]
fn tlcx22_p3_p03_weighted_status_difference_and_irrational_tnc_are_outward_not_false_exact() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let two=q(2,1);
        let ps=[s::SemanticProbe {role:f::QueryRole::Placement,formula:h.probes[1].formula,weight:&two},lem_probe(&h,&one)];
        let bs=bridges(&one);let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();
        let le=root.expected_case_current(f::Subject::Generated(&generated),f::Kernel::ClassicalPointV1,view).unwrap();
        let left=root.execute_case_current(le,view).unwrap();
        let re=root.expected_case_current(f::Subject::Generated(&generated),f::Kernel::PersistentChain2V1,view).unwrap();
        let right=root.execute_case_current(re,view).unwrap();
        let e=semantic_pair(&root,&left,le,&right,re,view);let owner=root.measure_current(e,view).unwrap();
        owner.with_current(e,view,|v|{
            measured(&v,ComponentMetric::FrameworkDistance,1,4);
            measured(&v,ComponentMetric::SemanticConfigurationDistance,1,3);
            measured(&v,ComponentMetric::Compatibility,1,2);
            let t=v.tnc().unwrap();assert!(t.lower()<t.upper());
            assert!(t.lower()*t.lower()*q(48,1)<=q(1,1));
            assert!(t.upper()*t.upper()*q(48,1)>=q(1,1));
            assert!(t.upper()-t.lower()<=Rational::new(1.into(),(1u128<<65).into()));
            assert_eq!(v.semantic_denominator(),&q(3,1));assert_eq!(v.bridge_denominator(),&q(2,1));
        }).unwrap();
    });
}

#[test]
fn tlcx22_p3_p04_reverse_actual_map_is_rechecked_and_compatibility_is_directional() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
        let e=semantic_pair(&root,&right,re,&left,le,view);let owner=root.measure_current(e,view).unwrap();
        owner.with_current(e,view,|v|{
            measured(&v,ComponentMetric::FrameworkDistance,1,4);
            measured(&v,ComponentMetric::SemanticConfigurationDistance,1,1);
            measured(&v,ComponentMetric::Compatibility,1,1);
            measured(&v,ComponentMetric::NoveltyCompatibility,1,2);
            assert!(v.bridge_results().iter().all(|b|b.passed()));
        }).unwrap();
    });
}

#[test]
fn tlcx22_p3_n03_same_signature_zero_cannot_rescue_undefined_semantic_denominator() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let bs=bridges(&one);
        let sr=semantic_registration(&[],&bs);p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();
        let le=root.expected_case_current(f::Subject::Original,f::Kernel::ClassicalPointV1,view).unwrap();
        let left=root.execute_case_current(le,view).unwrap();
        let re=root.expected_case_current(f::Subject::Generated(&generated),f::Kernel::ClassicalPointV1,view).unwrap();
        let right=root.execute_case_current(re,view).unwrap();
        let e=semantic_pair(&root,&left,le,&right,re,view);let owner=root.measure_current(e,view).unwrap();
        owner.with_current(e,view,|v|{
            measured(&v,ComponentMetric::FrameworkDistance,0,1);
            assert_eq!(v.report(ComponentMetric::SemanticConfigurationDistance).unwrap().state,
                ComponentState::Undefined(MeasurementError::EmptyDenominator));
            assert!(v.tnc().is_err());assert!(!matches!(v.report(ComponentMetric::NoveltyCompatibility).unwrap().state,ComponentState::Measured(_)));
            no_full_f2(&v);
        }).unwrap();
    });
}

#[test]
fn tlcx22_p3_n04_missing_outcome_or_bridge_mapping_remains_no_comparable_no_scalar() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        for kind in 0..2 {
            let mut sr=semantic_registration(&ps,&bs);
            if kind==0 {sr.status_map=None;} else {sr.bridge_map=None;}
            p3_root!(h,view,fd,nr,sr,root);
            let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
            let e=semantic_pair(&root,&left,le,&right,re,view);let owner=root.measure_current(e,view).unwrap();
            owner.with_current(e,view,|v|{
                assert_eq!(v.status(),s::Status::NoComparable);assert!(v.tnc().is_err());
                let metric=if kind==0 {ComponentMetric::SemanticConfigurationDistance}else{ComponentMetric::Compatibility};
                assert!(matches!(v.report(metric).unwrap().state,ComponentState::Incomparable {..}));
                no_full_f2(&v);
            }).unwrap();
        }
    });
}

#[test]
fn tlcx22_p3_n05_required_lem_is_hard_failure_not_soft_half_compatibility() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let mut probes=h.probes.to_vec();probes[2].duty=f::Duty::Required;
        let mut fd=h.declaration();fd.probes=&probes;let nr=registration(h.constructions,m::ArchiveInput::Unread);
        let sr=semantic_registration(&ps,&bs);
        let novelty=m::Root::preregister(f::FrameworkRoot::preregister(h.native(view),&fd,view).unwrap(),&nr,view).unwrap();
        assert!(matches!(s::Root::preregister(novelty,&sr,view),Err(s::Error::RequiredSemanticProbe)));
    });
}

#[test]
fn tlcx22_p3_n06_zero_feasible_models_are_context_inconsistent_not_vacuous_supported() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let bs=bridges(&one);
        let bad=query(&h,vec![BoolExpr::Constant(false),var(p(&h))]);
        let bp=[GivenPremise {id:d(5),span:h.premises[0].span,formula:&bad.assertions()[0]},
            GivenPremise {id:d(6),span:h.premises[1].span,formula:&bad.assertions()[1]}];
        let h=Inputs {theory:&bad,premises:&bp,..h};
        let ps=[lem_probe(&h,&one)];let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let e=root.expected_case_current(f::Subject::Original,f::Kernel::PersistentChain2V1,view).unwrap();
        assert!(matches!(root.execute_case_current(e,view),Err(s::Error::Novelty(m::Error::Framework(f::Error::ContextInconsistent)))));
    });
}

#[test]
fn tlcx22_p3_p05_checked_alias_renaming_preserves_real_leaf_values() {
    for rename in [false,true] {
        framework_fixture!(h,2,rename,Duration::from_secs(60),{
            let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
            let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
            let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
            let e=semantic_pair(&root,&left,le,&right,re,view);let owner=root.measure_current(e,view).unwrap();
            owner.with_current(e,view,|v|{
                measured(&v,ComponentMetric::SemanticConfigurationDistance,1,1);
                measured(&v,ComponentMetric::Compatibility,1,2);
                measured(&v,ComponentMetric::NoveltyCompatibility,1,4);
            }).unwrap();
        });
    }
}

