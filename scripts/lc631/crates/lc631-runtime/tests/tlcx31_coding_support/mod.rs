//! Shared actual coding/native fixture. Test issuers are not observed host users.
#![allow(dead_code)]
use lc631_analysis::{issue_dgcl_evidence_lease,DgclEvidenceLease,DgclExecutionSnapshot};
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_runtime::{contextual_repair::{execution::*,measurement::*},resource::ResourceReservationView};
use std::path::{Path,PathBuf};
use sha2::{Digest,Sha256};

pub const SOURCE:&str="Please edit the source and test the package.";
pub const BEFORE:&str=r#"pub fn add(a:u32,b:u32)->Option<u32>{None}
fn main(){use std::io::Read;let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();for line in s.lines(){let v:Vec<u32>=line.split_whitespace().map(|x|x.parse().unwrap()).collect();match add(v[0],v[1]){Some(n)=>println!("some:{n}"),None=>println!("none")}}}
"#;
pub const AFTER:&str=r#"pub fn add(a:u32,b:u32)->Option<u32>{a.checked_add(b)}
fn main(){use std::io::Read;let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();for line in s.lines(){let v:Vec<u32>=line.split_whitespace().map(|x|x.parse().unwrap()).collect();match add(v[0],v[1]){Some(n)=>println!("some:{n}"),None=>println!("none")}}}
"#;
pub fn clock()->u64 {100}
pub fn compile_native(source:&Path,out:&Path) {
    // Test-harness construction only. Central executes this code, never agents.
    let status=std::process::Command::new("rustc").args(["--edition=2021","-Awarnings","-o"]).arg(out).arg(source).status().unwrap();
    assert!(status.success());
}
pub fn file_hash(path:&Path)->String {
    let d=Sha256::digest(std::fs::read(path).unwrap());
    format!("sha256:{}",d.iter().map(|b|format!("{b:02x}")).collect::<String>())
}

/// Owns registration material, never a process observation. New child processes
/// must create it again at their current epoch, then actually run both artifacts.
pub struct NativeProbeFixture {
    pub snapshot:DgclExecutionSnapshot,
    pub lease:DgclEvidenceLease,
    pub target_path:PathBuf,
    pub program:PathBuf,
    executable_digest:String,
    tool_receipt:UntrustedReceipt,
    permit:ExecutionPermit,
    scope:String,
    epoch:u64,
    clock:fn()->u64,
}
impl NativeProbeFixture {
    pub fn registered(plan:&CodingProbePlan,snapshot:DgclExecutionSnapshot,target_path:PathBuf,program:PathBuf,issuer:&ReceiptIssuer,verifier:&ReceiptVerifier)->Self {
        Self::registered_at(plan,snapshot,target_path,program,issuer,verifier,100,clock)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn registered_at(plan:&CodingProbePlan,snapshot:DgclExecutionSnapshot,target_path:PathBuf,program:PathBuf,issuer:&ReceiptIssuer,verifier:&ReceiptVerifier,epoch:u64,clock:fn()->u64)->Self {
        assert_eq!(snapshot.source_revision,stable_sha256(SOURCE));
        let executable_digest=file_hash(&program);
        let registration=ProbeToolRegistration{program:program.to_str().unwrap(),executable_digest:&executable_digest,target_path:&target_path,target_ref:"src/lib.rs",snapshot:&snapshot};
        let expires=epoch.checked_add(300).unwrap();
        let tool_receipt=issuer.issue(ReceiptClass::ToolExecution,SubjectRevision::checked(&snapshot.source_revision).unwrap(),ReceiptScope::checked(registration.receipt_scope(plan)).unwrap(),registration.payload_digest(plan),epoch,Some(expires),None).unwrap();
        let scope=registration.execution_scope(plan);
        let permit=test_permit_at(&scope,issuer,verifier,epoch,expires);
        let lease=issue_dgcl_evidence_lease(&snapshot,epoch,300).unwrap();
        Self{snapshot,lease,target_path,program,executable_digest,tool_receipt,permit,scope,epoch,clock}
    }
    pub fn with_invocation<R>(&self,verifier:&ReceiptVerifier,consume:impl FnOnce(ProbeInvocation<'_>)->R)->R {
        let authority=ExecutionPermitContext{principal:self.permit.principal_binding(),action:Action::Test,scope:&self.scope,source_revision:&self.snapshot.source_revision,authority_revision:AuthorityRevision(1),now_epoch:self.epoch,revocation_revision:0,revoked_permit_digests:&[],caller_origin:CallerOrigin::HostVerifiedUser,trusted_host_fingerprint:verifier.key_fingerprint()};
        consume(ProbeInvocation{source:SOURCE,registration:ProbeToolRegistration{program:self.program.to_str().unwrap(),executable_digest:&self.executable_digest,target_path:&self.target_path,target_ref:"src/lib.rs",snapshot:&self.snapshot},lease:&self.lease,tool_receipt:&self.tool_receipt,tool_verifier:verifier,permit:&self.permit,authority:&authority,clock:self.clock,timeout_ms:5000})
    }
}
#[allow(clippy::too_many_arguments)]
pub fn observe<'v>(plan:&CodingProbePlan,snapshot:&DgclExecutionSnapshot,target:&Path,program:&Path,issuer:&ReceiptIssuer,verifier:&ReceiptVerifier,resources:&'v ResourceReservationView<'_>)->OwnedCodingProbeObservation<'v> {
    // Explicit test-host grant, configured BEFORE the actual native effects.
    // Historical baseline observations deliberately carry no live-root provider.
    let authority=if target.ends_with("src/lib.rs") {
        let root=target.parent().unwrap().parent().unwrap();
        Some(Box::new(snapshot_authority(root,issuer,verifier,100,AuthorityRevision(1))) as Box<dyn lc631_analysis::declared_build_capture::CurrentSnapshotAuthority>)
    } else {None};
    NativeProbeFixture::registered(plan,snapshot.clone(),target.to_path_buf(),program.to_path_buf(),issuer,verifier)
        .with_invocation(verifier,|invocation|match authority {
            Some(authority)=>observe_current_with_build_authority(plan,invocation,resources,authority).unwrap(),
            None=>observe_current(plan,invocation,resources).unwrap(),
        })
}
/// Synthetic fixture provider, not production permission creation or host proof.
/// Every current() supplies an explicit Test context; late cuts are observable.
pub struct FixtureSnapshotAuthority {
    pub permit:ExecutionPermit,
    pub principal:PrincipalBinding,
    pub scope:String,
    pub host:String,
    pub source:String,
    pub epoch:u64,
    pub revision:AuthorityRevision,
    pub revoked:Vec<String>,
    pub calls:usize,
    pub reject_after:Option<usize>,
}
impl lc631_analysis::declared_build_capture::CurrentSnapshotAuthority for FixtureSnapshotAuthority {
    fn current(&mut self)->Result<lc631_analysis::declared_build_capture::SnapshotAuthorityView<'_>,lc631_analysis::declared_build_capture::SnapshotCaptureError> {
        self.calls+=1;
        if self.reject_after.is_some_and(|limit|self.calls>limit) {return Err(lc631_analysis::declared_build_capture::SnapshotCaptureError::AuthorityUnavailable);}
        Ok(lc631_analysis::declared_build_capture::SnapshotAuthorityView{permit:&self.permit,expected_principal:&self.principal,
            context:ExecutionPermitContext{principal:&self.principal,action:Action::Test,scope:&self.scope,source_revision:&self.source,authority_revision:self.revision,now_epoch:self.epoch,revocation_revision:0,revoked_permit_digests:&self.revoked,caller_origin:CallerOrigin::HostVerifiedUser,trusted_host_fingerprint:&self.host}})
    }
}
pub fn snapshot_authority(root:&Path,issuer:&ReceiptIssuer,verifier:&ReceiptVerifier,epoch:u64,revision:AuthorityRevision)->FixtureSnapshotAuthority {
    use lc631_analysis::{build_dgcl_pipeline,DgclBackendSet,dgcl_repository_root_digest,declared_build_capture::*};
    let pipeline=build_dgcl_pipeline(SOURCE,DgclBackendSet{english:None,japanese:None}).unwrap();
    let binding=SourceReadBinding::checked(root,SOURCE,&pipeline,
        declared_capture_digest(&dgcl_repository_root_digest(root).unwrap()).unwrap(),
        declared_capture_digest(&pipeline.program_ir.program_digest).unwrap(),
        declared_capture_digest(&pipeline.completion_plan.plan_digest).unwrap()).unwrap();
    let scope=binding.execution_scope();
    let permit=test_permit_revision(&scope,issuer,verifier,epoch,epoch+300,revision);
    let principal=permit.principal_binding().clone();
    FixtureSnapshotAuthority{permit,principal,scope,host:verifier.key_fingerprint().into(),source:stable_sha256(SOURCE),epoch,revision,revoked:Vec::new(),calls:0,reject_after:None}
}
fn test_permit_at(scope:&str,issuer:&ReceiptIssuer,verifier:&ReceiptVerifier,epoch:u64,expires:u64)->ExecutionPermit {
    test_permit_revision(scope,issuer,verifier,epoch,expires,AuthorityRevision(1))
}
fn test_permit_revision(scope:&str,issuer:&ReceiptIssuer,verifier:&ReceiptVerifier,epoch:u64,expires:u64,revision:AuthorityRevision)->ExecutionPermit {
    let parent=stable_sha256("TLCX31/32 local fixture ingress, not actual host observation");
    let event=AuthorityEvent{action:Action::Test,source:AuthoritySourceKind::VerifiedHostUserSpan,polarity:DeonticPolarity::Grant,span:SourceSpan{start:0,end:SOURCE.len()},scope:scope.into()};
    let subject=SubjectRevision::checked(stable_sha256(SOURCE)).unwrap();
    let receipt_scope=ReceiptScope::checked(format!("authority/test/{scope}")).unwrap();
    let payload=authority_event_payload_digest(&event);
    let raw=issuer.issue(ReceiptClass::Authority,subject.clone(),receipt_scope.clone(),payload.clone(),epoch,Some(expires),Some(parent.clone())).unwrap();
    let verified=verifier.verify(raw,&ReceiptPolicy::exact(ReceiptClass::Authority,subject,receipt_scope,payload,epoch).with_parent(&parent),&mut ReplayGuard::default()).unwrap();
    ExecutionPermit::from_verified_user_event(VerifiedAuthorityEvent::checked(event,verified).unwrap(),SOURCE,revision,epoch,expires,0).unwrap()
}
