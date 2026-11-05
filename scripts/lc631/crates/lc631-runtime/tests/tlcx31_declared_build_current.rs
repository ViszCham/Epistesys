//! Actual repair/native probes plus fresh declared-input captures. Central runs.
#[path="tlcx31_coding_support/mod.rs"] mod native;
#[path="tlcx32_verified_support/mod.rs"] mod file;
use lc631_analysis::*;
use lc631_analysis::declared_build_capture::*;
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_host::JournalBundleLimits;
use lc631_runtime::{contextual_repair::{execution::*,measurement::*,snapshot_gate::*},
    contextual_resume::budget::*,journal::JournalChainLimits,resource::*};
use lc631_substrate::Digest32;
use std::time::Duration;

#[derive(Clone,Copy)] enum Mode { Renewed, Missing, Revoked, LateCut }
fn q(kind:ResourceKind,n:u64)->ResourceRequest {ResourceRequest::new(kind,ResourceAmount::checked(n).unwrap())}

fn exercise(mode:Mode) {
    let root=file::root(); eprintln!("retained owned declared-build fixture {}",root.display());
    std::fs::write(root.join("src/lib.rs"),native::BEFORE).unwrap();
    std::fs::write(root.join("baseline.rs"),native::BEFORE).unwrap();
    let baseline_program=root.join(format!("baseline{}",std::env::consts::EXE_SUFFIX));
    native::compile_native(&root.join("baseline.rs"),&baseline_program);
    let source=native::SOURCE;
    let pipeline=build_dgcl_pipeline(source,DgclBackendSet{english:None,japanese:None}).unwrap();
    let issuer=ReceiptIssuer::from_key_bytes("tlcx31-declared-build",&[116;32]).unwrap();
    let verifier=ReceiptVerifier::from_key_bytes("tlcx31-declared-build",&[116;32]).unwrap();
    let scope=format!("dgcl/repair/{}/src/lib.rs",dgcl_repository_root_digest(&root).unwrap());
    let edit=file::permit(&scope,&issuer,&verifier);
    let context=ExecutionPermitContext{principal:edit.principal_binding(),action:Action::Edit,scope:&scope,source_revision:&pipeline.source_revision,authority_revision:AuthorityRevision(1),now_epoch:100,revocation_revision:0,revoked_permit_digests:&[],caller_origin:CallerOrigin::HostVerifiedUser,trusted_host_fingerprint:verifier.key_fingerprint()};
    let initial=capture_dgcl_cargo_snapshot(&root,source,&pipeline,&context).unwrap();
    std::fs::write(root.join("src/lib.rs"),native::AFTER).unwrap();
    let intended=capture_dgcl_cargo_snapshot(&root,source,&pipeline,&context).unwrap();
    std::fs::write(root.join("src/lib.rs"),native::BEFORE).unwrap();
    let request=DgclRepairRequest{attempt:1,gap_id:pipeline.implementation_closure.gaps[0].gap_id.clone(),source_revision:pipeline.source_revision.clone(),plan_digest:pipeline.completion_plan.plan_digest.clone(),target_ref:"src/lib.rs".into(),target_digest_before:stable_sha256(native::BEFORE),candidate_digest:stable_sha256(native::AFTER),authorization_scope:scope.clone(),authorization_span:edit.source_span(),repository_root_digest:Some(dgcl_repository_root_digest(&root).unwrap())};
    let plan=CodingProbePlan::checked_add(digest(b"actual goal"),digest(b"actual policy")).unwrap();
    let caps=BudgetCaps{attempts:1,per_attempt_delta_bytes:1000,total_delta_bytes:1000};
    let binding=BudgetBinding{run_family:Digest32::from_bytes([117;32]),source:file::parse(&pipeline.source_revision),context:digest(b"context"),scope:digest(b"comparison"),goal:plan.goal(),policy:plan.policy(),root_target:file::parse(request.repository_root_digest.as_deref().unwrap()),initial_snapshot:file::parse(&initial.snapshot_digest)};
    let manifest=BudgetRunManifest{binding,plan:file::parse(&request.plan_digest),caps,requests:vec![BudgetRequestSpec{request_digest:file::parse(&dgcl_repair_request_digest(&request)),before_snapshot:binding.initial_snapshot,intended_snapshot:file::parse(&intended.snapshot_digest),reserved_delta_bytes:1000}]};
    let raw=issuer.issue(ReceiptClass::Validation,SubjectRevision::checked(&pipeline.source_revision).unwrap(),ReceiptScope::checked(manifest.receipt_scope()).unwrap(),manifest.payload_digest().unwrap(),100,Some(400),None).unwrap();
    let checked=CheckedBudgetRun::verify(manifest,raw,&verifier,&mut ReplayGuard::default(),100).unwrap();
    let limits=ResourceLimits::uniform(128*1024*1024).unwrap();
    let broker=ResourceBroker::new(limits).unwrap();
    let mut run=broker.begin_run(1,binding.source,limits,Deadline::after(Duration::from_secs(90)).unwrap(),Cancellation::default()).unwrap();
    let mut reservation=run.reserve(&[q(ResourceKind::Ram,64*1024*1024),q(ResourceKind::Output,64*1024*1024),q(ResourceKind::Disk,64*1024*1024),q(ResourceKind::Child,2),q(ResourceKind::Cleanup,2)]).unwrap();
    let lease=issue_dgcl_evidence_lease(&initial,100,300).unwrap();
    let authorization=DgclRepairAuthorization{permit:&edit,authority_revision:AuthorityRevision(1),trusted_host_fingerprint:verifier.key_fingerprint(),now_epoch:100,revocation_revision:0,revoked_permit_digests:&[]};
    let mut inputs=file::Inputs{run:&run}; let mut effects=file::Effects{run:&run,reject:false,calls:0}; let mut validator=file::EmptyValidation;
    {
        let resources=reservation.lease();
        let budget=BudgetSession::initialize_new(&root.join("budget-journal"),binding,caps,65536,JournalBundleLimits::checked(20,1024*1024).unwrap(),JournalChainLimits::default(),&resources).unwrap();
        let config=DgclFileRepairConfig{root:&root,source,pipeline:&pipeline,initial_snapshot:&initial,authorization:&authorization,issuer:&issuer,verifier:&verifier,candidates:std::collections::BTreeMap::from([(stable_sha256(native::AFTER),native::AFTER.into())]),validator:&mut validator,epoch_clock:None};
        let inner=BudgetedFileRepairDriver::new(config,&mut inputs,&mut effects,budget,checked).unwrap();
        let baseline=native::observe(&plan,&initial,&root.join("baseline.rs"),&baseline_program,&issuer,&verifier,&resources);
        let mut callback=|token:&DgclVerifiedRepairValidation<'_>| {
            let program=root.join(format!("current{}",std::env::consts::EXE_SUFFIX));
            native::compile_native(&root.join("src/lib.rs"),&program);
            // Explicit fixture Test authority is independently renewed to v2;
            // the accepted validation snapshot still records Edit revision v1.
            let mut provider=native::snapshot_authority(&root,&issuer,&verifier,100,AuthorityRevision(2));
            if matches!(mode,Mode::Revoked) {provider.revoked.push(provider.permit.digest().into());}
            if matches!(mode,Mode::LateCut) {provider.reject_after=Some(1);}
            assert!(provider.scope.starts_with("tlcx/declared-build/read/v1/"));
            let registration=native::NativeProbeFixture::registered(&plan,token.application().after_snapshot.clone(),root.join("src/lib.rs"),program,&issuer,&verifier);
            registration.with_invocation(&verifier,|invocation| {
                if matches!(mode,Mode::Missing) {observe_current(&plan,invocation,&resources)}
                else {observe_current_with_build_authority(&plan,invocation,&resources,Box::new(provider))}
            })
        };
        let mut driver=RemeasuringRepairDriver::new(inner,&plan,&baseline,&resources,&mut callback).unwrap();
        let gaps=pipeline.implementation_closure.gaps.iter().filter(|g|!matches!(g.state,ImplementationGapState::VerifiedReceipt|ImplementationGapState::NotApplicable)).map(|g|g.gap_id.clone()).collect::<Vec<_>>();
        let report=run_dgcl_repair_loop(DgclRepairLoopInput{source,source_revision:&pipeline.source_revision,plan_digest:&pipeline.completion_plan.plan_digest,program_ir:&pipeline.program_ir,translation:&pipeline.translation_projection,completion_plan:&pipeline.completion_plan,initial_snapshot:&initial,initial_lease:&lease,initial_gap_ids:&gaps,requests:&[request],authorization:DgclRepairAuthorization{permit:&edit,authority_revision:AuthorityRevision(1),trusted_host_fingerprint:verifier.key_fingerprint(),now_epoch:100,revocation_revision:0,revoked_permit_digests:&[]},verifier:&verifier,replay:&mut ReplayGuard::default(),max_attempts:1,max_wall_time:Duration::from_secs(60),max_delta_bytes:1000},&mut driver).unwrap();
        assert_eq!(driver.budget_summary().unwrap().charged_attempts,1);
        assert!(report.attempts[0].application_receipt_digest.is_some());
        assert!(report.attempts[0].validation_receipt_digest.is_some());
        assert_eq!(driver.observations().len(),1); assert_eq!(driver.gains()[0].gain_quarters(),3);
        match mode {
            Mode::Renewed=>{
                assert_eq!(driver.recommendation_declared_current(),Ok(Some(0)));
                assert_eq!(driver.build_profile(),CodingBuildProfile::DeclaredInputsChecked);
                assert_eq!(driver.build_captures().len(),2);
                for attempt in driver.build_captures() {
                    let capture=attempt.current().unwrap();
                    assert_eq!(capture.content_pins(),DeclaredBuildContentPins::from_snapshot(&intended).unwrap());
                    assert_eq!(capture.authority_before().authority_revision,AuthorityRevision(2));
                    assert_eq!(capture.authority_after().unwrap().authority_revision,AuthorityRevision(2));
                }
            }
            Mode::Missing=>{
                assert_eq!(driver.recommendation_declared_current(),Err(ProbeError::BuildSnapshot(SnapshotGateError::MissingBuildAuthority)));
                assert_eq!(driver.build_profile(),CodingBuildProfile::PhysicalOnly);
                assert!(driver.build_captures().is_empty());
            }
            Mode::Revoked=>{
                assert!(driver.recommendation_declared_current().is_err());
                assert!(driver.build_captures()[0].capture().is_none());
                assert_eq!(driver.build_captures()[0].diagnostic(),Some(&SnapshotCaptureError::Unauthorized));
            }
            Mode::LateCut=>{
                assert!(driver.recommendation_declared_current().is_err());
                assert!(driver.build_captures()[0].capture().is_some());
                assert_eq!(driver.build_captures()[0].diagnostic(),Some(&SnapshotCaptureError::AuthorityUnavailable));
            }
        }
    }
    assert_eq!(effects.calls,1);
    reservation.release_confirmed().unwrap(); run.close_confirmed().unwrap();
}
#[test] fn tlcx31_declared_current_accepts_renewed_test_authority_and_retains_real_captures(){exercise(Mode::Renewed);}
#[test] fn tlcx31_declared_current_missing_authority_stays_explicitly_physical_only(){exercise(Mode::Missing);}
#[test] fn tlcx31_declared_current_revoked_test_does_not_erase_repair_or_gain(){exercise(Mode::Revoked);}
#[test] fn tlcx31_declared_current_late_authority_cut_retains_actual_capture(){exercise(Mode::LateCut);}
