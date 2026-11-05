//! External fixture builder/signing domain. No producer or observed-owner stub.
//! Central builds these real native artifacts before invoking the adapter API.
use lc631_analysis::{build_dgcl_execution_snapshot,issue_dgcl_evidence_lease,
    DgclEvidenceLease,DgclExecutionSnapshot,DgclTargetIdentity};
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_runtime::{contextual_repair::{attempt::*,execution::ProbeToolRegistration,
    measurement::*},resource::*};
use std::{path::PathBuf,time::{Duration,SystemTime,UNIX_EPOCH}};
#[path="../tlcx31_coding_support/mod.rs"]
#[allow(dead_code,reason="Only real native artifact compilation and file hashing are reused; no legacy observation owner is reconstructed.")]
mod artifacts;

pub const SOURCE:&str="Please edit the source and test the package.";
pub fn clock()->u64 {100}
#[derive(Clone,Copy)] pub enum Output { Baseline, Correct, Malformed, Failed }
#[derive(Clone,Copy,Debug)] pub enum Cut {
    None, PostUnavailable, PostRevoked, PostClockRollback, PostFrame,
    PostCancel, PostDeadline, PublicationUnavailable,
}

pub struct Fixture {
    pub target:PathBuf,
    pub program:PathBuf,
    pub marker:PathBuf,
    pub snapshot:DgclExecutionSnapshot,
    pub lease:DgclEvidenceLease,
    pub plan:CodingProbePlan,
    pub frame:Digest32,
    pub principal:PrincipalBinding,
    pub host:String,
    binary:String,
    receipt:UntrustedReceipt,
    issuer:ReceiptIssuer,
    verifier:ReceiptVerifier,
}
impl Fixture {
    pub fn new(output:Output)->Self {
        let stamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("tlcx31-native-attempt-{}-{stamp}",std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        eprintln!("retained owned native-attempt fixture {}",root.display());
        let target=root.join("candidate.rs");
        let marker=root.join("actual-child.marker");
        let literal=format!("{:?}",marker.to_str().unwrap());
        let body=match output {
            Output::Baseline=>"for _ in s.lines(){println!(\"none\");}",
            Output::Correct=>"for l in s.lines(){let v:Vec<u32>=l.split_whitespace().map(|x|x.parse().unwrap()).collect();match v[0].checked_add(v[1]){Some(n)=>println!(\"some:{n}\"),None=>println!(\"none\")}}",
            Output::Malformed=>"println!(\"invalid actual native output\");",
            Output::Failed=>"std::process::exit(7);",
        };
        let code=format!("fn main(){{use std::io::Read;let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();std::fs::write({literal},b\"executed\").unwrap();{body}}}");
        std::fs::write(&target,&code).unwrap();
        let program=root.join(format!("candidate{}",std::env::consts::EXE_SUFFIX));
        artifacts::compile_native(&target,&program);
        let binary=artifacts::file_hash(&program);
        let snapshot=build_dgcl_execution_snapshot(SOURCE,code.as_bytes(),&stable_sha256("fixed declared profile"),
            &stable_sha256("independent four-probe oracle"),&stable_sha256("historical authority"),
            vec![DgclTargetIdentity{target_ref:"src/lib.rs".into(),target_digest:stable_sha256(&code)}]).unwrap();
        let lease=issue_dgcl_evidence_lease(&snapshot,100,300).unwrap();
        let plan=CodingProbePlan::checked_add(digest(b"fixed checked-add goal"),digest(b"fixed probe policy")).unwrap();
        let frame=digest(b"externally selected command frame, never inferred from seed");
        let issuer=ReceiptIssuer::from_key_bytes("tlcx31-attempt-external-fixture",&[123;32]).unwrap();
        let verifier=ReceiptVerifier::from_key_bytes("tlcx31-attempt-external-fixture",&[123;32]).unwrap();
        let registration=ProbeToolRegistration{program:program.to_str().unwrap(),executable_digest:&binary,
            target_path:&target,target_ref:"src/lib.rs",snapshot:&snapshot};
        let receipt=issuer.issue(ReceiptClass::ToolExecution,SubjectRevision::checked(&snapshot.source_revision).unwrap(),
            ReceiptScope::checked(registration.receipt_scope(&plan)).unwrap(),registration.payload_digest(&plan),100,Some(400),None).unwrap();
        let permission=external_permit(&native_attempt_scope(&plan,&registration,ProbePurpose::Current,frame),
            Action::Test,&issuer,&verifier);
        let principal=permission.principal_binding().clone();
        let host=verifier.key_fingerprint().to_owned();
        Self{target,program,marker,snapshot,lease,plan,frame,principal,host,binary,receipt,issuer,verifier}
    }
    pub fn registration(&self)->ProbeToolRegistration<'_> {
        ProbeToolRegistration{program:self.program.to_str().unwrap(),executable_digest:&self.binary,
            target_path:&self.target,target_ref:"src/lib.rs",snapshot:&self.snapshot}
    }
    pub fn input(&self)->ProbeAttemptInput<'_> {
        ProbeAttemptInput{source:SOURCE,registration:self.registration(),lease:&self.lease,
            tool_receipt:&self.receipt,tool_verifier:&self.verifier,clock,timeout_ms:5000,
            origin:ProbeExpectedOrigin{principal:&self.principal,host_fingerprint:&self.host,
                caller_origin:CallerOrigin::HostVerifiedUser,frame:self.frame,not_before_epoch:100,revocation_floor:0}}
    }
    pub fn provider<'a>(&self,purpose:ProbePurpose,action:Action,cut:Cut,run:&'a RunResources,cancel:Cancellation)->Provider<'a> {
        let scope=native_attempt_scope(&self.plan,&self.registration(),purpose,self.frame);
        self.provider_for_scope(scope,action,cut,run,cancel)
    }
    pub fn provider_for_scope<'a>(&self,scope:String,action:Action,cut:Cut,run:&'a RunResources,cancel:Cancellation)->Provider<'a> {
        let permit=external_permit(&scope,action,&self.issuer,&self.verifier);
        Provider{permit,scope,source:self.snapshot.source_revision.clone(),host:self.host.clone(),
            frame:self.frame,epoch:100,revoked:Vec::new(),cut,run,cancel,calls:Vec::new(),
            marker:self.marker.clone(),principal:self.principal.clone(),caller_origin:CallerOrigin::HostVerifiedUser}
    }
}

/// Only this setup function possesses the fixture signing key. The production
/// API receives an already externally issued permit and cannot mint another.
fn external_permit(scope:&str,action:Action,issuer:&ReceiptIssuer,verifier:&ReceiptVerifier)->ExecutionPermit {
    let parent=stable_sha256("synthetic external fixture ingress; not actual host or human independence");
    let event=AuthorityEvent{action,source:AuthoritySourceKind::VerifiedHostUserSpan,
        polarity:DeonticPolarity::Grant,span:SourceSpan{start:0,end:SOURCE.len()},scope:scope.into()};
    let action_name=if action==Action::Test {"test"}else{"edit"};
    let subject=SubjectRevision::checked(stable_sha256(SOURCE)).unwrap();
    let scope=ReceiptScope::checked(format!("authority/{action_name}/{scope}")).unwrap();
    let payload=authority_event_payload_digest(&event);
    let raw=issuer.issue(ReceiptClass::Authority,subject.clone(),scope.clone(),payload.clone(),100,Some(400),Some(parent.clone())).unwrap();
    let verified=verifier.verify(raw,&ReceiptPolicy::exact(ReceiptClass::Authority,subject,scope,payload,100)
        .with_parent(&parent),&mut ReplayGuard::default()).unwrap();
    ExecutionPermit::from_verified_user_event(VerifiedAuthorityEvent::checked(event,verified).unwrap(),
        SOURCE,AuthorityRevision(1),100,400,0).unwrap()
}

pub struct Provider<'a> {
    pub permit:ExecutionPermit,
    pub scope:String,
    pub source:String,
    pub host:String,
    pub frame:Digest32,
    pub epoch:u64,
    pub revoked:Vec<String>,
    pub cut:Cut,
    pub run:&'a RunResources,
    pub cancel:Cancellation,
    pub calls:Vec<(ProbePurpose,ProbeAuthorityPhase)>,
    pub marker:PathBuf,
    pub principal:PrincipalBinding,
    pub caller_origin:CallerOrigin,
}
impl CurrentProbeAuthority for Provider<'_> {
    fn current(&mut self,request:ProbeAuthorityRequest<'_>)->Result<ProbeAuthorityView<'_>,ProbeError> {
        self.calls.push((request.purpose,request.phase));
        match request.phase {
            ProbeAuthorityPhase::BeforeSpawn=>assert!(!self.marker.exists(),"provider is sampled before the actual child"),
            ProbeAuthorityPhase::AfterChild=>{
                assert!(self.marker.exists(),"post sample requires actual child effects, not a fake report");
                match self.cut {
                    Cut::PostUnavailable=>return Err(ProbeError::Unauthorized),
                    Cut::PostRevoked=>self.revoked.push(self.permit.digest().into()),
                    Cut::PostClockRollback=>self.epoch=99,
                    Cut::PostFrame=>self.frame=digest(b"different current external frame"),
                    Cut::PostCancel=>self.cancel.cancel(),
                    Cut::PostDeadline=>{
                        self.run.narrow_deadline(Deadline::after(Duration::from_nanos(1)).unwrap()).unwrap();
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Cut::None|Cut::PublicationUnavailable=>{}
                }
            }
            ProbeAuthorityPhase::BeforePublication=>{
                if matches!(self.cut,Cut::PublicationUnavailable) {return Err(ProbeError::Unauthorized);}
            }
        }
        Ok(ProbeAuthorityView{permit:&self.permit,frame:self.frame,context:ExecutionPermitContext{
            principal:&self.principal,action:Action::Test,scope:&self.scope,source_revision:&self.source,
            authority_revision:AuthorityRevision(1),now_epoch:self.epoch,revocation_revision:0,
            revoked_permit_digests:&self.revoked,caller_origin:self.caller_origin,trusted_host_fingerprint:&self.host}})
    }
}
pub fn with_resources(operation:impl FnOnce(&RunResources,&ResourceReservationView<'_>,Cancellation)) {
    let limits=ResourceLimits::uniform(128*1024*1024).unwrap();
    let broker=ResourceBroker::new(limits).unwrap();
    let cancel=Cancellation::default();
    let mut run=broker.begin_run(1,lc631_core::context::SourceRevision::of(SOURCE).digest(),limits,
        Deadline::after(Duration::from_secs(60)).unwrap(),cancel.clone()).unwrap();
    let mut reservation=run.reserve(&[q(ResourceKind::Ram,32*1024*1024),q(ResourceKind::Output,1024*1024),
        q(ResourceKind::Child,2),q(ResourceKind::Cleanup,1024*1024)]).unwrap();
    {let view=reservation.lease();operation(&run,&view,cancel);}
    // Cancel/expiry are intentionally historical failures; release still must
    // verify that actual buffers/claims were dropped by the attempt owner.
    reservation.release_confirmed().unwrap();run.close_confirmed().unwrap();
}
fn q(kind:ResourceKind,n:u64)->ResourceRequest {ResourceRequest::new(kind,ResourceAmount::checked(n).unwrap())}
