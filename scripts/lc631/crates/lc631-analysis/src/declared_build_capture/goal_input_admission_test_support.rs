//! Real bounded resources and declared-file captures for H4 unit tests only.
//! Local fixture receipts are not host ingress or semantic-validation evidence.
use crate::declared_build_capture::*;
use crate::{DgclBackendSet, DgclPipelineReport, ToolProcessRunner};
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_substrate::resource::*;
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const SOURCE: &str = "Please edit the source and test the package.\n";
pub fn pipeline() -> DgclPipelineReport {
    crate::build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap()
}
pub fn lease(ram: u64, cancellation: Cancellation, deadline: Option<Instant>) -> ResourceLease {
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let deadline = deadline
        .map(Deadline::at)
        .unwrap_or_else(|| Deadline::after(Duration::from_secs(90)).unwrap());
    let reported = ReportedResourceLease::checked(
        1,
        context::SourceRevision::of(SOURCE).digest(),
        &[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(ram).unwrap(),
        )],
        deadline,
        cancellation,
    )
    .unwrap();
    issuer.issue_admitted(1, reported).unwrap()
}
pub fn funded(
    pipeline: &DgclPipelineReport,
    resources: &ResourceReservationView<'_>,
) -> Result<(), SnapshotCaptureError> {
    resources.check()?;
    let scratch = resources.claim_resources(&[ResourceRequest::new(
        ResourceKind::Ram,
        ResourceAmount::checked(64 * 1024)?,
    )])?;
    super::goal_input_admission::check(pipeline, resources)?;
    scratch.check()?;
    Ok(())
}
pub fn binding<'a>(root: &'a Path, pipeline: &'a DgclPipelineReport) -> SourceReadBinding<'a> {
    // Positive fixture intentionally normalizes its actual existing root.
    // Negative descriptors use a declared Digest32 and do not call this helper.
    let root_digest = crate::dgcl_repository_root_digest(root).unwrap();
    SourceReadBinding::checked(
        root,
        SOURCE,
        pipeline,
        declared_capture_digest(&root_digest).unwrap(),
        declared_capture_digest(&pipeline.program_ir.program_digest).unwrap(),
        declared_capture_digest(&pipeline.completion_plan.plan_digest).unwrap(),
    )
    .unwrap()
}
pub fn actual_root() -> PathBuf {
    let serial = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("h4-{}-{serial:x}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"h4\"\nversion=\"0.0.0\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn value()->u8{1}\n").unwrap();
    eprintln!("retained H4 owned fixture: {}", root.display());
    root
}
struct Authority {
    permit: ExecutionPermit,
    principal: PrincipalBinding,
    host: String,
    source: String,
    scope: String,
    calls: usize,
}
impl CurrentSnapshotAuthority for Authority {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        self.calls += 1;
        Ok(SnapshotAuthorityView {
            permit: &self.permit,
            expected_principal: &self.principal,
            context: ExecutionPermitContext {
                principal: &self.principal,
                action: Action::Test,
                scope: &self.scope,
                source_revision: &self.source,
                authority_revision: AuthorityRevision(1),
                now_epoch: 100,
                revocation_revision: 0,
                revoked_permit_digests: &[],
                caller_origin: CallerOrigin::HostVerifiedUser,
                trusted_host_fingerprint: &self.host,
            },
        })
    }
}
fn authority(binding: &SourceReadBinding<'_>) -> Authority {
    let issuer = ReceiptIssuer::from_key_bytes("h4-local-fixture", &[124; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("h4-local-fixture", &[124; 32]).unwrap();
    let scope = binding.execution_scope();
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
    let parent = stable_sha256("H4 local fixture, not actual host ingress");
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
    let receipt = verifier
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
        VerifiedAuthorityEvent::checked(event, receipt).unwrap(),
        SOURCE,
        AuthorityRevision(1),
        100,
        400,
        0,
    )
    .unwrap();
    Authority {
        principal: permit.principal_binding().clone(),
        permit,
        host: verifier.key_fingerprint().into(),
        source: stable_sha256(SOURCE),
        scope,
        calls: 0,
    }
}
struct Reader<'a, 'b> {
    resources: &'a ResourceReservationView<'b>,
    reads: usize,
}
impl ToolProcessRunner for Reader<'_, '_> {
    fn run(
        &mut self,
        _: &str,
        _: crate::BoundedToolRequest<'_>,
        _: &mut dyn FnMut(&crate::ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        Err(ResourceError::InvalidConfig)
    }
    fn with_input(
        &mut self,
        source: &str,
        _: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.reads += 1;
        if self.resources.source_revision() != context::SourceRevision::of(source).digest() {
            return Err(ResourceError::SourceMismatch);
        }
        consume(self.resources)
    }
}
pub fn actual_capture<'r>(
    binding: &SourceReadBinding<'_>,
    resources: &'r ResourceReservationView<'_>,
) -> (SnapshotCaptureAttempt<'r>, usize, usize) {
    let mut authority = authority(binding);
    let mut reader = Reader {
        resources,
        reads: 0,
    };
    let attempt = capture_declared_build(binding, &mut reader, &mut authority, resources);
    (attempt, authority.calls, reader.reads)
}
pub struct NeverAuthority<'a>(pub &'a Cell<usize>);
impl CurrentSnapshotAuthority for NeverAuthority<'_> {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        self.0.set(self.0.get() + 1);
        Err(SnapshotCaptureError::AuthorityUnavailable)
    }
}
pub struct NeverReader<'a>(pub &'a Cell<usize>);
impl ToolProcessRunner for NeverReader<'_> {
    fn run(
        &mut self,
        _: &str,
        _: crate::BoundedToolRequest<'_>,
        _: &mut dyn FnMut(&crate::ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.0.set(self.0.get() + 1);
        Err(ResourceError::InvalidConfig)
    }
    fn with_input(
        &mut self,
        _: &str,
        _: usize,
        _: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.0.set(self.0.get() + 1);
        Err(ResourceError::InvalidConfig)
    }
}
