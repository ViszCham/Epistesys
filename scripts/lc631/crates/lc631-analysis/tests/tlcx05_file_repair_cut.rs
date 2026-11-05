//! Post-implementation adversarial behavior: a valid edit permit does not let
//! an injected resource rejection fall back to legacy snapshots or write files.
use lc631_analysis::*;
use lc631_core::*;
use lc631_process_supervisor::{ResourceError, ResourceReservationView};
use lc631_receipt_kernel::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct RejectReads {
    source: String,
    reads: usize,
    children: usize,
}
impl ToolProcessRunner for RejectReads {
    fn run(
        &mut self,
        _source: &str,
        _request: BoundedToolRequest<'_>,
        _consume: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        self.children += 1;
        Err(ResourceError::Cancelled)
    }
    fn with_input(
        &mut self,
        source: &str,
        _cap: usize,
        _consume: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
    ) -> Result<(), ResourceError> {
        assert_eq!(source, self.source);
        self.reads += 1;
        Err(ResourceError::Cancelled)
    }
}
struct NoValidation;
struct NoEffects;
impl DgclFileEffectOwner for NoEffects {
    fn apply(
        &mut self,
        _plan: DgclFileEffectPlan<'_>,
        _operation: &mut dyn FnMut(
            &ResourceReservationView<'_>,
        ) -> Result<(), DgclRepairDriverError>,
    ) -> Result<(), DgclRepairDriverError> {
        panic!("snapshot rejection must precede any file-effect scope")
    }
}
impl DgclFileEvidenceValidator for NoValidation {
    fn observe(
        &mut self,
        _snapshot: &DgclExecutionSnapshot,
        _budget: Duration,
    ) -> Result<Vec<CodingEvidenceClaim>, DgclRepairDriverError> {
        panic!("resource-rejected repair cannot reach evidence production")
    }
}

fn edit_permit(
    source: &str,
    scope: &str,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
) -> ExecutionPermit {
    let parent = stable_sha256("TLCX05 fixture host ingress; not a Codex host observation");
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
    let checked = verifier
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
        VerifiedAuthorityEvent::checked(event, checked).unwrap(),
        source,
        AuthorityRevision(1),
        100,
        400,
        0,
    )
    .unwrap()
}

#[test]
fn tlcx05_file_repair_valid_authority_cannot_bypass_cancelled_resource_snapshot() {
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
    let root =
        std::env::temp_dir().join(format!("tlcx05-repair-cut-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let broken = "pub fn value() -> u8 { 0 }\n";
    let correct = "pub fn value() -> u8 { 1 }\n";
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='tlcx05_repair_cut'\nversion='0.0.0'\nedition='2021'\n",
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), broken).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("tlcx05-repair-test", &[71; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx05-repair-test", &[71; 32]).unwrap();
    let root_digest = dgcl_repository_root_digest(&root).unwrap();
    let scope = format!("dgcl/repair/{root_digest}/src/lib.rs");
    let permit = edit_permit(source, &scope, &issuer, &verifier);
    let authority = ExecutionPermitContext {
        principal: permit.principal_binding(),
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
    assert_eq!(permit.validate(&authority), PermitValidation::ValidGrant);
    let snapshot = capture_dgcl_cargo_snapshot(&root, source, &pipeline, &authority).unwrap();
    let authorization = DgclRepairAuthorization {
        permit: &permit,
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
        target_digest_before: stable_sha256(broken),
        candidate_digest: stable_sha256(correct),
        authorization_scope: scope,
        authorization_span: permit.source_span(),
    };
    let mut validator = NoValidation;
    let mut runner = RejectReads {
        source: source.into(),
        reads: 0,
        children: 0,
    };
    assert_eq!(
        DgclFileRepairDriver::new_with_runner(
            DgclFileRepairConfig {
                root: &root,
                source,
                pipeline: &pipeline,
                initial_snapshot: &snapshot,
                authorization: &authorization,
                issuer: &issuer,
                verifier: &verifier,
                candidates: std::collections::BTreeMap::new(),
                validator: &mut validator,
                epoch_clock: None,
            },
            &mut runner,
            None
        )
        .err(),
        Some(DgclRepairDriverError::Unavailable),
        "missing file-effect owner must be visible before work"
    );
    let mut effects = NoEffects;
    let mut driver = DgclFileRepairDriver::new_with_runner(
        DgclFileRepairConfig {
            root: &root,
            source,
            pipeline: &pipeline,
            initial_snapshot: &snapshot,
            authorization: &authorization,
            issuer: &issuer,
            verifier: &verifier,
            candidates: std::collections::BTreeMap::from([(
                stable_sha256(correct),
                correct.into(),
            )]),
            validator: &mut validator,
            epoch_clock: None,
        },
        &mut runner,
        Some(&mut effects),
    )
    .unwrap();
    assert_eq!(
        driver
            .apply(&request, &permit, Duration::from_secs(10), 1024 * 1024)
            .err(),
        Some(DgclRepairDriverError::Failed)
    );
    drop(driver);
    assert_eq!(
        runner.reads, 1,
        "valid request must reach the actual injected snapshot cut"
    );
    assert_eq!(runner.children, 0);
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        broken
    );
    assert!(!root.join(".dgcl-repair-backups").exists());
}
