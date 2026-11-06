//! 実filesystemのcanonical互換とadmission cuts。中央だけが実行する。
use super::*;
use crate::{build_dgcl_pipeline, capture_dgcl_cargo_snapshot_with_runner, DgclBackendSet};
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_substrate::resource::{
    Cancellation, Deadline, ReportedResourceLease, ResourceLease, ResourceLeaseIssuer,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SOURCE: &str = "Please edit the source and test the package.";
struct Reader<'a, 'b>(&'a ResourceReservationView<'b>);
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
        if self.0.source_revision() != context::SourceRevision::of(source).digest() {
            return Err(ResourceError::SourceMismatch);
        }
        consume(self.0)
    }
}
fn lease(source: &str, ram: u64, cancellation: Cancellation) -> ResourceLease {
    let issuer = ResourceLeaseIssuer::new().unwrap();
    let report = ReportedResourceLease::checked(
        1,
        context::SourceRevision::of(source).digest(),
        &[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(ram).unwrap(),
        )],
        Deadline::after(Duration::from_secs(90)).unwrap(),
        cancellation,
    )
    .unwrap();
    // Unit fixture's admitted owner, not an OS-global quota or a host receipt.
    issuer.issue_admitted(1, report).unwrap()
}
fn root() -> std::path::PathBuf {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("t31i-{}-{id:x}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"inventory\"\nversion=\"0.0.0\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn value()->u8{1}\n").unwrap();
    std::fs::write(root.join("target/ignored.rs"), "excluded").unwrap();
    eprintln!("retained owned inventory fixture: {}", root.display());
    root
}
fn permit() -> ExecutionPermit {
    let issuer = ReceiptIssuer::from_key_bytes("inventory-unit", &[119; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("inventory-unit", &[119; 32]).unwrap();
    let event = AuthorityEvent {
        action: Action::Test,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        scope: "inventory-unit".into(),
    };
    let parent = stable_sha256("synthetic inventory fixture, not actual host");
    let scope = ReceiptScope::checked("authority/test/inventory-unit").unwrap();
    let subject = SubjectRevision::checked(stable_sha256(SOURCE)).unwrap();
    let payload = authority_event_payload_digest(&event);
    let raw = issuer
        .issue(
            ReceiptClass::Authority,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            100,
            Some(400),
            Some(parent.clone()),
        )
        .unwrap();
    let checked = verifier
        .verify(
            raw,
            &ReceiptPolicy::exact(ReceiptClass::Authority, subject, scope, payload, 100)
                .with_parent(&parent),
            &mut ReplayGuard::default(),
        )
        .unwrap();
    ExecutionPermit::from_verified_user_event(
        VerifiedAuthorityEvent::checked(event, checked).unwrap(),
        SOURCE,
        AuthorityRevision(1),
        100,
        400,
        0,
    )
    .unwrap()
}
#[test]
fn tlcx31_inventory_matches_actual_canonical_capture_and_changed_declared_inputs() {
    let root = root();
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let owner = lease(SOURCE, 32 * 1024 * 1024, Cancellation::default());
    let view = owner.view();
    let permit = permit();
    let verifier = ReceiptVerifier::from_key_bytes("inventory-unit", &[119; 32]).unwrap();
    let authority = ExecutionPermitContext {
        principal: permit.principal_binding(),
        action: Action::Test,
        scope: "inventory-unit",
        source_revision: &pipeline.source_revision,
        authority_revision: AuthorityRevision(1),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: verifier.key_fingerprint(),
    };
    let actual = collect(
        &root,
        SOURCE,
        &pipeline,
        &authority,
        &mut Reader(&view),
        &view,
    )
    .unwrap();
    let legacy_actual = capture_dgcl_cargo_snapshot_with_runner(
        &root,
        SOURCE,
        &pipeline,
        &authority,
        &mut Reader(&view),
    )
    .unwrap();
    assert_eq!(actual, legacy_actual);
    std::fs::write(root.join("target/ignored.rs"), "still excluded").unwrap();
    let excluded = collect(
        &root,
        SOURCE,
        &pipeline,
        &authority,
        &mut Reader(&view),
        &view,
    )
    .unwrap();
    assert_eq!(actual.build_input_digest, excluded.build_input_digest);
    for path in ["Cargo.toml", "src/lib.rs"] {
        let prior = std::fs::read(root.join(path)).unwrap();
        std::fs::write(root.join(path), [prior.as_slice(), b"\n"].concat()).unwrap();
        let changed = collect(
            &root,
            SOURCE,
            &pipeline,
            &authority,
            &mut Reader(&view),
            &view,
        )
        .unwrap();
        assert_ne!(actual.build_input_digest, changed.build_input_digest);
        std::fs::write(root.join(path), prior).unwrap();
    }
}
#[test]
fn tlcx31_inventory_rejects_missing_funding_before_filesystem_and_foreign_source() {
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let owner = lease(SOURCE, 4096, Cancellation::default());
    let view = owner.view();
    let permit = permit();
    let verifier = ReceiptVerifier::from_key_bytes("inventory-unit", &[119; 32]).unwrap();
    let authority = ExecutionPermitContext {
        principal: permit.principal_binding(),
        action: Action::Test,
        scope: "inventory-unit",
        source_revision: &pipeline.source_revision,
        authority_revision: AuthorityRevision(1),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: verifier.key_fingerprint(),
    };
    assert!(matches!(
        collect(
            Path::new("not-an-admitted-root"),
            SOURCE,
            &pipeline,
            &authority,
            &mut Reader(&view),
            &view
        ),
        Err(InventoryError::Resource(ResourceError::Exhausted(
            ResourceKind::Ram
        )))
    ));
    let other = lease(
        "different source",
        32 * 1024 * 1024,
        Cancellation::default(),
    );
    let foreign = other.view();
    assert!(matches!(
        collect(
            Path::new("not-an-admitted-root"),
            SOURCE,
            &pipeline,
            &authority,
            &mut Reader(&foreign),
            &foreign
        ),
        Err(InventoryError::Resource(ResourceError::SourceMismatch))
    ));
}

#[test]
fn tlcx31_inventory_rejects_actual_aggregate_path_budget_and_cancelled_view() {
    let root = root();
    // Real declared-input metadata, not an injected count/report. Short parent
    // names keep these ASCII paths within ordinary Windows path lengths.
    // The retained fixture is a targeted allocation-admission test, not cache
    // deletion quota or independently validated program semantics.
    for index in 0..1500 {
        let name = format!("{index:04}_{}.rs", "x".repeat(168));
        std::fs::write(root.join(name), b"").unwrap();
    }
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let cancellation = Cancellation::default();
    let owner = lease(SOURCE, 32 * 1024 * 1024, cancellation.clone());
    let view = owner.view();
    let permit = permit();
    let verifier = ReceiptVerifier::from_key_bytes("inventory-unit", &[119; 32]).unwrap();
    let authority = ExecutionPermitContext {
        principal: permit.principal_binding(),
        action: Action::Test,
        scope: "inventory-unit",
        source_revision: &pipeline.source_revision,
        authority_revision: AuthorityRevision(1),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: verifier.key_fingerprint(),
    };
    assert!(matches!(
        collect(
            &root,
            SOURCE,
            &pipeline,
            &authority,
            &mut Reader(&view),
            &view
        ),
        Err(InventoryError::Limit)
    ));
    cancellation.cancel();
    assert!(matches!(
        collect(
            &root,
            SOURCE,
            &pipeline,
            &authority,
            &mut Reader(&view),
            &view
        ),
        Err(InventoryError::Resource(ResourceError::Cancelled))
    ));
}
