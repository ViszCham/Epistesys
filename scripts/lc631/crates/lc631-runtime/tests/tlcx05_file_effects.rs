//! Real same-parent resource admission, with fixture-only signed Edit authority.
//! No compiler, host callback, runtime safety or general repair proof is implied.
use lc631_analysis::*;
use lc631_core::*;
use lc631_receipt_kernel::*;
use lc631_runtime::resource::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn quota(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}

struct Inputs<'a> {
    run: &'a RunResources,
    calls: usize,
}
impl ToolProcessRunner for Inputs<'_> {
    fn run(
        &mut self,
        _source: &str,
        _request: BoundedToolRequest<'_>,
        _consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        panic!("file-effect admission tests do not start a child")
    }
    fn with_input(
        &mut self,
        source: &str,
        cap: usize,
        consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.calls += 1;
        let mut reservation = self.run.reserve_for_source(
            context::SourceRevision::of(source).digest(),
            &[quota(ResourceKind::Ram, cap as u64)],
        )?;
        let result = consume(&reservation.lease());
        reservation.release_confirmed()?;
        result
    }
}

#[derive(Clone, Copy)]
enum Cut {
    Cancelled,
    Overcap,
    ForeignSource,
}
struct Effects<'a> {
    run: &'a RunResources,
    cancel: Cancellation,
    cut: Cut,
    calls: usize,
}
impl DgclFileEffectOwner for Effects<'_> {
    fn apply(
        &mut self,
        plan: DgclFileEffectPlan<'_>,
        operation: &mut dyn FnMut(
            &ResourceReservationView<'_>,
        ) -> Result<(), DgclRepairDriverError>,
    ) -> Result<(), DgclRepairDriverError> {
        self.calls += 1;
        let source = if matches!(self.cut, Cut::ForeignSource) {
            "foreign effect scope"
        } else {
            plan.source
        };
        if matches!(self.cut, Cut::Cancelled) {
            self.cancel.cancel();
        }
        let amount = if matches!(self.cut, Cut::Overcap) {
            1
        } else {
            2 * 1024 * 1024
        };
        let mut reservation = self
            .run
            .reserve_for_source(
                context::SourceRevision::of(source).digest(),
                &[quota(ResourceKind::Disk, amount)],
            )
            .map_err(|_| DgclRepairDriverError::Failed)?;
        let result = operation(&reservation.lease());
        reservation
            .release_confirmed()
            .map_err(|_| DgclRepairDriverError::Failed)?;
        result
    }
}
struct NoValidation;
impl DgclFileEvidenceValidator for NoValidation {
    fn observe(
        &mut self,
        _snapshot: &DgclExecutionSnapshot,
        _budget: Duration,
    ) -> Result<Vec<CodingEvidenceClaim>, DgclRepairDriverError> {
        panic!("rejected file effects cannot produce validation evidence")
    }
}

fn permit(
    source: &str,
    scope: &str,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
) -> ExecutionPermit {
    let parent = stable_sha256("TLCX05 file effect fixture; not a host observation");
    let event = AuthorityEvent {
        action: Action::Edit,
        source: AuthoritySourceKind::VerifiedHostUserSpan,
        polarity: DeonticPolarity::Grant,
        span: SourceSpan {
            start: 0,
            end: source.len(),
        },
        scope: scope.into(),
    };
    let subject = SubjectRevision::checked(stable_sha256(source)).unwrap();
    let receipt_scope = ReceiptScope::checked(format!("authority/edit/{scope}")).unwrap();
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
    ExecutionPermit::from_verified_user_event(
        VerifiedAuthorityEvent::checked(event, verified).unwrap(),
        source,
        AuthorityRevision(1),
        100,
        400,
        0,
    )
    .unwrap()
}

fn exercise(cut: Cut) {
    let source = "Please edit the source and test the package.";
    let pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Acquire one fresh owner; equal clock values never share mutable fixtures.
    let root = (0..64)
        .find_map(|attempt| {
            let path = std::env::temp_dir().join(format!(
                "tlcx05-file-effect-{}-{unique}-{attempt}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => Some(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(error) => panic!("fixture directory: {error}"),
            }
        })
        .expect("fixture collision budget exhausted");
    std::fs::create_dir(root.join("src")).unwrap();
    let before = "pub fn value() -> u8 { 0 }\n";
    let after = "pub fn value() -> u8 { 1 }\n";
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='tlcx05_file_effect'\nversion='0.0.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), before).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("tlcx05-file-effect-fixture", &[72; 32]).unwrap();
    let verifier =
        ReceiptVerifier::from_key_bytes("tlcx05-file-effect-fixture", &[72; 32]).unwrap();
    let root_digest = dgcl_repository_root_digest(&root).unwrap();
    let scope = format!("dgcl/repair/{root_digest}/src/lib.rs");
    let permission = permit(source, &scope, &issuer, &verifier);
    let authority = ExecutionPermitContext {
        principal: permission.principal_binding(),
        action: Action::Edit,
        scope: &scope,
        source_revision: &pipeline.source_revision,
        authority_revision: AuthorityRevision(1),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: verifier.key_fingerprint(),
    };
    assert_eq!(
        permission.validate(&authority),
        PermitValidation::ValidGrant
    );
    let snapshot = capture_dgcl_cargo_snapshot(&root, source, &pipeline, &authority).unwrap();
    let authorization = DgclRepairAuthorization {
        permit: &permission,
        authority_revision: AuthorityRevision(1),
        trusted_host_fingerprint: verifier.key_fingerprint(),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
    };
    let request = DgclRepairRequest {
        repository_root_digest: Some(root_digest),
        attempt: 1,
        gap_id: pipeline.implementation_closure.gaps[0].gap_id.clone(),
        source_revision: pipeline.source_revision.clone(),
        plan_digest: pipeline.completion_plan.plan_digest.clone(),
        target_ref: "src/lib.rs".into(),
        target_digest_before: stable_sha256(before),
        candidate_digest: stable_sha256(after),
        authorization_scope: scope,
        authorization_span: permission.source_span(),
    };
    let limits = ResourceLimits::checked([16 * 1024 * 1024, 2 * 1024 * 1024, 0, 0, 0, 0]).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let cancellation = Cancellation::default();
    let mut run = broker
        .begin_run(
            1,
            context::SourceRevision::of(source).digest(),
            limits,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            cancellation.clone(),
        )
        .unwrap();
    let mut inputs = Inputs {
        run: &run,
        calls: 0,
    };
    let mut effects = Effects {
        run: &run,
        cancel: cancellation,
        cut,
        calls: 0,
    };
    let mut validator = NoValidation;
    let mut driver = DgclFileRepairDriver::new_with_runner(
        DgclFileRepairConfig {
            root: &root,
            source,
            pipeline: &pipeline,
            initial_snapshot: &snapshot,
            authorization: &authorization,
            issuer: &issuer,
            verifier: &verifier,
            candidates: std::collections::BTreeMap::from([(stable_sha256(after), after.into())]),
            validator: &mut validator,
            epoch_clock: None,
        },
        &mut inputs,
        Some(&mut effects),
    )
    .unwrap();
    assert!(driver
        .apply(&request, &permission, Duration::from_secs(10), 1024 * 1024)
        .is_err());
    drop(driver);
    assert!(
        inputs.calls > 0,
        "must consume real source-bound snapshot inputs before effect cut"
    );
    assert_eq!(
        effects.calls, 1,
        "must reach the actual file-effect admission cut"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        before
    );
    assert!(!root.join(".dgcl-repair-backups").exists());
    assert_eq!(broker.used(ResourceKind::Disk).unwrap(), 0);
    assert_eq!(broker.used(ResourceKind::Child).unwrap(), 0);
    run.close_confirmed().unwrap();
}

#[test]
fn tlcx05_cancelled_file_effect_admission_has_zero_target_or_backup_effects() {
    exercise(Cut::Cancelled)
}
#[test]
fn tlcx05_underfunded_file_effect_has_zero_target_or_backup_effects() {
    exercise(Cut::Overcap)
}
#[test]
fn tlcx05_foreign_source_file_effect_has_zero_target_or_backup_effects() {
    exercise(Cut::ForeignSource)
}
