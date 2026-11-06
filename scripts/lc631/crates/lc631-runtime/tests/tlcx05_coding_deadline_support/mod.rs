//! 日本語: 中央実行用の有限fixture。実host証拠やcompiler成功を作らない。
//! English: Bounded fixtures for central execution; no real-host/compiler claim.
#![forbid(unsafe_code)]
use lc631_analysis::{declared_build_capture::*, *};
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_runtime::resource::*;
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const SOURCE: &str = "Please edit the source and test the package.";
pub fn source_digest(source: &str) -> Digest32 {
    context::SourceRevision::of(source).digest()
}
pub fn q(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
pub fn requests() -> [ResourceRequest; 5] {
    [
        q(ResourceKind::Ram, 64 * 1024 * 1024),
        q(ResourceKind::Disk, 1024 * 1024),
        q(ResourceKind::Output, 8 * 1024 * 1024),
        q(ResourceKind::Child, 2),
        q(ResourceKind::Cleanup, 2),
    ]
}
pub fn resources(source: &str) -> (ResourceBroker, RunResources) {
    let caps = ResourceLimits::checked([
        128 * 1024 * 1024,
        16 * 1024 * 1024,
        16 * 1024 * 1024,
        0,
        2,
        2,
    ])
    .unwrap();
    let broker = ResourceBroker::new(caps).unwrap();
    // Match CommandIo's existing long deadline; external narrowing is additive.
    let run = broker
        .begin_run(
            1,
            source_digest(source),
            caps,
            Deadline::after(Duration::from_secs(1800)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    (broker, run)
}
pub fn child_args(marker: &Path) -> Vec<&str> {
    // --skip accepts an arbitrary test-name filter. Here it transports a private
    // marker path to the explicitly selected child test without a new CLI flag.
    vec![
        "--exact",
        "tlcx05_child_wait_fixture",
        "--skip",
        marker.to_str().unwrap(),
        "--nocapture",
    ]
}
pub fn child_request<'a>(program: &'a Path, args: &'a [&'a str]) -> BoundedToolRequest<'a> {
    BoundedToolRequest {
        program: program.to_str().unwrap(),
        args,
        stdin: None,
        timeout_ms: 12_000,
        stdout_limit: 1024,
        stderr_limit: 1024,
    }
}

pub struct CaptureFixture {
    pub root: PathBuf,
    pub pipeline: DgclPipelineReport,
    root_digest: Digest32,
    scope: String,
    permit: ExecutionPermit,
    host: String,
}
impl CaptureFixture {
    pub fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tlcx05-deadline-{}-{nonce}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='tlcx05_deadline'\nversion='0.0.0'\nedition='2021'\n",
        )
        .unwrap();
        std::fs::write(root.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n").unwrap();
        eprintln!("retained owned deadline fixture {}", root.display());
        let pipeline = build_dgcl_pipeline(
            SOURCE,
            DgclBackendSet {
                english: None,
                japanese: None,
            },
        )
        .unwrap();
        let root_digest =
            declared_capture_digest(&dgcl_repository_root_digest(&root).unwrap()).unwrap();
        let binding = SourceReadBinding::checked(
            &root,
            SOURCE,
            &pipeline,
            root_digest,
            declared_capture_digest(&pipeline.program_ir.program_digest).unwrap(),
            declared_capture_digest(&pipeline.completion_plan.plan_digest).unwrap(),
        )
        .unwrap();
        let scope = binding.execution_scope();
        // Test-only signer is external to the actual capture producer. The
        // synthetic parent, fixed epoch and local key are NOT real host ingress.
        let issuer = ReceiptIssuer::from_key_bytes("tlcx05-deadline-fixture", &[125; 32]).unwrap();
        let verifier =
            ReceiptVerifier::from_key_bytes("tlcx05-deadline-fixture", &[125; 32]).unwrap();
        let event = AuthorityEvent {
            action: Action::Test,
            source: AuthoritySourceKind::VerifiedHostUserSpan,
            polarity: DeonticPolarity::Grant,
            span: SourceSpan {
                start: 0,
                end: SOURCE.len(),
            },
            scope: scope.clone(),
        };
        let parent = stable_sha256("TLCX05 synthetic parent; real host unobserved");
        let subject = SubjectRevision::checked(stable_sha256(SOURCE)).unwrap();
        let receipt_scope = ReceiptScope::checked(format!("authority/test/{scope}")).unwrap();
        let payload = authority_event_payload_digest(&event);
        let raw = issuer
            .issue(
                ReceiptClass::Authority,
                subject.clone(),
                receipt_scope.clone(),
                payload.clone(),
                100,
                Some(400),
                Some(parent.clone()),
            )
            .unwrap();
        let verified = verifier
            .verify(
                raw,
                &ReceiptPolicy::exact(
                    ReceiptClass::Authority,
                    subject,
                    receipt_scope,
                    payload,
                    100,
                )
                .with_parent(&parent),
                &mut ReplayGuard::default(),
            )
            .unwrap();
        let permit = ExecutionPermit::from_verified_user_event(
            VerifiedAuthorityEvent::checked(event, verified).unwrap(),
            SOURCE,
            AuthorityRevision(1),
            100,
            400,
            0,
        )
        .unwrap();
        Self {
            root,
            pipeline,
            root_digest,
            scope,
            permit,
            host: verifier.key_fingerprint().into(),
        }
    }
    pub fn binding(&self) -> SourceReadBinding<'_> {
        SourceReadBinding::checked(
            &self.root,
            SOURCE,
            &self.pipeline,
            self.root_digest,
            declared_capture_digest(&self.pipeline.program_ir.program_digest).unwrap(),
            declared_capture_digest(&self.pipeline.completion_plan.plan_digest).unwrap(),
        )
        .unwrap()
    }
    pub fn authority(&self) -> FixtureAuthority<'_> {
        FixtureAuthority {
            fixture: self,
            calls: 0,
        }
    }
}
pub struct FixtureAuthority<'a> {
    fixture: &'a CaptureFixture,
    pub calls: usize,
}
impl CurrentSnapshotAuthority for FixtureAuthority<'_> {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        self.calls += 1;
        let f = self.fixture;
        Ok(SnapshotAuthorityView {
            permit: &f.permit,
            expected_principal: f.permit.principal_binding(),
            context: ExecutionPermitContext {
                principal: f.permit.principal_binding(),
                action: Action::Test,
                scope: &f.scope,
                source_revision: &f.pipeline.source_revision,
                authority_revision: AuthorityRevision(1),
                now_epoch: 100,
                revocation_revision: 0,
                revoked_permit_digests: &[],
                caller_origin: CallerOrigin::HostVerifiedUser,
                trusted_host_fingerprint: &f.host,
            },
        })
    }
}
pub struct MissingAuthority;
impl CurrentSnapshotAuthority for MissingAuthority {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        Err(SnapshotCaptureError::AuthorityUnavailable)
    }
}

pub struct Inputs<'a> {
    run: &'a RunResources,
    pub reads: usize,
    pub expire_after_first_read: bool,
}
impl<'a> Inputs<'a> {
    pub fn new(run: &'a RunResources) -> Self {
        Self {
            run,
            reads: 0,
            expire_after_first_read: false,
        }
    }
}
impl ToolProcessRunner for Inputs<'_> {
    fn run(
        &mut self,
        _source: &str,
        _request: BoundedToolRequest<'_>,
        _consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        Err(ResourceError::InvalidConfig) // Inventory must use real bounded leaf I/O, never spawn.
    }
    fn with_input(
        &mut self,
        source: &str,
        cap: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        let mut reservation = self.run.reserve_for_source(
            source_digest(source),
            &[q(ResourceKind::Ram, u64::try_from(cap).unwrap())],
        )?;
        let result = consume(&reservation.lease());
        reservation.release_confirmed()?;
        result?;
        self.reads += 1;
        if self.expire_after_first_read && self.reads == 1 {
            // The actual first leaf callback already ran under its funded view.
            // Narrowing must invalidate the preexisting outer capture view too.
            self.run
                .narrow_deadline(Deadline::after(Duration::from_millis(20))?)?;
            std::thread::sleep(Duration::from_millis(60));
        }
        Ok(())
    }
}
