//! 合成外部issuerと実repair fixture。checked ownerは復元しない。
//! Synthetic external issuer plus actual repair fixture; never restores owners.
use super::{coding_support as coding, file_fixture, fresh_support};
use lc631_analysis::{build_dgcl_pipeline, capture_dgcl_cargo_snapshot_with_runner,
    DgclBackendSet, DgclExecutionSnapshot};
use lc631_analysis::declared_build_capture::CurrentSnapshotAuthority;
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_runtime::{contextual_repair::{attempt::*, execution::ProbeInvocation,
    measurement::CodingProbePlan, snapshot_gate::CodingBuildPort},
    contextual_resume::{production_contract::*, transition::*}, journal::*, resource::*};
use lc631_substrate::{codec::{encode_frame, SectionInput, FRAME_DOMAIN}, Digest32};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::{Mutex, OnceLock}, time::{Duration, SystemTime}};

pub static SERIAL: Mutex<()> = Mutex::new(());
static ACTUAL_ROOT: OnceLock<PathBuf> = OnceLock::new();
pub const TAGS: [u16; 6] = [0x3200, 0x3201, 0x3202, 0x3203, 0x3204, 0x3205];
pub const RAM: u64 = 64 * 1024 * 1024;
pub fn d(bytes: &[u8]) -> Digest32 { Digest32::from_bytes(Sha256::digest(bytes).into()) }
pub fn parse(text: &str) -> Digest32 {
    let hex = text.strip_prefix("sha256:").unwrap();
    let mut out = [0; 32];
    assert_eq!(hex.len(), 64);
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
    }
    Digest32::from_bytes(out)
}
pub fn frame_digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(bytes[32..64].try_into().unwrap())
}
pub fn rehash(bytes: &mut [u8]) {
    bytes[32..64].fill(0);
    let mut hash = Sha256::new(); hash.update(FRAME_DOMAIN); hash.update(&*bytes);
    bytes[32..64].copy_from_slice(&hash.finalize());
}
pub fn signing() -> (ReceiptIssuer, ReceiptVerifier) {
    // Same EXPLICIT synthetic trust domain as actual setup, outside adapter.
    (ReceiptIssuer::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap(),
     ReceiptVerifier::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap())
}
pub fn clock() -> u64 { 200 }
pub fn plan() -> CodingProbePlan {
    CodingProbePlan::checked_add(Digest32::from_bytes([34; 32]),
        Digest32::from_bytes([35; 32])).unwrap()
}
pub fn funded<R>(consume: impl for<'v> FnOnce(&RunResources, &ResourceReservationView<'v>) -> R) -> R {
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker.begin_run(32, d(coding::SOURCE.as_bytes()), limits,
        Deadline::after(Duration::from_secs(90)).unwrap(), Cancellation::default()).unwrap();
    let request = |kind, n| ResourceRequest::new(kind, ResourceAmount::checked(n).unwrap());
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, RAM),
        request(ResourceKind::Output, 1024 * 1024), request(ResourceKind::Disk, 64 * 1024 * 1024),
        request(ResourceKind::Child, 2), request(ResourceKind::Cleanup, 2)]).unwrap();
    let result = { let view = reservation.lease(); consume(&run, &view) };
    reservation.release_confirmed().unwrap(); run.close_confirmed().unwrap(); result
}

/// Only UNTRUSTED reported DTO bytes cross persistence. Actual application was
/// captured by the existing real coordinator's borrowed token callback.
pub struct PacketFixture {
    pub root: PathBuf,
    pub raw: Value,
    pub parts: Vec<(u16, Vec<u8>)>,
    pub pins: [Digest32; 15],
}
impl PacketFixture {
    pub fn actual() -> Self {
        let root = ACTUAL_ROOT.get_or_init(|| {
            let root = file_fixture::root();
            std::fs::write(root.join("tlcx32-attempt-owner"), b"owned test evidence; retain on failure\n").unwrap();
            fresh_support::create_checkpoint(&root); root
        }).clone();
        let raw: Value = serde_json::from_slice(&fresh_support::read_owned(&root.join("history.json"), 65536)).unwrap();
        let request = &raw["request"]; let app = &raw["application"];
        let plan = plan();
        let program = build_dgcl_pipeline(coding::SOURCE,
            DgclBackendSet { english: None, japanese: None }).unwrap();
        let before = &raw["baseline_snapshot"];
        let native = |name: &str| root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        // Order is the proposed finite packet-v1 contract; origin is Given,
        // not a provider-authentication or Authority claim.
        let pins = [parse(request["repository_root_digest"].as_str().unwrap()),
            d(coding::SOURCE.as_bytes()), d(coding::SOURCE.as_bytes()), Digest32::from_bytes([33; 32]),
            parse(request["plan_digest"].as_str().unwrap()), plan.goal(), plan.policy(), plan.digest(),
            d(request["target_ref"].as_str().unwrap().as_bytes()),
            parse(before["build_input_digest"].as_str().unwrap()),
            parse(app["after_snapshot"]["build_input_digest"].as_str().unwrap()),
            parse(&coding::file_hash(&native("baseline"))), parse(&coding::file_hash(&native("current"))),
            d(b"TLCX32/explicit-synthetic-provider-origin/v1"), parse(&program.program_ir.program_digest)];
        let descriptor = pins.iter().flat_map(|pin| pin.as_bytes().iter().copied()).collect();
        let parts = vec![(TAGS[0], descriptor), (TAGS[1], coding::SOURCE.as_bytes().to_vec()),
            (TAGS[2], serde_json::to_vec(&json!({"schema_version":"epistesys-coding-resume-request.v1","request":request})).unwrap()),
            (TAGS[3], serde_json::to_vec(&json!({"schema_version":"epistesys-coding-resume-application.v1","application":app})).unwrap()),
            (TAGS[4], serde_json::to_vec(&json!({"schema_version":"epistesys-coding-resume-attestation.v1","receipt":raw["transition_attestation"]})).unwrap()),
            (TAGS[5], fresh_support::read_owned(&root.join("transition.bin"), 65536))];
        Self { root, raw, parts, pins }
    }
    pub fn bytes(&self) -> Vec<u8> { self.encode(&self.parts) }
    pub fn encode(&self, parts: &[(u16, Vec<u8>)]) -> Vec<u8> {
        let sections = parts.iter().map(|(tag, payload)| SectionInput {
            tag: *tag, version: 1, flags: 0, count: 1, payload }).collect::<Vec<_>>();
        encode_frame(&sections).unwrap()
    }
    pub fn origin(&self, bytes: &[u8]) -> CodingResumeOrigin {
        CodingResumeOrigin::checked(self.pins, frame_digest(bytes)).unwrap()
    }
    pub fn target(&self) -> PathBuf { self.root.join("src/lib.rs") }
}

pub struct Grant { permit: ExecutionPermit, scope: String }
pub struct ProbeAuthority {
    grants: [Grant; 2], host: String, source: String, frame: Digest32,
    pub calls: Vec<(ProbePurpose, ProbeAuthorityPhase)>,
    pub reject: Option<(ProbePurpose, ProbeAuthorityPhase)>,
    pub foreign_frame: bool, pub rollback: bool, pub foreign_scope: bool,
    pub revoked: Vec<String>,
}
impl ProbeAuthority {
    pub fn revoke_current(&mut self) { self.revoked.push(self.grants[1].permit.digest().to_owned()); }
}
fn permit(scope: &str, issuer: &ReceiptIssuer, verifier: &ReceiptVerifier) -> ExecutionPermit {
    let parent = stable_sha256("TLCX32 external TEST FIXTURE ingress; not actual host proof");
    let event = AuthorityEvent { action: Action::Test, source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant, span: SourceSpan { start: 0, end: coding::SOURCE.len() }, scope: scope.into() };
    let subject = SubjectRevision::checked(stable_sha256(coding::SOURCE)).unwrap();
    let rs = ReceiptScope::checked(format!("authority/test/{scope}")).unwrap();
    let payload = authority_event_payload_digest(&event);
    let raw = issuer.issue(ReceiptClass::Authority, subject.clone(), rs.clone(), payload.clone(),
        200, Some(500), Some(parent.clone())).unwrap();
    let verified = verifier.verify(raw, &ReceiptPolicy::exact(ReceiptClass::Authority, subject, rs,
        payload, 200).with_parent(&parent), &mut ReplayGuard::default()).unwrap();
    ExecutionPermit::from_verified_user_event(VerifiedAuthorityEvent::checked(event, verified).unwrap(),
        coding::SOURCE, AuthorityRevision(2), 200, 500, 0).unwrap()
}
impl CurrentProbeAuthority for ProbeAuthority {
    fn current(&mut self, request: ProbeAuthorityRequest<'_>) -> Result<ProbeAuthorityView<'_>,
        lc631_runtime::contextual_repair::measurement::ProbeError> {
        use lc631_runtime::contextual_repair::measurement::ProbeError;
        self.calls.push((request.purpose, request.phase));
        if self.reject == Some((request.purpose, request.phase)) { return Err(ProbeError::Unauthorized); }
        let index = match request.purpose { ProbePurpose::Baseline => 0, ProbePurpose::Current => 1 };
        let grant = &self.grants[index];
        if request.execution_scope != grant.scope || request.source_revision != self.source
            || request.frame != self.frame { return Err(ProbeError::BindingMismatch); }
        Ok(ProbeAuthorityView { permit: &grant.permit,
            frame: if self.foreign_frame { Digest32::from_bytes([99;32]) } else { self.frame },
            context: ExecutionPermitContext { principal: grant.permit.principal_binding(), action: Action::Test,
                scope: if self.foreign_scope { "foreign/test/scope" } else { &grant.scope },
                source_revision: &self.source, authority_revision: AuthorityRevision(2),
                now_epoch: if self.rollback { 199 } else { 200 }, revocation_revision: 0,
                revoked_permit_digests: &self.revoked, caller_origin: CallerOrigin::HostVerifiedUser,
                trusted_host_fingerprint: &self.host } })
    }
}
fn input<'a>(invocation: ProbeInvocation<'a>, principal: &'a PrincipalBinding,
    host: &'a str, frame: Digest32) -> ProbeAttemptInput<'a> {
    ProbeAttemptInput { source: invocation.source, registration: invocation.registration,
        lease: invocation.lease, tool_receipt: invocation.tool_receipt, tool_verifier: invocation.tool_verifier,
        clock: clock, timeout_ms: 5000, origin: ProbeExpectedOrigin { principal,
            host_fingerprint: host, caller_origin: CallerOrigin::HostVerifiedUser,
            frame, not_before_epoch: 200, revocation_floor: 0 } }
}

pub fn configured<R>(fixture: &PacketFixture, run: &RunResources, view: &ResourceReservationView<'_>,
    consume: impl for<'a, 'ctx, 'runner> FnOnce(CodingResumeAttemptInput<'a>, ProbeAttemptInput<'a>, CodingBuildPort<'ctx,'runner>,
        ProbeAuthority, coding::FixtureSnapshotAuthority, &'ctx lc631_analysis::DgclPipelineReport) -> R) -> R {
    let bytes = fixture.bytes(); let origin = fixture.origin(&bytes);
    let checked = decode_coding_resume_packet(UntrustedCodingResumePacket::borrowed(&bytes),
        &origin, CodingResumePacketLimits::finite_v1(), view).unwrap();
    let (issuer, verifier) = signing();
    let history = inspect_transition_chain(&[checked.segment()], None, JournalChainLimits::default(),
        TransitionEvidence { request: checked.request(), application: checked.application(),
            transition_attestation: checked.attestation(), verifier: &verifier, now_epoch: 200 }, view).unwrap();
    drop(checked);
    let pipeline = build_dgcl_pipeline(coding::SOURCE, DgclBackendSet { english: None, japanese: None }).unwrap();
    let plan = plan(); let mut reader = file_fixture::Inputs { run };
    let mut build_authority = coding::snapshot_authority(&fixture.root, &issuer, &verifier, 200, AuthorityRevision(2));
    let current = { let authority = build_authority.current().unwrap();
        capture_dgcl_cargo_snapshot_with_runner(&fixture.root, coding::SOURCE, &pipeline,
            &authority.context, &mut reader).unwrap() };
    let baseline: DgclExecutionSnapshot = serde_json::from_value(fixture.raw["baseline_snapshot"].clone()).unwrap();
    let native = |name: &str| fixture.root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let before = coding::NativeProbeFixture::registered_at(&plan, baseline, fixture.root.join("baseline.rs"),
        native("baseline"), &issuer, &verifier, 200, clock);
    let after = coding::NativeProbeFixture::registered_at(&plan, current, fixture.target(),
        native("current"), &issuer, &verifier, 200, clock);
    let frame = frame_digest(&bytes);
    let before_scope = before.with_invocation(&verifier, |i| native_attempt_scope(&plan, &i.registration, ProbePurpose::Baseline, frame));
    let after_scope = after.with_invocation(&verifier, |i| native_attempt_scope(&plan, &i.registration, ProbePurpose::Current, frame));
    let grants = [Grant { permit: permit(&before_scope, &issuer, &verifier), scope: before_scope },
        Grant { permit: permit(&after_scope, &issuer, &verifier), scope: after_scope }];
    let expected = grants[0].permit.principal_binding().clone();
    assert_eq!(&expected, grants[1].permit.principal_binding());
    let authority = ProbeAuthority { grants, host: verifier.key_fingerprint().into(), source: stable_sha256(coding::SOURCE),
        frame, calls: Vec::new(), reject: None, foreign_frame: false, rollback: false,
        foreign_scope:false, revoked:Vec::new() };
    let port = CodingBuildPort::checked(&fixture.root, coding::SOURCE, &pipeline, fixture.pins[0],
        fixture.pins[14], fixture.pins[4], &mut reader).unwrap();
    before.with_invocation(&verifier, |b| after.with_invocation(&verifier, |c| {
        // Duplicate only borrowed registration metadata, never an observed owner.
        let current_again = ProbeAttemptInput { source: c.source,
            registration: lc631_runtime::contextual_repair::execution::ProbeToolRegistration {
                program:c.registration.program, executable_digest:c.registration.executable_digest,
                target_path:c.registration.target_path, target_ref:c.registration.target_ref,
                snapshot:c.registration.snapshot }, lease:c.lease, tool_receipt:c.tool_receipt,
            tool_verifier:c.tool_verifier, clock, timeout_ms:5000,
            origin:ProbeExpectedOrigin { principal:&expected, host_fingerprint:verifier.key_fingerprint(),
                caller_origin:CallerOrigin::HostVerifiedUser, frame, not_before_epoch:200, revocation_floor:0 } };
        consume(CodingResumeAttemptInput { history,
            context: context::ContextRevision::original(context::SourceRevision::of(coding::SOURCE)),
            comparison_scope: fixture.pins[3], plan: &plan, origin: &origin,
            baseline: input(b, &expected, verifier.key_fingerprint(), frame),
            current: input(c, &expected, verifier.key_fingerprint(), frame) }, current_again, port, authority, build_authority, &pipeline)
    }))
}

/// Observable bytes/mtimes, not a claim to intercept all OS writes or network.
pub fn protected_state(root: &std::path::Path) -> Vec<(PathBuf, Digest32, SystemTime)> {
    fn walk(root: &std::path::Path, out: &mut Vec<(PathBuf, Digest32, SystemTime)>) {
        use std::io::Read;
        for entry in std::fs::read_dir(root).unwrap() {
            assert!(out.len() < 4096);
            let path = entry.unwrap().path(); let metadata = std::fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() { walk(&path, out); }
            else { assert!(metadata.len() <= 64*1024*1024);
                let mut file=std::fs::File::open(&path).unwrap(); let mut hash=Sha256::new();
                let mut buffer=[0;8192]; let mut total=0_usize;
                loop {let n=file.read(&mut buffer).unwrap(); if n==0 {break;}
                    total=total.checked_add(n).unwrap(); assert!(total<=64*1024*1024); hash.update(&buffer[..n]);}
                out.push((path.clone(), Digest32::from_bytes(hash.finalize().into()), metadata.modified().unwrap())); }
        }
    }
    let mut out = Vec::new(); walk(root, &mut out); out.sort_by(|a,b|a.0.cmp(&b.0)); out
}
