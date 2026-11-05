//! New source-supported build-drift negatives. Central executes before repair GO.
//! Target/native freshness alone must not claim a fresh declared Cargo snapshot.
#[path="tlcx31_coding_support/mod.rs"]
mod native;
#[path="tlcx32_verified_support/mod.rs"]
mod file;
#[path="../../lc631-analysis/tests/tlcx31_file_repair_fixture/mod.rs"]
#[allow(dead_code, reason="This target reuses the actual package and signed connection specification; execution validation helpers are covered by analysis repair tests.")]
mod package;
use lc631_analysis::*;
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_runtime::{contextual_repair::{execution::RemeasuringRepairDriver,measurement::*},contextual_resume::budget::*,journal::JournalChainLimits,resource::*};
use lc631_host::JournalBundleLimits;
use lc631_substrate::Digest32;
use std::time::Duration;

#[derive(Clone,Copy,Debug)]
enum Drift { Manifest, RequiredRust }
struct Observed {
    report:DgclRepairLoopReport,
    recommendation:Result<Option<usize>,ProbeError>,
    before_build:String,
    fresh_build:String,
    target_unchanged:bool,
    native_unchanged:bool,
    charged_attempts:u64,
}
fn exercise(drift:Drift)->Observed {
    let root=package::fixture_package();
    eprintln!("retained owned build-drift fixture {:?}: {}",drift,root.display());
    let before=native::BEFORE.replace("fn add(","fn checked_add(").replace("match add(","match checked_add(");
    let after=native::AFTER.replace("fn add(","fn checked_add(").replace("match add(","match checked_add(");
    std::fs::write(root.join("baseline.rs"),&before).unwrap();
    std::fs::write(root.join("src/lib.rs"),&before).unwrap();
    let baseline_program=root.join(format!("baseline{}",std::env::consts::EXE_SUFFIX));
    native::compile_native(&root.join("baseline.rs"),&baseline_program);
    let source=native::SOURCE;
    let pipeline=build_dgcl_pipeline(source,DgclBackendSet{english:None,japanese:None}).unwrap();
    let issuer=ReceiptIssuer::from_key_bytes("tlcx31-build-drift",&[111;32]).unwrap();
    let verifier=ReceiptVerifier::from_key_bytes("tlcx31-build-drift",&[111;32]).unwrap();
    // This is a registered required direct driver of the actual implementation,
    // not an arbitrary unreferenced .rs file counted only by the inventory scan.
    let connection=package::authority::connection_spec(&root,&issuer);
    assert_eq!(connection.driver_path,"tests/connection.rs");
    assert_eq!(connection.implementation_path,"src/lib.rs");
    assert_eq!(connection.entrypoint,"dgcl_producer_fixture::checked_add");
    assert!(after.contains("pub fn checked_add("));
    let connection_source=std::fs::read_to_string(root.join(&connection.driver_path)).unwrap();
    assert!(connection_source.contains("dgcl_producer_fixture::checked_add(3, 7)"));
    assert_eq!(native::file_hash(&root.join(&connection.driver_path)),connection.driver_digest);
    verifier.verify(connection.registration.clone().unwrap(),&ReceiptPolicy::exact(ReceiptClass::VerifierCapability,SubjectRevision::checked(&connection.driver_digest).unwrap(),ReceiptScope::checked(dgcl_connection_registration_scope(&connection)).unwrap(),dgcl_connection_registration_payload(&connection),100),&mut ReplayGuard::default()).unwrap();
    let scope=format!("dgcl/repair/{}/src/lib.rs",dgcl_repository_root_digest(&root).unwrap());
    let edit=file::permit(&scope,&issuer,&verifier);
    let authority=ExecutionPermitContext{principal:edit.principal_binding(),action:Action::Edit,scope:&scope,source_revision:&pipeline.source_revision,authority_revision:AuthorityRevision(1),now_epoch:100,revocation_revision:0,revoked_permit_digests:&[],caller_origin:CallerOrigin::HostVerifiedUser,trusted_host_fingerprint:verifier.key_fingerprint()};
    let initial=capture_dgcl_cargo_snapshot(&root,source,&pipeline,&authority).unwrap();
    assert!(initial.targets.iter().any(|target|target.target_ref==connection.driver_path && target.target_digest==connection.driver_digest));
    std::fs::write(root.join("src/lib.rs"),&after).unwrap();
    let intended=capture_dgcl_cargo_snapshot(&root,source,&pipeline,&authority).unwrap();
    std::fs::write(root.join("src/lib.rs"),&before).unwrap();
    let request=DgclRepairRequest{attempt:1,gap_id:pipeline.implementation_closure.gaps[0].gap_id.clone(),source_revision:pipeline.source_revision.clone(),plan_digest:pipeline.completion_plan.plan_digest.clone(),target_ref:"src/lib.rs".into(),target_digest_before:stable_sha256(&before),candidate_digest:stable_sha256(&after),authorization_scope:scope.clone(),authorization_span:edit.source_span(),repository_root_digest:Some(dgcl_repository_root_digest(&root).unwrap())};
    let plan=CodingProbePlan::checked_add(digest(b"fixed-add-goal"),digest(b"fixed-add-policy")).unwrap();
    let caps=BudgetCaps{attempts:1,per_attempt_delta_bytes:2048,total_delta_bytes:2048};
    let binding=BudgetBinding{run_family:Digest32::from_bytes([112;32]),source:file::parse(&pipeline.source_revision),context:digest(b"context"),scope:digest(b"comparison"),goal:plan.goal(),policy:plan.policy(),root_target:file::parse(request.repository_root_digest.as_deref().unwrap()),initial_snapshot:file::parse(&initial.snapshot_digest)};
    let manifest=BudgetRunManifest{binding,plan:file::parse(&request.plan_digest),caps,requests:vec![BudgetRequestSpec{request_digest:file::parse(&dgcl_repair_request_digest(&request)),before_snapshot:binding.initial_snapshot,intended_snapshot:file::parse(&intended.snapshot_digest),reserved_delta_bytes:2048}]};
    let run_receipt=issuer.issue(ReceiptClass::Validation,SubjectRevision::checked(&pipeline.source_revision).unwrap(),ReceiptScope::checked(manifest.receipt_scope()).unwrap(),manifest.payload_digest().unwrap(),100,Some(400),None).unwrap();
    let checked=CheckedBudgetRun::verify(manifest,run_receipt,&verifier,&mut ReplayGuard::default(),100).unwrap();
    let limits=ResourceLimits::uniform(128*1024*1024).unwrap();
    let broker=ResourceBroker::new(limits).unwrap();
    let mut run=broker.begin_run(1,binding.source,limits,Deadline::after(Duration::from_secs(90)).unwrap(),Cancellation::default()).unwrap();
    let mut reservation=run.reserve(&[q(ResourceKind::Ram,64*1024*1024),q(ResourceKind::Output,64*1024*1024),q(ResourceKind::Disk,64*1024*1024),q(ResourceKind::Child,2),q(ResourceKind::Cleanup,2)]).unwrap();
    let lease=issue_dgcl_evidence_lease(&initial,100,300).unwrap();
    let authorization=DgclRepairAuthorization{permit:&edit,authority_revision:AuthorityRevision(1),trusted_host_fingerprint:verifier.key_fingerprint(),now_epoch:100,revocation_revision:0,revoked_permit_digests:&[]};
    let mut inputs=file::Inputs{run:&run};
    let mut effects=file::Effects{run:&run,reject:false,calls:0};
    let mut validator=file::EmptyValidation;
    let result;
    {
        let resources=reservation.lease();
        let budget=BudgetSession::initialize_new(&root.join("budget-journal"),binding,caps,65536,JournalBundleLimits::checked(20,1024*1024).unwrap(),JournalChainLimits::default(),&resources).unwrap();
        let config=DgclFileRepairConfig{root:&root,source,pipeline:&pipeline,initial_snapshot:&initial,authorization:&authorization,issuer:&issuer,verifier:&verifier,candidates:std::collections::BTreeMap::from([(stable_sha256(&after),after.clone())]),validator:&mut validator,epoch_clock:None};
        let inner=BudgetedFileRepairDriver::new(config,&mut inputs,&mut effects,budget,checked).unwrap();
        let baseline=native::observe(&plan,&initial,&root.join("baseline.rs"),&baseline_program,&issuer,&verifier,&resources);
        let mut actual_build=None;
        let mut target_unchanged=false;
        let mut native_unchanged=false;
        let mut callback=|token:&DgclVerifiedRepairValidation<'_>| {
            let program=root.join(format!("current{}",std::env::consts::EXE_SUFFIX));
            native::compile_native(&root.join("src/lib.rs"),&program);
            let observed=native::observe(&plan,&token.application().after_snapshot,&root.join("src/lib.rs"),&program,&issuer,&verifier,&resources);
            let target_before=std::fs::read(root.join("src/lib.rs")).unwrap();
            let binary_before=native::file_hash(&program);
            match drift {
                Drift::Manifest=>{
                    let path=root.join("Cargo.toml");
                    let original=std::fs::read_to_string(&path).unwrap();
                    let changed=original.replace("version = \"0.0.0\"","version = \"0.0.1\"");
                    assert_ne!(changed,original); assert_eq!(changed.len(),original.len());
                    std::fs::write(path,changed).unwrap();
                }
                Drift::RequiredRust=>{
                    let changed=connection_source.replace("checked_add(3, 7)","checked_add(3, 8)");
                    assert_ne!(changed,connection_source); assert_eq!(changed.len(),connection_source.len());
                    std::fs::write(root.join(&connection.driver_path),changed).unwrap();
                }
            }
            // Independently recapture actual declared inputs, not caller report
            // strings. No compile/test/reapply is hidden in this snapshot read.
            let fresh=capture_dgcl_cargo_snapshot(&root,source,&pipeline,&authority).unwrap();
            actual_build=Some(fresh.build_input_digest);
            target_unchanged=std::fs::read(root.join("src/lib.rs")).unwrap()==target_before;
            native_unchanged=native::file_hash(&program)==binary_before;
            Ok(observed)
        };
        let mut driver=RemeasuringRepairDriver::new(inner,&plan,&baseline,&resources,&mut callback).unwrap();
        let gaps=pipeline.implementation_closure.gaps.iter().filter(|g|!matches!(g.state,ImplementationGapState::VerifiedReceipt|ImplementationGapState::NotApplicable)).map(|g|g.gap_id.clone()).collect::<Vec<_>>();
        let report=run_dgcl_repair_loop(DgclRepairLoopInput{source,source_revision:&pipeline.source_revision,plan_digest:&pipeline.completion_plan.plan_digest,program_ir:&pipeline.program_ir,translation:&pipeline.translation_projection,completion_plan:&pipeline.completion_plan,initial_snapshot:&initial,initial_lease:&lease,initial_gap_ids:&gaps,requests:&[request],authorization:DgclRepairAuthorization{permit:&edit,authority_revision:AuthorityRevision(1),trusted_host_fingerprint:verifier.key_fingerprint(),now_epoch:100,revocation_revision:0,revoked_permit_digests:&[]},verifier:&verifier,replay:&mut ReplayGuard::default(),max_attempts:1,max_wall_time:Duration::from_secs(60),max_delta_bytes:2048},&mut driver).unwrap();
        let recommendation=driver.recommendation_current();
        let summary=driver.budget_summary().unwrap();
        drop(driver);
        result=Observed{report,recommendation,before_build:intended.build_input_digest.clone(),fresh_build:actual_build.expect("genuine callback must run"),target_unchanged,native_unchanged,charged_attempts:summary.charged_attempts};
    }
    assert_eq!(effects.calls,1);
    reservation.release_confirmed().unwrap(); run.close_confirmed().unwrap();
    result
}
fn q(kind:ResourceKind,n:u64)->ResourceRequest {ResourceRequest::new(kind,ResourceAmount::checked(n).unwrap())}
fn assert_no_fresh_recommendation(drift:Drift) {
    let observed=exercise(drift);
    assert!(observed.target_unchanged && observed.native_unchanged);
    assert_ne!(observed.before_build,observed.fresh_build,"actual snapshot reader must observe the mutated required input");
    assert_eq!(observed.charged_attempts,1);
    assert!(observed.report.attempts[0].application_receipt_digest.is_some());
    assert!(observed.report.attempts[0].validation_receipt_digest.is_some());
    assert_ne!(observed.report.status,DgclRepairStatus::Completed);
    assert!(!matches!(&observed.recommendation,Ok(Some(_))),"{drift:?} changed the actual declared build snapshot while target/native bytes stayed identical; no fresh recommendation is valid: {:?}",observed.recommendation);
}
#[test]
fn tlcx31_manifest_drift_after_native_observation_withholds_fresh_snapshot_recommendation() {assert_no_fresh_recommendation(Drift::Manifest);}
#[test]
fn tlcx31_required_rust_drift_after_native_observation_withholds_fresh_snapshot_recommendation() {assert_no_fresh_recommendation(Drift::RequiredRust);}
