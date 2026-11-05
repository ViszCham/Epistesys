//! Retained real owners/current/budgets. No raw report can construct P3 ownership.
mod tlcx22_framework_p3_support;
use tlcx22_framework_p3_support::*;

#[test]
fn tlcx22_p3_n07_exact_native_not_bank_cap_zero_38_39_keeps_partial_non_scalar() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        for cap in [0,38,39] {
            let mut sr=semantic_registration(&ps,&bs);sr.limits.max_native_dispatches=cap;
            p3_root!(h,view,fd,nr,sr,root);
            let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
            let e=semantic_pair(&root,&left,le,&right,re,view);let owner=root.measure_current(e,view).unwrap();
            owner.with_current(e,view,|v|{
                measured(&v,ComponentMetric::FrameworkDistance,1,4);
                assert_eq!(v.counts().required_not_dispatches,39);
                if cap<39 {
                    assert_eq!(v.counts().native_not_dispatches,0);
                    assert_eq!(v.counts().native_not_store_entries,0);
                    assert_eq!(v.status(),s::Status::Partial);
                    assert!(matches!(v.report(ComponentMetric::SemanticConfigurationDistance).unwrap().state,
                        ComponentState::ResourceExhausted {partial:None}));
                    assert!(v.tnc().is_err());
                } else {
                    assert_eq!(v.counts().native_not_dispatches,39);
                    assert_eq!(v.status(),s::Status::Complete);
                    measured(&v,ComponentMetric::NoveltyCompatibility,1,4);
                }
            }).unwrap();
        }
    });
}

#[test]
fn tlcx22_p3_n08_root_native_budget_is_nonrefundable_across_new_assessments() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let mut sr=semantic_registration(&ps,&bs);sr.limits.max_native_dispatches=39;
        p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
        let e=semantic_pair(&root,&left,le,&right,re,view);
        let first=root.measure_current(e,view).unwrap();
        first.with_current(e,view,|v|{measured(&v,ComponentMetric::NoveltyCompatibility,1,4);}).unwrap();
        let second=root.measure_current(e,view).unwrap();
        second.with_current(e,view,|v|{
            assert_eq!(v.status(),s::Status::Partial);assert_eq!(v.counts().native_not_dispatches,0);
            assert!(v.tnc().is_err());
        }).unwrap();
        assert!(matches!(root.measure_current(e,view),Err(s::Error::AssessmentBudget)));
    });
}

#[test]
fn tlcx22_p3_n09_current_identity_rejects_foreign_root_registration_queries_and_reservation() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,run)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let other=s::Root::preregister(m::Root::preregister(
            f::FrameworkRoot::preregister(h.native(view),&fd,view).unwrap(),&nr,view).unwrap(),&sr,view).unwrap();
        assert_eq!(root.historical_digest(),other.historical_digest());
        let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
        let e=semantic_pair(&root,&left,le,&right,re,view);let owner=root.measure_current(e,view).unwrap();
        let calls=Cell::new(0);let mut changed=e;changed.root=&other;
        assert!(matches!(owner.with_current(changed,view,|_|calls.set(1)),Err(s::Error::ForeignRoot)));
        let copy=semantic_registration(&ps,&bs);let mut changed=e;changed.registration=&copy;
        assert!(matches!(owner.with_current(changed,view,|_|calls.set(1)),Err(s::Error::ExpectedInputMismatch)));
        let cloned=re.subject_query.clone();let mut changed=e;changed.pair.target.expected.subject_query=&cloned;
        assert!(matches!(owner.with_current(changed,view,|_|calls.set(1)),Err(s::Error::ExpectedInputMismatch)));
        let foreign=run.reserve(&[resource(ResourceKind::Ram,1024*1024)]).unwrap();let foreign_view=foreign.lease();
        assert!(matches!(owner.with_current(e,&foreign_view,|_|calls.set(1)),Err(s::Error::ForeignReservation)));
        assert_eq!(calls.get(),0);
    });
}

#[test]
fn tlcx22_p3_n10_cancellation_inside_callback_rejects_the_value_not_rollback() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,cancel,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
        let e=semantic_pair(&root,&left,le,&right,re,view);let owner=root.measure_current(e,view).unwrap();
        let calls=Cell::new(0);
        let result=owner.with_current(e,view,|v|{
            measured(&v,ComponentMetric::NoveltyCompatibility,1,4);
            calls.set(calls.get()+1);cancel.cancel();73
        });
        assert!(matches!(result,Err(s::Error::Resource(ResourceError::Cancelled))));
        assert_eq!(calls.get(),1);
        assert!(matches!(owner.with_current(e,view,|_|calls.set(9)),Err(s::Error::Resource(ResourceError::Cancelled))));
        assert_eq!(calls.get(),1);
    });
}

#[test]
fn tlcx22_p3_n11_two_measure_four_callback_quota_fits_existing_p1_sixteen_without_reset() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let sr=semantic_registration(&ps,&bs);p3_root!(h,view,fd,nr,sr,root);
        let generated=root.generate_current(operator(),view).unwrap();p2_pair!(root,generated,view,left,le,right,re);
        let e=semantic_pair(&root,&left,le,&right,re,view);
        let before=root.work_remaining_current(view).unwrap();
        let owners=[root.measure_current(e,view).unwrap(),root.measure_current(e,view).unwrap()];
        let calls=Cell::new(0);
        for owner in &owners {
            for _ in 0..2 {owner.with_current(e,view,|v|{
                measured(&v,ComponentMetric::NoveltyCompatibility,1,4);calls.set(calls.get()+1);
            }).unwrap();}
        }
        assert_eq!(calls.get(),4);assert!(root.work_remaining_current(view).unwrap()<before);
        assert!(matches!(owners[0].with_current(e,view,|_|calls.set(9)),Err(s::Error::CurrentBudget)));
        assert_eq!(calls.get(),4);assert!(matches!(root.measure_current(e,view),Err(s::Error::AssessmentBudget)));
    });
}

#[test]
fn tlcx22_p3_n12_already_generated_novelty_root_cannot_backdate_registration() {
    framework_fixture!(h,2,false,Duration::from_secs(60),{
        let (h,view,_,_)=h;let one=q(1,1);let ps=[lem_probe(&h,&one)];let bs=bridges(&one);
        let fd=h.declaration();let nr=registration(h.constructions,m::ArchiveInput::Unread);
        let mut novelty=m::Root::preregister(f::FrameworkRoot::preregister(h.native(view),&fd,view).unwrap(),&nr,view).unwrap();
        let actual=novelty.generate_current(operator(),view).unwrap();assert_eq!(actual.operator(),operator());
        let sr=semantic_registration(&ps,&bs);
        assert!(matches!(s::Root::preregister(novelty,&sr,view),Err(s::Error::AlreadyGenerated)));
    });
}

