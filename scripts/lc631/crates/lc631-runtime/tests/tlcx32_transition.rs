//! 独立wire負例。署名fixtureを実repair/process観測へ昇格しない。
//! Independent wire negatives; fixtures never become actual repair/process observations.
use lc631_analysis::*;
use lc631_core::{context::SourceRevision, stable_sha256, SourceSpan};
use lc631_receipt_kernel::*;
use lc631_runtime::contextual_resume::transition::{
    digest_text, inspect_transition_chain, TransitionError, TransitionEvidence,
    UntrustedTransitionCheckpoint, BOUND_TRANSITION_BYTES, TRANSITION_BYTES,
};
use lc631_runtime::{journal::*, resource::*};
use lc631_substrate::Digest32;
use std::time::Duration;
#[path = "../../lc631-analysis/tests/tlcx31_file_repair_fixture/authority.rs"]
#[allow(
    dead_code,
    reason = "This target reuses permit only; Cargo registration/connection helpers are exercised by analysis repair tests."
)]
mod authority_fixture;

fn independent_wire() -> Vec<u8> {
    let mut bytes = vec![0; TRANSITION_BYTES];
    bytes[..8].copy_from_slice(b"ETRANS01");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    for (index, pin) in bytes[16..16 + 14 * 32].chunks_exact_mut(32).enumerate() {
        pin.fill(index as u8 + 1);
    }
    bytes[16 + 14 * 32..].copy_from_slice(&100_u64.to_le_bytes());
    bytes
}

#[test]
fn tlcx32_transition_rejects_truncation_trailing_and_reserved_bytes() {
    let valid = independent_wire();
    for cut in 0..valid.len() {
        assert!(matches!(
            UntrustedTransitionCheckpoint::decode(&valid[..cut]),
            Err(TransitionError::InvalidPayload)
        ));
    }
    let mut trailing = valid.clone();
    trailing.push(0);
    assert!(UntrustedTransitionCheckpoint::decode(&trailing).is_err());
    for offset in 10..16 {
        let mut bad = valid.clone();
        bad[offset] = 1;
        assert!(UntrustedTransitionCheckpoint::decode(&bad).is_err());
    }
}

#[test]
fn tlcx32_transition_rejects_unknown_version_magic_and_zero_identity() {
    let valid = independent_wire();
    for offset in [0, 8, 9] {
        let mut bad = valid.clone();
        bad[offset] ^= 0x80;
        assert!(UntrustedTransitionCheckpoint::decode(&bad).is_err());
    }
    for index in 0..14 {
        let mut bad = valid.clone();
        bad[16 + index * 32..16 + (index + 1) * 32].fill(0);
        assert!(UntrustedTransitionCheckpoint::decode(&bad).is_err());
    }
}

#[test]
fn tlcx32_transition_decode_remains_untrusted_and_target_can_change() {
    let bytes = independent_wire();
    let decoded = UntrustedTransitionCheckpoint::decode(&bytes).unwrap();
    assert_eq!(decoded.as_bytes(), bytes.as_slice());
    assert!(decoded.claim_boundary().contains("untrusted"));
    // Distinct before/after physical targets do not require a changed task seed.
    assert_ne!(
        &bytes[16 + 9 * 32..16 + 10 * 32],
        &bytes[16 + 10 * 32..16 + 11 * 32]
    );
}

#[test]
fn tlcx32_v2_adds_a_separate_bounded_before_build_binding() {
    let original = UntrustedTransitionCheckpoint::decode(&independent_wire()).unwrap();
    let mut v2 = original.as_bytes().to_vec();
    v2.resize(BOUND_TRANSITION_BYTES, 91);
    v2[8..10].copy_from_slice(&2_u16.to_le_bytes());
    let decoded = UntrustedTransitionCheckpoint::decode(&v2).unwrap();
    assert_ne!(decoded.payload_digest(), original.payload_digest());
    assert_ne!(decoded.receipt_scope(), original.receipt_scope());
    assert!(UntrustedTransitionCheckpoint::decode(&v2[..TRANSITION_BYTES]).is_err());
    v2[TRANSITION_BYTES..].fill(0);
    assert!(UntrustedTransitionCheckpoint::decode(&v2).is_err());
}

const SOURCE: &str = "Please edit the source and test the package.";
const BEFORE: &str = "pub fn add(a:u32,b:u32)->Option<u32>{None}";
const AFTER: &str = "pub fn add(a:u32,b:u32)->Option<u32>{a.checked_add(b)}";

struct SigningFixture {
    request: DgclRepairRequest,
    application: DgclRepairApplication,
    checkpoint: UntrustedTransitionCheckpoint,
    attestation: UntrustedReceipt,
    verifier: ReceiptVerifier,
}
fn parsed(value: &str) -> Digest32 {
    let value = value.strip_prefix("sha256:").unwrap();
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    Digest32::from_bytes(bytes)
}
fn signing_fixture() -> SigningFixture {
    // Synthetic historical signatures exercise the decoder, NOT an actual
    // repair/process positive or a provider for real host authority.
    let issuer = ReceiptIssuer::from_key_bytes("tlcx32-transition-fixture", &[93; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx32-transition-fixture", &[93; 32]).unwrap();
    let request = DgclRepairRequest {
        attempt: 1,
        gap_id: stable_sha256("gap"),
        source_revision: stable_sha256(SOURCE),
        plan_digest: stable_sha256("dgcl-plan"),
        target_ref: "src/lib.rs".into(),
        target_digest_before: stable_sha256(BEFORE),
        candidate_digest: stable_sha256(AFTER),
        authorization_scope: "fixture/repository/src/lib.rs".into(),
        authorization_span: SourceSpan {
            start: 0,
            end: SOURCE.len(),
        },
        repository_root_digest: Some(stable_sha256("fixture-root")),
    };
    let snapshot = build_dgcl_execution_snapshot(
        SOURCE,
        AFTER.as_bytes(),
        "profile",
        "validator",
        "authority",
        vec![DgclTargetIdentity {
            target_ref: request.target_ref.clone(),
            target_digest: request.candidate_digest.clone(),
        }],
    )
    .unwrap();
    let lease = issue_dgcl_evidence_lease(&snapshot, 100, 200).unwrap();
    let mut application = DgclRepairApplication {
        schema_version: "epistesys-dgcl-repair-application.v1",
        attempt: 1,
        gap_id: request.gap_id.clone(),
        request_digest: dgcl_repair_request_digest(&request),
        source_revision: request.source_revision.clone(),
        plan_digest: request.plan_digest.clone(),
        target_ref: request.target_ref.clone(),
        target_digest_before: request.target_digest_before.clone(),
        target_digest_after: request.candidate_digest.clone(),
        candidate_digest: request.candidate_digest.clone(),
        delta_bytes: 100,
        run_digest: stable_sha256("run"),
        permit_digest: stable_sha256("historical-permit"),
        applied: true,
        after_snapshot: snapshot,
        after_lease: lease,
        attestation: None,
    };
    application.attestation = Some(
        issuer
            .issue(
                ReceiptClass::ToolExecution,
                SubjectRevision::checked(&request.source_revision).unwrap(),
                ReceiptScope::checked(dgcl_repair_application_scope(&request)).unwrap(),
                dgcl_repair_application_payload_digest(&application),
                100,
                Some(150),
                None,
            )
            .unwrap(),
    );
    let values = [
        parsed(&request.source_revision),
        parsed(&request.source_revision),
        parsed(&stable_sha256("scope")),
        coding_plan().digest(),
        parsed(&stable_sha256("goal")),
        parsed(&stable_sha256("policy")),
        parsed(&application.request_digest),
        parsed(&request.plan_digest),
        parsed(&stable_sha256(&request.target_ref)),
        parsed(&request.target_digest_before),
        parsed(&request.candidate_digest),
        parsed(&application.after_snapshot.snapshot_digest),
        parsed(&dgcl_repair_application_payload_digest(&application)),
        parsed(&application.attestation.as_ref().unwrap().receipt_digest()),
    ];
    let mut bytes = independent_wire();
    for (slot, pin) in bytes[16..16 + 14 * 32].chunks_exact_mut(32).zip(values) {
        slot.copy_from_slice(pin.as_bytes());
    }
    let checkpoint = UntrustedTransitionCheckpoint::decode(&bytes).unwrap();
    let attestation = issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(&request.source_revision).unwrap(),
            ReceiptScope::checked(checkpoint.receipt_scope()).unwrap(),
            digest_text(checkpoint.payload_digest()),
            110,
            Some(160),
            Some(application.attestation.as_ref().unwrap().receipt_digest()),
        )
        .unwrap();
    SigningFixture {
        request,
        application,
        checkpoint,
        attestation,
        verifier,
    }
}
fn coding_plan() -> lc631_runtime::contextual_repair::measurement::CodingProbePlan {
    lc631_runtime::contextual_repair::measurement::CodingProbePlan::checked_add(
        parsed(&stable_sha256("goal")),
        parsed(&stable_sha256("policy")),
    )
    .unwrap()
}
fn segment(checkpoint: &UntrustedTransitionCheckpoint) -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            JournalRecordKind::Evidence,
            checkpoint.payload_digest(),
            checkpoint.as_bytes(),
        )
        .unwrap();
    builder.finish().unwrap()
}
fn with_resources<T>(operation: impl FnOnce(&ResourceReservationView<'_>) -> T) -> T {
    let limits = ResourceLimits::uniform(1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            32,
            SourceRevision::of(SOURCE).digest(),
            limits,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[
            ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(262_144).unwrap()),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(262_144).unwrap(),
            ),
        ])
        .unwrap();
    let result = operation(&reservation.lease());
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    result
}
fn inspect(
    fixture: &SigningFixture,
    bytes: &[u8],
    now_epoch: u64,
    view: &ResourceReservationView<'_>,
) -> Result<lc631_runtime::contextual_resume::transition::HistoricalRepairTransition, TransitionError>
{
    inspect_transition_chain(
        &[bytes],
        None,
        JournalChainLimits::checked(1, bytes.len()).unwrap(),
        TransitionEvidence {
            request: &fixture.request,
            application: &fixture.application,
            transition_attestation: &fixture.attestation,
            verifier: &fixture.verifier,
            now_epoch,
        },
        view,
    )
}

#[test]
fn tlcx32_expired_authentic_history_is_history_only_and_changed_target_keeps_task_source() {
    let fixture = signing_fixture();
    let bytes = segment(&fixture.checkpoint);
    with_resources(|view| {
        let history = inspect(&fixture, &bytes, 250, view).unwrap();
        assert_eq!(history.task_source(), SourceRevision::of(SOURCE).digest());
        assert_eq!(history.before_target(), parsed(&stable_sha256(BEFORE)));
        assert_eq!(history.after_target(), parsed(&stable_sha256(AFTER)));
        assert_eq!(
            history.original_request_root(),
            Some(parsed(&stable_sha256("fixture-root")))
        );
        assert_eq!(
            history.original_plan_digest(),
            parsed(&fixture.request.plan_digest)
        );
        assert_eq!(
            history.after_profile(),
            parsed(&fixture.application.after_snapshot.profile_digest)
        );
        assert_eq!(
            history.after_validator(),
            parsed(&fixture.application.after_snapshot.validator_digest)
        );
        assert_eq!(
            history.after_targets(),
            parsed(&fixture.application.after_snapshot.targets_digest)
        );
        assert_ne!(history.before_target(), history.after_target());
        assert!(history
            .claim_boundary()
            .contains("effects/delivery unknown"));
        assert!(history
            .claim_boundary()
            .contains("no authority, completion, quota"));
        assert_eq!(
            history.chain().head_state(),
            JournalChainHeadState::Unanchored
        );
    });
}

#[test]
fn tlcx32_signed_legacy_missing_root_cannot_take_a_resume_callers_root() {
    use lc631_runtime::contextual_repair::snapshot_gate::{
        CodingBuildPort, ExpectedCurrentBuild, SnapshotGateError,
    };
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let mut fixture = signing_fixture();
    fixture.request.repository_root_digest = None;
    fixture.request.plan_digest = pipeline.completion_plan.plan_digest.clone();
    fixture.application.plan_digest = fixture.request.plan_digest.clone();
    fixture.application.request_digest = dgcl_repair_request_digest(&fixture.request);
    let issuer = ReceiptIssuer::from_key_bytes("tlcx32-transition-fixture", &[93; 32]).unwrap();
    fixture.application.attestation = Some(
        issuer
            .issue(
                ReceiptClass::ToolExecution,
                SubjectRevision::checked(&fixture.request.source_revision).unwrap(),
                ReceiptScope::checked(dgcl_repair_application_scope(&fixture.request)).unwrap(),
                dgcl_repair_application_payload_digest(&fixture.application),
                100,
                Some(150),
                None,
            )
            .unwrap(),
    );
    let parent = fixture
        .application
        .attestation
        .as_ref()
        .unwrap()
        .receipt_digest();
    let mut raw = fixture.checkpoint.as_bytes().to_vec();
    for (index, value) in [
        (6, &fixture.application.request_digest),
        (7, &fixture.request.plan_digest),
        (
            12,
            &dgcl_repair_application_payload_digest(&fixture.application),
        ),
        (13, &parent),
    ] {
        raw[16 + index * 32..16 + (index + 1) * 32].copy_from_slice(parsed(value).as_bytes());
    }
    fixture.checkpoint = UntrustedTransitionCheckpoint::decode(&raw).unwrap();
    fixture.attestation = issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(&fixture.request.source_revision).unwrap(),
            ReceiptScope::checked(fixture.checkpoint.receipt_scope()).unwrap(),
            digest_text(fixture.checkpoint.payload_digest()),
            110,
            Some(160),
            Some(parent),
        )
        .unwrap();
    let bytes = segment(&fixture.checkpoint);
    struct NeverCapture;
    impl ToolProcessRunner for NeverCapture {
        fn run(
            &mut self,
            _: &str,
            _: BoundedToolRequest<'_>,
            _: &mut dyn FnMut(&ResourceToolResult<'_>) -> Result<(), ResourceError>,
        ) -> Result<(), ResourceError> {
            panic!("a missing original root cannot execute a process");
        }
        fn with_input(
            &mut self,
            _: &str,
            _: usize,
            _: &mut dyn FnMut(&ResourceReservationView<'_>) -> Result<(), ResourceError>,
        ) -> Result<(), ResourceError> {
            panic!("a missing original root cannot start a source capture");
        }
    }
    with_resources(|view| {
        let history = inspect(&fixture, &bytes, 250, view).unwrap();
        assert_eq!(history.original_request_root(), None);
        let root = std::env::current_dir().unwrap();
        let mut runner = NeverCapture;
        // An intentionally supplied current caller root cannot fill the gap in
        // the original signed request, even though this descriptor is valid.
        let port = CodingBuildPort::checked(
            &root,
            SOURCE,
            &pipeline,
            parsed(&dgcl_repository_root_digest(&root).unwrap()),
            parsed(&pipeline.program_ir.program_digest),
            parsed(&pipeline.completion_plan.plan_digest),
            &mut runner,
        )
        .unwrap();
        assert!(matches!(
            ExpectedCurrentBuild::from_verified_history(&history, &port),
            Err(SnapshotGateError::MissingOriginalRoot)
        ));
    });
}

#[test]
fn tlcx32_modified_probe_plan_with_rehashed_local_chain_cannot_keep_old_attestation() {
    let fixture = signing_fixture();
    let mut raw = fixture.checkpoint.as_bytes().to_vec();
    raw[16 + 3 * 32] ^= 1;
    let altered = UntrustedTransitionCheckpoint::decode(&raw).unwrap();
    let bytes = segment(&altered);
    with_resources(|view| {
        assert!(matches!(
            inspect(&fixture, &bytes, 250, view),
            Err(TransitionError::ReceiptRejected)
        ));
    });
}

#[test]
fn tlcx32_swapped_application_target_missing_attestation_and_clock_rollback_are_not_resumed() {
    let mut fixture = signing_fixture();
    let bytes = segment(&fixture.checkpoint);
    with_resources(|view| {
        assert!(matches!(
            inspect(&fixture, &bytes, 109, view),
            Err(TransitionError::ClockRollback)
        ));
        fixture.application.target_digest_after = stable_sha256("foreign target");
        assert!(matches!(
            inspect(&fixture, &bytes, 250, view),
            Err(TransitionError::BindingMismatch)
        ));
        fixture.application.target_digest_after = fixture.request.candidate_digest.clone();
        fixture.application.attestation = None;
        assert!(matches!(
            inspect(&fixture, &bytes, 250, view),
            Err(TransitionError::MissingAttestation)
        ));
    });
}

#[test]
fn tlcx32_even_resigned_not_applied_application_is_rejected() {
    let mut fixture = signing_fixture();
    fixture.application.applied = false;
    let issuer = ReceiptIssuer::from_key_bytes("tlcx32-transition-fixture", &[93; 32]).unwrap();
    fixture.application.attestation = Some(
        issuer
            .issue(
                ReceiptClass::ToolExecution,
                SubjectRevision::checked(&fixture.request.source_revision).unwrap(),
                ReceiptScope::checked(dgcl_repair_application_scope(&fixture.request)).unwrap(),
                dgcl_repair_application_payload_digest(&fixture.application),
                100,
                Some(150),
                None,
            )
            .unwrap(),
    );
    // Rebind and re-sign every outer field too. The legacy application digest
    // does not contain `applied`; the explicit semantic check must reject it.
    let parent = fixture
        .application
        .attestation
        .as_ref()
        .unwrap()
        .receipt_digest();
    let mut raw = fixture.checkpoint.as_bytes().to_vec();
    raw[16 + 13 * 32..16 + 14 * 32].copy_from_slice(parsed(&parent).as_bytes());
    fixture.checkpoint = UntrustedTransitionCheckpoint::decode(&raw).unwrap();
    fixture.attestation = issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(&fixture.request.source_revision).unwrap(),
            ReceiptScope::checked(fixture.checkpoint.receipt_scope()).unwrap(),
            digest_text(fixture.checkpoint.payload_digest()),
            110,
            Some(160),
            Some(parent),
        )
        .unwrap();
    let bytes = segment(&fixture.checkpoint);
    with_resources(|view| {
        assert!(matches!(
            inspect(&fixture, &bytes, 250, view),
            Err(TransitionError::BindingMismatch)
        ));
    });
}

#[test]
fn tlcx32_oversized_report_is_rejected_before_canonical_hashing() {
    let mut fixture = signing_fixture();
    fixture.request.target_ref = "x".repeat(1025);
    let bytes = segment(&fixture.checkpoint);
    with_resources(|view| {
        assert!(matches!(
            inspect(&fixture, &bytes, 250, view),
            Err(TransitionError::InputLimit)
        ));
    });
}

#[test]
fn tlcx32_fresh_reselection_rejects_rebound_goal_before_any_native_target_is_opened() {
    use lc631_core::{
        context::ContextRevision, Action, AuthorityRevision, CallerOrigin, ExecutionPermitContext,
    };
    use lc631_runtime::contextual_repair::{execution::*, measurement::CodingProbePlan};
    use lc631_runtime::contextual_resume::fresh_reselection::*;
    let fixture = signing_fixture();
    let bytes = segment(&fixture.checkpoint);
    let issuer = ReceiptIssuer::from_key_bytes("tlcx32-native-fixture", &[94; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx32-native-fixture", &[94; 32]).unwrap();
    let wrong = CodingProbePlan::checked_add(
        parsed(&stable_sha256("rebound-goal")),
        parsed(&stable_sha256("policy")),
    )
    .unwrap();
    let snapshot = &fixture.application.after_snapshot;
    let lease = issue_dgcl_evidence_lease(snapshot, 250, 100).unwrap();
    let absent = std::env::temp_dir().join("tlcx32-never-open-this-unregistered-target.rs");
    let native = std::env::temp_dir().join("tlcx32-never-spawn-this-native-artifact");
    let binary_digest = stable_sha256("unexecuted binary");
    let registration = || ProbeToolRegistration {
        program: native.to_str().unwrap(),
        executable_digest: &binary_digest,
        target_path: &absent,
        target_ref: "src/lib.rs",
        snapshot,
    };
    let scope = registration().execution_scope(&wrong);
    let permit = authority_fixture::permit(SOURCE, &scope, &issuer, &verifier, Action::Test);
    let authority = ExecutionPermitContext {
        principal: permit.principal_binding(),
        action: Action::Test,
        scope: &scope,
        source_revision: &snapshot.source_revision,
        authority_revision: AuthorityRevision(1),
        now_epoch: 250,
        revocation_revision: 0,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: verifier.key_fingerprint(),
    };
    let tool = issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(&snapshot.source_revision).unwrap(),
            ReceiptScope::checked(registration().receipt_scope(&wrong)).unwrap(),
            registration().payload_digest(&wrong),
            250,
            Some(350),
            None,
        )
        .unwrap();
    fn current_epoch() -> u64 {
        250
    }
    let invocation = || ProbeInvocation {
        source: SOURCE,
        registration: registration(),
        lease: &lease,
        tool_receipt: &tool,
        tool_verifier: &verifier,
        permit: &permit,
        authority: &authority,
        clock: current_epoch,
        timeout_ms: 1000,
    };
    with_resources(|view| {
        let history = inspect(&fixture, &bytes, 250, view).unwrap();
        let result = resume_reselect_coding(
            FreshCodingResumeRequest {
                history,
                context: ContextRevision::original(SourceRevision::of(SOURCE)),
                comparison_scope: parsed(&stable_sha256("scope")),
                plan: &wrong,
                baseline: invocation(),
                current: invocation(),
            },
            view,
        );
        assert!(matches!(result, Err(FreshResumeError::BindingMismatch)));
    });
}
