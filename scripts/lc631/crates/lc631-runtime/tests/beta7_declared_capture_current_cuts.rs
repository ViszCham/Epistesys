//! 日本語: 公開capture producerの実inventoryとcurrent拒否を、compilerなしで中央が検査する。
//! English: Central checks real inventory/current refusals through the public capture producer.
//! Local configured grants are fixtures. These tests construct no source-build origin.
#![forbid(unsafe_code)]

use lc631_analysis::{
    build_dgcl_pipeline,
    declared_build_capture::{
        capture_declared_build, declared_capture_digest, CurrentSnapshotAuthority,
        SnapshotAuthorityView, SnapshotCaptureError, SourceReadBinding,
    },
    BoundedToolRequest, DgclBackendSet, DgclPipelineReport, ResourceToolResult,
    ToolProcessRunner,
};
use lc631_core::{
    authority_event_payload_digest, stable_sha256, Action, AuthorityEvent,
    AuthorityRevision, AuthoritySourceKind, CallerOrigin, DeonticPolarity,
    ExecutionPermit, ExecutionPermitContext, PermitRejection, PermitValidation,
    SourceSpan, VerifiedAuthorityEvent,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier,
    ReplayGuard, SubjectRevision,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceError,
    ResourceKind, ResourceLimits, ResourceRequest, ResourceReservationView,
};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const TASK: &str = "Please test the package.";
static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

fn request(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}

fn new_file(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

struct Fixture {
    root: PathBuf,
    pipeline: DgclPipelineReport,
}

impl Fixture {
    fn new() -> Self {
        let epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let sequence = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "beta7-c1-capture-{}-{epoch:x}-{sequence:x}", std::process::id(),
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        new_file(
            &root.join("Cargo.toml"),
            b"[package]\nname='capture_current_fixture'\nversion='0.0.0'\nedition='2021'\n",
        );
        new_file(&root.join("src/lib.rs"), b"pub fn value() -> u8 { 1 }\n");
        let pipeline = build_dgcl_pipeline(
            TASK, DgclBackendSet { english: None, japanese: None },
        ).unwrap();
        eprintln!("retained new C1 capture fixture: {}", root.display());
        Self { root, pipeline }
    }

    fn binding(&self) -> SourceReadBinding<'_> {
        SourceReadBinding::checked(
            &self.root,
            TASK,
            &self.pipeline,
            declared_capture_digest(
                &lc631_analysis::dgcl_repository_root_digest(&self.root).unwrap(),
            ).unwrap(),
            declared_capture_digest(&self.pipeline.program_ir.program_digest).unwrap(),
            declared_capture_digest(&self.pipeline.completion_plan.plan_digest).unwrap(),
        ).unwrap()
    }

    fn append_declared_source(&self) {
        let path = self.root.join("src/lib.rs");
        assert!(path.starts_with(&self.root));
        let mut file = OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(b"// new owned declared input bytes\n").unwrap();
        file.sync_all().unwrap();
    }
}

fn resources(
    consume: impl for<'view, 'lease> FnOnce(&'view ResourceReservationView<'lease>),
) {
    let mut amounts = [0; lc631_substrate::resource::RESOURCE_KINDS];
    amounts[ResourceKind::Ram.index()] = 96 * 1024 * 1024;
    let limits = ResourceLimits::checked(amounts).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker.begin_run(
        0xc101,
        lc631_core::context::SourceRevision::of(TASK).digest(),
        limits,
        Deadline::after(Duration::from_secs(60)).unwrap(),
        Cancellation::default(),
    ).unwrap();
    let mut reservation = run.reserve(&[request(ResourceKind::Ram, 32 * 1024 * 1024)]).unwrap();
    {
        let view = reservation.lease();
        consume(&view);
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(broker.used(ResourceKind::Ram).unwrap(), 0);
}

/// Public reader supplies the actual live reservation. No process/capture DTO is injected.
struct Reader<'view, 'lease> {
    view: &'view ResourceReservationView<'lease>,
    inputs: usize,
    native_requests: usize,
}

impl ToolProcessRunner for Reader<'_, '_> {
    fn run(
        &mut self,
        _: &str,
        _: BoundedToolRequest<'_>,
        _: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.native_requests += 1;
        Err(ResourceError::InvalidConfig)
    }

    fn with_input(
        &mut self,
        source: &str,
        cap: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.inputs += 1;
        if source != TASK || cap != 8192 {
            return Err(ResourceError::InvalidConfig);
        }
        self.view.check()?;
        consume(self.view)
    }
}

#[derive(Clone, Copy)]
enum Cut {
    None,
    UnavailableBefore,
    UnavailableAfter,
    RevokedAfter,
    EpochRollback,
    RevocationRollback,
}

struct Authority {
    permit: ExecutionPermit,
    rollback_permit: Option<ExecutionPermit>,
    source: String,
    scope: String,
    host_fingerprint: String,
    revoked: Vec<String>,
    calls: usize,
    cut: Cut,
}

impl Authority {
    fn new(binding: &SourceReadBinding<'_>, cut: Cut) -> Self {
        let issuer = ReceiptIssuer::from_key_bytes("c1-declared-capture-fixture", &[0xa3; 32]).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("c1-declared-capture-fixture", &[0xa3; 32]).unwrap();
        let scope = binding.execution_scope();
        let make_permit = |revocation_revision| {
            let event = AuthorityEvent {
                action: Action::Test,
                source: AuthoritySourceKind::VerifiedHostUserSpan,
                polarity: DeonticPolarity::Grant,
                span: SourceSpan { start: 0, end: TASK.len() },
                scope: scope.clone(),
            };
            let parent = stable_sha256("configured capture fixture; no observed host authority");
            let subject = SubjectRevision::checked(stable_sha256(TASK)).unwrap();
            let receipt_scope = ReceiptScope::checked(format!("authority/test/{scope}")).unwrap();
            let payload = authority_event_payload_digest(&event);
            let receipt = issuer.issue(
                ReceiptClass::Authority, subject.clone(), receipt_scope.clone(),
                payload.clone(), 100, Some(400), Some(parent.clone()),
            ).unwrap();
            let verified = verifier.verify(
                receipt,
                &ReceiptPolicy::exact(
                    ReceiptClass::Authority, subject, receipt_scope, payload, 100,
                ).with_parent(&parent),
                &mut ReplayGuard::default(),
            ).unwrap();
            ExecutionPermit::from_verified_user_event(
                VerifiedAuthorityEvent::checked(event, verified).unwrap(),
                TASK, AuthorityRevision(1), 100, 400, revocation_revision,
            ).unwrap()
        };
        let permit = make_permit(if matches!(cut, Cut::RevocationRollback) { 9 } else { 0 });
        let rollback_permit = if matches!(cut, Cut::RevocationRollback) {
            Some(make_permit(8))
        } else {
            None
        };
        Self {
            permit,
            rollback_permit,
            source: stable_sha256(TASK),
            scope,
            host_fingerprint: verifier.key_fingerprint().to_owned(),
            revoked: Vec::new(),
            calls: 0,
            cut,
        }
    }
}

impl CurrentSnapshotAuthority for Authority {
    fn current(&mut self) -> Result<SnapshotAuthorityView<'_>, SnapshotCaptureError> {
        let after_first = self.calls != 0;
        self.calls += 1;
        if matches!(self.cut, Cut::UnavailableBefore)
            || (after_first && matches!(self.cut, Cut::UnavailableAfter))
        {
            return Err(SnapshotCaptureError::AuthorityUnavailable);
        }
        self.revoked.clear();
        if after_first && matches!(self.cut, Cut::RevokedAfter) {
            self.revoked.push(self.permit.digest().to_owned());
        }
        let epoch = match self.cut {
            Cut::EpochRollback if after_first => 150,
            Cut::EpochRollback => 200,
            _ => 100,
        };
        let revocation = match self.cut {
            Cut::RevocationRollback if after_first => 8,
            Cut::RevocationRollback => 9,
            _ => 0,
        };
        let permit = if after_first && matches!(self.cut, Cut::RevocationRollback) {
            self.rollback_permit.as_ref().expect("configured lower-revision permit")
        } else {
            &self.permit
        };
        let context = ExecutionPermitContext {
            principal: permit.principal_binding(),
            action: Action::Test,
            scope: &self.scope,
            source_revision: &self.source,
            authority_revision: AuthorityRevision(1),
            now_epoch: epoch,
            revocation_revision: revocation,
            revoked_permit_digests: &self.revoked,
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: &self.host_fingerprint,
        };
        let expected_validation = if after_first && matches!(self.cut, Cut::RevokedAfter) {
            PermitValidation::Rejected(PermitRejection::Revoked)
        } else {
            PermitValidation::ValidGrant
        };
        assert_eq!(permit.validate(&context), expected_validation);
        Ok(SnapshotAuthorityView {
            permit,
            context,
            expected_principal: permit.principal_binding(),
        })
    }
}

#[test]
fn public_capture_control_and_owned_source_append_change_actual_content_pins() {
    let fixture = Fixture::new();
    let binding = fixture.binding();
    resources(|view| {
        let mut reader = Reader { view, inputs: 0, native_requests: 0 };
        let mut authority = Authority::new(&binding, Cut::None);
        let first = capture_declared_build(&binding, &mut reader, &mut authority, view);
        let before = first.current().expect("normal public capture");
        before.check_current(view).unwrap();
        before.matches_expected(before.content_pins()).unwrap();
        assert_eq!(before.root(), binding.root());
        assert_eq!(before.program(), binding.program());
        assert_eq!(before.plan(), binding.plan());
        assert_eq!(before.content_pins().source, binding.source());
        assert_eq!(before.authority_before().checked_epoch, 100);
        assert_eq!(before.authority_after().unwrap().checked_epoch, 100);
        assert_eq!((authority.calls, reader.inputs, reader.native_requests), (2, 2, 0));

        fixture.append_declared_source();
        let second = capture_declared_build(&binding, &mut reader, &mut authority, view);
        let after = second.current().expect("fresh public recapture after owned append");
        assert_ne!(after.content_pins().build, before.content_pins().build);
        assert_ne!(after.snapshot_digest(), before.snapshot_digest());
        assert_eq!(after.content_pins().source, before.content_pins().source);
        assert_eq!(
            after.matches_expected(before.content_pins()),
            Err(SnapshotCaptureError::ContentMismatch),
        );
        assert_eq!((authority.calls, reader.inputs, reader.native_requests), (4, 4, 0));
    });
}

#[test]
fn late_authority_loss_retains_actual_capture_and_cannot_be_cleared_by_provider_recovery() {
    for (cut, expected) in [
        (Cut::UnavailableAfter, SnapshotCaptureError::AuthorityUnavailable),
        (Cut::RevokedAfter, SnapshotCaptureError::Unauthorized),
    ] {
        let fixture = Fixture::new();
        let binding = fixture.binding();
        resources(|view| {
            let mut reader = Reader { view, inputs: 0, native_requests: 0 };
            let mut authority = Authority::new(&binding, cut);
            let failed = capture_declared_build(&binding, &mut reader, &mut authority, view);
            assert_eq!(failed.diagnostic(), Some(&expected));
            assert_eq!(failed.current().err(), Some(expected.clone()));
            let actual = failed.capture().expect("actual inventory survives the post-cut");
            let pins = actual.content_pins();
            assert_eq!(actual.authority_before().checked_epoch, 100);
            assert_eq!(actual.authority_after(), None);
            assert_eq!(actual.check_current(view), Err(expected.clone()));
            assert_eq!(actual.matches_expected(pins), Err(expected.clone()));
            assert_eq!((authority.calls, reader.inputs, reader.native_requests), (2, 2, 0));

            authority.cut = Cut::None;
            let recovered = capture_declared_build(&binding, &mut reader, &mut authority, view);
            let current = recovered.current().expect("new capture after provider recovery");
            assert_eq!(current.content_pins(), pins);
            assert!(current.authority_after().is_some());
            assert_eq!(failed.current().err(), Some(expected.clone()));
            assert_eq!(actual.matches_expected(pins), Err(expected.clone()));
            assert_eq!(actual.check_current(view), Err(expected));
            assert_eq!((authority.calls, reader.inputs, reader.native_requests), (4, 4, 0));
        });
    }
}

#[test]
fn regressing_post_authority_keeps_actual_before_stamp_and_exact_failure() {
    for (cut, expected, before_epoch, before_revocation) in [
        (Cut::EpochRollback, SnapshotCaptureError::ClockRollback, 200, 0),
        (Cut::RevocationRollback, SnapshotCaptureError::RevocationRollback, 100, 9),
    ] {
        let fixture = Fixture::new();
        let binding = fixture.binding();
        resources(|view| {
            let mut reader = Reader { view, inputs: 0, native_requests: 0 };
            let mut authority = Authority::new(&binding, cut);
            let failed = capture_declared_build(&binding, &mut reader, &mut authority, view);
            assert_eq!(failed.diagnostic(), Some(&expected));
            assert_eq!(failed.current().err(), Some(expected.clone()));
            let actual = failed.capture().expect("post-check regression retains actual inventory");
            assert_eq!(actual.authority_before().checked_epoch, before_epoch);
            assert_eq!(actual.authority_before().revocation_revision, before_revocation);
            assert_eq!(actual.authority_after(), None);
            assert_eq!(actual.matches_expected(actual.content_pins()), Err(expected));
            assert_eq!((authority.calls, reader.inputs, reader.native_requests), (2, 2, 0));
        });
    }
}

#[test]
fn missing_pre_authority_publishes_no_capture_and_performs_no_inventory_reads() {
    let fixture = Fixture::new();
    let binding = fixture.binding();
    resources(|view| {
        let mut reader = Reader { view, inputs: 0, native_requests: 0 };
        let mut authority = Authority::new(&binding, Cut::UnavailableBefore);
        let refused = capture_declared_build(&binding, &mut reader, &mut authority, view);
        assert_eq!(refused.diagnostic(), Some(&SnapshotCaptureError::AuthorityUnavailable));
        assert_eq!(refused.current().err(), Some(SnapshotCaptureError::AuthorityUnavailable));
        assert!(refused.capture().is_none());
        assert_eq!((authority.calls, reader.inputs, reader.native_requests), (1, 0, 0));

        authority.cut = Cut::None;
        let normal = capture_declared_build(&binding, &mut reader, &mut authority, view);
        normal.current().expect("same fixture admits with a fresh valid provider");
        assert_eq!((authority.calls, reader.inputs, reader.native_requests), (3, 2, 0));
    });
}
