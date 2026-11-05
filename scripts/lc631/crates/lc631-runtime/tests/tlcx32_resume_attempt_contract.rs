//! 負例先行の未来API契約。中央のmissing-API RED待ちで、実行成功ではない。
//! Future producer-owned attempt contract; central missing-API RED is pending.
#[path="tlcx32_verified_support/mod.rs"]
#[allow(dead_code, reason="Actual repair setup reused; other helpers have separate targets.")]
mod file_fixture;
#[path="tlcx31_coding_support/mod.rs"]
#[allow(dead_code, reason="Native registration fixture reused; live helpers have separate targets.")]
mod coding_support;
#[path="tlcx32_fresh_support/mod.rs"]
#[allow(dead_code, reason="Only actual coordinator/token-capture setup reused; old resume is not the tested entry.")]
mod fresh_support;
#[path="tlcx32_resume_attempt_support/mod.rs"] mod support;
use support::*;
use lc631_runtime::{contextual_repair::{attempt::*, measurement::ProbeError},
    contextual_resume::{production_contract::*, transition::*}, journal::*, resource::*};
use lc631_substrate::{codec::CRITICAL, Digest32};
use std::time::Duration;

fn decode<'a,'r>(bytes:&'a [u8], origin:&CodingResumeOrigin, view:&'r ResourceReservationView<'_>)
    ->Result<CheckedCodingResumePacket<'a,'r>,CodingResumePacketError> {
    decode_coding_resume_packet(UntrustedCodingResumePacket::borrowed(bytes),origin,
        CodingResumePacketLimits::finite_v1(),view)
}

#[test]
fn tlcx32_whole_packet_requires_all_six_sections_and_exact_counts() {
    let _serial=SERIAL.lock().unwrap(); let fixture=PacketFixture::actual();
    funded(|_,view| {
        let bytes=fixture.bytes(); let origin=fixture.origin(&bytes);
        let packet=decode(&bytes,&origin,view).unwrap();
        assert_eq!(packet.source(),coding_support::SOURCE);
        assert_eq!(packet.frame_digest(),frame_digest(&bytes));
        assert_eq!(packet.segment(),fixture.parts[5].1.as_slice());
        // Shape/crossbinding is not authentication or current evidence.
        assert!(packet.claim_boundary().contains("untrusted"));
        drop(packet);
        for omitted in 0..6 {
            let mut parts=fixture.parts.clone(); parts.remove(omitted);
            let bad=fixture.encode(&parts); let expected=fixture.origin(&bad);
            assert!(decode(&bad,&expected,view).is_err(),"missing tag {omitted}");
        }
        for section in 0..6 {
            let mut bad=bytes.clone(); let at=64+section*32+24;
            bad[at..at+4].copy_from_slice(&2_u32.to_le_bytes()); rehash(&mut bad);
            assert!(decode(&bad,&fixture.origin(&bad),view).is_err());
        }
    });
}

#[test]
fn tlcx32_whole_packet_rejects_trailing_duplicates_unknowncritical_and_frame_swap() {
    let _serial=SERIAL.lock().unwrap(); let fixture=PacketFixture::actual();
    funded(|_,view| {
        let bytes=fixture.bytes(); let origin=fixture.origin(&bytes);
        for cut in [0,7,63,64,255,bytes.len()-1] {assert!(decode(&bytes[..cut],&origin,view).is_err());}
        let mut trailing=bytes.clone(); trailing.push(0); assert!(decode(&trailing,&origin,view).is_err());
        let mut duplicate=bytes.clone(); duplicate[96..98].copy_from_slice(&TAGS[0].to_le_bytes());
        rehash(&mut duplicate); assert!(decode(&duplicate,&fixture.origin(&duplicate),view).is_err());
        let mut parts=fixture.parts.clone(); parts.push((0x32ff,b"opaque-preserved".to_vec()));
        let optional=fixture.encode(&parts);
        let packet=decode(&optional,&fixture.origin(&optional),view).unwrap();
        assert_eq!(packet.opaque_sections().len(),1);
        assert_eq!(packet.opaque_sections()[0].payload(),b"opaque-preserved"); drop(packet);
        let mut critical=optional.clone(); let flags=64+6*32+4;
        critical[flags..flags+4].copy_from_slice(&CRITICAL.to_le_bytes()); rehash(&mut critical);
        assert!(decode(&critical,&fixture.origin(&critical),view).is_err());
        let foreign=CodingResumeOrigin::checked(fixture.pins,Digest32::from_bytes([90;32])).unwrap();
        assert!(decode(&bytes,&foreign,view).is_err());
    });
}

#[test]
fn tlcx32_whole_packet_strict_json_and_rehashed_binding_cuts() {
    let _serial=SERIAL.lock().unwrap(); let fixture=PacketFixture::actual();
    funded(|_,view| {
        let request=serde_json::to_string(&fixture.raw["request"]).unwrap();
        for (section,payload) in [
            (2,format!("{{\"schema_version\":\"epistesys-coding-resume-request.v1\",\"request\":{request},\"request\":{request}}}").into_bytes()),
            (2,format!("{{\"schema_version\":\"epistesys-coding-resume-request.v1\",\"request\":{request},\"unknown\":1}}").into_bytes()),
        ] {let mut parts=fixture.parts.clone();parts[section].1=payload;
            let bad=fixture.encode(&parts);assert!(decode(&bad,&fixture.origin(&bad),view).is_err());}
        for pin in 0..15 {
            let mut parts=fixture.parts.clone();parts[0].1[pin*32..(pin+1)*32].fill(77);
            let bad=fixture.encode(&parts);
            // Expected origin is independently supplied, not copied from bad descriptor.
            assert!(decode(&bad,&fixture.origin(&bad),view).is_err(),"pin {pin}");
        }
        for field in ["target_digest_before","target_digest_after","candidate_digest"] {
            let mut parts=fixture.parts.clone();
            let mut app:serde_json::Value=serde_json::from_slice(&parts[3].1).unwrap();
            app["application"][field]=serde_json::Value::String(digest_text(Digest32::from_bytes([78;32])));
            parts[3].1=serde_json::to_vec(&app).unwrap();let bad=fixture.encode(&parts);
            assert!(decode(&bad,&fixture.origin(&bad),view).is_err());
        }
        let mut parts=fixture.parts.clone();let mut app:serde_json::Value=serde_json::from_slice(&parts[3].1).unwrap();
        app["application"]["applied"]=serde_json::Value::Bool(false);
        parts[3].1=serde_json::to_vec(&app).unwrap();let bad=fixture.encode(&parts);
        assert!(decode(&bad,&fixture.origin(&bad),view).is_err());
    });
}

#[test]
fn tlcx32_packet_authentication_is_only_the_existing_history_producer() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();
    funded(|_,view| {
        let bytes=fixture.bytes();let origin=fixture.origin(&bytes);let packet=decode(&bytes,&origin,view).unwrap();
        let (_,verifier)=signing();
        let history=inspect_transition_chain(&[packet.segment()],None,JournalChainLimits::default(),
            TransitionEvidence{request:packet.request(),application:packet.application(),
                transition_attestation:packet.attestation(),verifier:&verifier,now_epoch:200},view).unwrap();
        assert_eq!(history.before_build(),Some(fixture.pins[9]));
        assert_eq!(history.original_request_root(),Some(fixture.pins[0]));
        assert!(history.claim_boundary().contains("Unanchored"));
        let wrong=lc631_receipt_kernel::ReceiptVerifier::from_key_bytes("foreign-provider",&[82;32]).unwrap();
        assert!(matches!(inspect_transition_chain(&[packet.segment()],None,JournalChainLimits::default(),
            TransitionEvidence{request:packet.request(),application:packet.application(),
                transition_attestation:packet.attestation(),verifier:&wrong,now_epoch:200},view),
            Err(TransitionError::ReceiptRejected)));
    });
}

#[test]
fn tlcx32_packet_rejects_two_evidence_records_even_with_valid_outer_frame() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();
    funded(|_,view| {
        let segment=decode_segment(&fixture.parts[5].1).unwrap();let record=&segment.records()[0];
        let mut builder=JournalSegmentBuilder::genesis();
        builder.append_at(1,JournalRecordKind::Evidence,record.object_digest(),record.payload()).unwrap();
        builder.append_at(2,JournalRecordKind::Evidence,record.object_digest(),record.payload()).unwrap();
        let mut parts=fixture.parts.clone();parts[5].1=builder.finish().unwrap();
        let bytes=fixture.encode(&parts);
        assert!(decode(&bytes,&fixture.origin(&bytes),view).is_err());
    });
}

#[test]
fn tlcx32_default_missing_provider_has_zero_native_attempts_and_no_current_winner() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();let before=protected_state(&fixture.root);
    funded(|run,view|configured(&fixture,run,view,|input,_again,mut port,_probe,_build,_pipeline| {
        let mut budget=CodingResumeCallBudget::checked(2,2).unwrap();
        let attempt=resume_coding_attempt(input,&mut port,None,None,&mut budget,view);
        assert!(matches!(attempt.diagnostic(),Some(CodingResumeError::MissingProvider)));
        assert!(attempt.baseline().is_none());assert!(attempt.current_probe().is_none());
        assert!(attempt.current(view).is_err());
        assert_eq!((budget.used_native(),budget.used_captures()),(0,0));
    }));assert_eq!(before,protected_state(&fixture.root));
}

#[test]
fn tlcx32_configured_same_sdk_entry_observes_gain_then_fresh_publication_without_writes() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();let before=protected_state(&fixture.root);
    funded(|run,view|configured(&fixture,run,view,|input,again,mut port,mut probe,mut build,pipeline| {
        let plan=input.plan;build.calls=0;let mut budget=CodingResumeCallBudget::checked(2,2).unwrap();
        let mut attempt=resume_coding_attempt(input,&mut port,Some(&mut probe),Some(&mut build),&mut budget,view);
        assert_eq!(attempt.stage(),CodingResumeStage::ReadyForPublication);
        assert!(matches!(attempt.current(view),Err(CodingResumeError::NeedsPublication)));
        assert_eq!(attempt.baseline().unwrap().observation().unwrap().actual_outputs(),&[None,None,None,None]);
        assert_eq!(attempt.current_probe().unwrap().observation().unwrap().actual_outputs(),
            &[Some(0),Some(3),None,Some(u32::MAX)]);
        assert_eq!(attempt.gain().unwrap().gain_quarters(),3);
        assert_eq!((budget.used_native(),budget.used_captures()),(2,1));
        assert!(attempt.publish_current(plan,&again,&mut port,&mut probe,&mut build,&mut budget,view).unwrap());
        assert!(attempt.current(view).unwrap());
        assert_eq!((budget.used_native(),budget.used_captures()),(2,2));
        assert_eq!(probe.calls,vec![(ProbePurpose::Baseline,ProbeAuthorityPhase::BeforeSpawn),
            (ProbePurpose::Baseline,ProbeAuthorityPhase::AfterChild),
            (ProbePurpose::Current,ProbeAuthorityPhase::BeforeSpawn),
            (ProbePurpose::Current,ProbeAuthorityPhase::AfterChild),
            (ProbePurpose::Current,ProbeAuthorityPhase::BeforePublication)]);
        // Publication consumes no third child, and a second publication cannot cache the winner.
        let calls=probe.calls.len();let captures=build.calls;drop(port);
        // Recreating a port must not recreate command-wide admitted work.
        let mut reader=file_fixture::Inputs{run};
        let mut new_port=lc631_runtime::contextual_repair::snapshot_gate::CodingBuildPort::checked(
            &fixture.root,coding_support::SOURCE,pipeline,fixture.pins[0],fixture.pins[14],fixture.pins[4],&mut reader).unwrap();
        assert!(matches!(attempt.publish_current(plan,&again,&mut new_port,&mut probe,&mut build,&mut budget,view),
            Err(CodingResumeError::BudgetExhausted)));
        assert_eq!((probe.calls.len(),build.calls),(calls,captures));assert!(attempt.current(view).is_err());
    }));
    // Actual protected bytes/mtime/inventories unchanged. This is not universal syscall interception.
    assert_eq!(before,protected_state(&fixture.root));
}

#[test]
fn tlcx32_current_postchild_failure_retains_both_actual_native_owners() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();
    funded(|run,view|configured(&fixture,run,view,|input,_again,mut port,mut probe,mut build,_pipeline| {
        probe.reject=Some((ProbePurpose::Current,ProbeAuthorityPhase::AfterChild));
        let mut budget=CodingResumeCallBudget::checked(2,2).unwrap();
        let attempt=resume_coding_attempt(input,&mut port,Some(&mut probe),Some(&mut build),&mut budget,view);
        let baseline=attempt.baseline().unwrap();let current=attempt.current_probe().unwrap();
        assert_eq!(baseline.observation().unwrap().correct_probes(),1);
        assert_eq!(current.effect(),NativeStageEffect::ExecutedObserved);
        assert_eq!(current.process().unwrap().stdout(),b"some:0\nsome:3\nnone\nsome:4294967295\n");
        assert_eq!(current.observation().unwrap().correct_probes(),4);
        assert!(matches!(current.diagnostic(),Some(ProbeError::Unauthorized)));
        assert!(attempt.current(view).is_err());assert!(attempt.diagnostic().is_some());
        assert!(view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,ResourceAmount::checked(RAM).unwrap())]).is_err());
        drop(attempt);
        let full=view.claim_resources(&[ResourceRequest::new(ResourceKind::Ram,ResourceAmount::checked(RAM).unwrap())]).unwrap();
        drop(full);
    }));
}

#[test]
fn tlcx32_per_child_authority_frame_and_clock_are_not_restored_from_packet() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();
    for mode in 0..5 {funded(|run,view|configured(&fixture,run,view,|input,_again,mut port,mut probe,mut build,_pipeline| {
        if mode==0 {probe.reject=Some((ProbePurpose::Current,ProbeAuthorityPhase::BeforeSpawn));}
        if mode==1 {probe.foreign_frame=true;}
        if mode==2 {probe.rollback=true;}
        if mode==3 {probe.foreign_scope=true;}
        if mode==4 {probe.revoke_current();}
        let mut budget=CodingResumeCallBudget::checked(2,2).unwrap();
        let attempt=resume_coding_attempt(input,&mut port,Some(&mut probe),Some(&mut build),&mut budget,view);
        assert!(attempt.current(view).is_err());
        if mode==0 || mode==4 {assert!(attempt.baseline().unwrap().process().is_some());
            assert_eq!(attempt.current_probe().unwrap().effect(),NativeStageEffect::NotAttempted);
            assert!(attempt.current_probe().unwrap().process().is_none());}
        else {assert!(attempt.baseline().unwrap().process().is_none());}
    }));}
}

#[test]
fn tlcx32_publication_cut_retains_nativepair_and_late_actual_capture() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();
    funded(|run,view|configured(&fixture,run,view,|input,again,mut port,mut probe,mut build,_pipeline| {
        let plan=input.plan;let mut budget=CodingResumeCallBudget::checked(2,2).unwrap();
        let mut attempt=resume_coding_attempt(input,&mut port,Some(&mut probe),Some(&mut build),&mut budget,view);
        build.reject_after=Some(build.calls+1);
        assert!(attempt.publish_current(plan,&again,&mut port,&mut probe,&mut build,&mut budget,view).is_err());
        assert!(attempt.baseline().unwrap().observation().is_some());
        assert!(attempt.current_probe().unwrap().observation().is_some());
        assert_eq!(attempt.build_attempts().len(),2);
        assert!(attempt.build_attempts()[1].capture().is_some());
        assert!(attempt.build_attempts()[1].current().is_err());assert!(attempt.current(view).is_err());
    }));
}

#[test]
fn tlcx32_publication_rechecks_new_origin_and_actual_declared_build() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();
    for foreign in [true,false] {funded(|run,view|configured(&fixture,run,view,
        |input,again,mut port,mut probe,mut build,_pipeline| {
        let plan=input.plan;let mut budget=CodingResumeCallBudget::checked(2,2).unwrap();
        let mut attempt=resume_coding_attempt(input,&mut port,Some(&mut probe),Some(&mut build),&mut budget,view);
        let cargo=fixture.root.join("Cargo.toml");let original=std::fs::read(&cargo).unwrap();
        if foreign {probe.foreign_frame=true;} else {
            let mut changed=original.clone();changed.extend_from_slice(b"\n# owned declared-build cut\n");
            std::fs::write(&cargo,changed).unwrap();
        }
        let outcome=attempt.publish_current(plan,&again,&mut port,&mut probe,&mut build,&mut budget,view);
        if !foreign {std::fs::write(&cargo,original).unwrap();}
        assert!(outcome.is_err());assert!(attempt.current(view).is_err());
        assert!(attempt.baseline().unwrap().observation().is_some());
        assert!(attempt.current_probe().unwrap().observation().is_some());assert_eq!(budget.used_native(),2);
        if !foreign {let capture=attempt.build_attempts().last().unwrap().capture().unwrap();
            assert_ne!(capture.content_pins().build,fixture.pins[10]);}
    }));}
}

#[test]
fn tlcx32_narrowed_authoritative_deadline_controls_already_live_resume_views() {
    let _serial=SERIAL.lock().unwrap();let fixture=PacketFixture::actual();
    funded(|run,view|configured(&fixture,run,view,|input,_again,mut port,mut probe,mut build,_pipeline| {
        let identity=(view.owner_id(),view.run_id(),view.source_revision(),view.reservation_id());
        run.narrow_deadline(Deadline::after(Duration::from_millis(1)).unwrap()).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(matches!(view.check(),Err(ResourceError::DeadlineExceeded)));
        assert_eq!(identity,(view.owner_id(),view.run_id(),view.source_revision(),view.reservation_id()));
        let mut budget=CodingResumeCallBudget::checked(2,2).unwrap();
        let attempt=resume_coding_attempt(input,&mut port,Some(&mut probe),Some(&mut build),&mut budget,view);
        assert!(attempt.current(view).is_err());assert!(probe.calls.is_empty());
        assert!(attempt.baseline().is_none());assert!(attempt.current_probe().is_none());
    }));
}
