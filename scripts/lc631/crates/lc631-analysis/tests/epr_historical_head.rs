use lc631_analysis::{
    append_checkpoint, inspect_resume_with_anchor, inspect_resume_with_historical_anchor,
    issue_checkpoint_head_anchor, load_checkpoint_head, load_checkpoint_head_historical,
    persist_checkpoint_head, persist_checkpoint_head_renewal, CheckpointBody, CheckpointEventKind,
    CheckpointHeadAnchor, CheckpointHeadRequest, CheckpointIntegrityState, DeliveryState,
    ResumeError, DGCL_CHECKPOINT_V2_SCHEMA,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptScope, ReceiptVerifier, SubjectRevision,
};
use std::path::PathBuf;

fn fixture() -> (
    PathBuf,
    PathBuf,
    ReceiptIssuer,
    ReceiptVerifier,
    CheckpointHeadAnchor,
) {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!("epr-history-{}-{unique}", std::process::id()));
    let ledger = base.join("ledger");
    let heads = base.join("heads");
    std::fs::create_dir_all(&ledger).unwrap();
    std::fs::create_dir_all(&heads).unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("epr-history-fixture", &[83; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("epr-history-fixture", &[83; 32]).unwrap();
    let source = stable_sha256("source");
    let plan = stable_sha256("workflow");
    let digest = append_checkpoint(
        &ledger,
        CheckpointBody {
            schema_version: DGCL_CHECKPOINT_V2_SCHEMA.into(),
            sequence: 0,
            source_revision: source.clone(),
            plan_digest: plan.clone(),
            authority_revision: "1".into(),
            action_key: stable_sha256("request"),
            event_kind: CheckpointEventKind::ActionStarted,
            delivery: DeliveryState::UnknownDelivery,
            result_digest: None,
            parent_digest: None,
        },
    )
    .unwrap();
    let anchor = issue_checkpoint_head_anchor(
        &issuer,
        CheckpointHeadRequest {
            source_revision: &source,
            plan_digest: &plan,
            authority_revision: "1",
            sequence: Some(0),
            head_digest: Some(&digest),
            now_epoch: 100,
            lease_seconds: 10,
        },
    )
    .unwrap();
    (ledger, heads, issuer, verifier, anchor)
}

#[test]
fn epr_11_head_rejects_receipt_calendar_different_from_its_signed_payload_calendar() {
    let (ledger, heads, issuer, verifier, mut anchor) = fixture();
    let payload = anchor.attestation.claims().payload_digest().to_string();
    anchor.attestation = issuer
        .issue(
            ReceiptClass::Closure,
            SubjectRevision::checked(&anchor.source_revision).unwrap(),
            ReceiptScope::checked("dgcl/checkpoint/head/0").unwrap(),
            payload,
            90,
            Some(200),
            None,
        )
        .unwrap();
    assert_eq!(
        persist_checkpoint_head(&ledger, &heads, &anchor, &verifier, 100),
        Err(ResumeError::TrustedHeadInvalid)
    );
}

#[test]
fn epr_11_expired_authentic_history_never_restores_authority_or_completion() {
    let (ledger, heads, _issuer, verifier, anchor) = fixture();
    persist_checkpoint_head(&ledger, &heads, &anchor, &verifier, 100).unwrap();
    assert_eq!(
        load_checkpoint_head(&ledger, &heads, &verifier, 200),
        Err(ResumeError::TrustedHeadExpired)
    );
    let history = load_checkpoint_head_historical(&ledger, &heads, &verifier, 200).unwrap();
    let observation = inspect_resume_with_historical_anchor(
        &ledger,
        &anchor.source_revision,
        &anchor.plan_digest,
        &anchor.authority_revision,
        Some(&history),
        Some(&verifier),
        200,
    )
    .unwrap();
    assert_eq!(
        observation.integrity_state,
        CheckpointIntegrityState::HistoricalHeadMatched
    );
    assert!(observation.fresh_revalidation_required);
    assert!(!observation.authority_created);
    assert!(!observation.completion_restored);
    assert!(!observation.retry_permitted_by_prior_result);
    assert_eq!(
        inspect_resume_with_anchor(
            &ledger,
            &anchor.source_revision,
            &anchor.plan_digest,
            &anchor.authority_revision,
            Some(&history),
            Some(&verifier),
            200
        ),
        Err(ResumeError::TrustedHeadExpired)
    );
}

#[test]
fn epr_11_historical_head_still_rejects_wrong_identity_clock_key_and_unanchored_tail() {
    let (ledger, heads, _issuer, verifier, anchor) = fixture();
    persist_checkpoint_head(&ledger, &heads, &anchor, &verifier, 100).unwrap();
    assert!(load_checkpoint_head_historical(&ledger, &heads, &verifier, 99).is_err());
    let wrong = ReceiptVerifier::from_key_bytes("epr-history-fixture", &[84; 32]).unwrap();
    assert!(load_checkpoint_head_historical(&ledger, &heads, &wrong, 200).is_err());
    assert_eq!(
        inspect_resume_with_historical_anchor(
            &ledger,
            &stable_sha256("other source"),
            &anchor.plan_digest,
            &anchor.authority_revision,
            Some(&anchor),
            Some(&verifier),
            200
        ),
        Err(ResumeError::StaleSource)
    );
    assert_eq!(
        inspect_resume_with_historical_anchor(
            &ledger,
            &anchor.source_revision,
            &stable_sha256("other workflow"),
            &anchor.authority_revision,
            Some(&anchor),
            Some(&verifier),
            200
        ),
        Err(ResumeError::StalePlan)
    );
    assert_eq!(
        inspect_resume_with_historical_anchor(
            &ledger,
            &anchor.source_revision,
            &anchor.plan_digest,
            "changed authority",
            Some(&anchor),
            Some(&verifier),
            200
        ),
        Err(ResumeError::AuthorityChanged)
    );
    append_checkpoint(
        &ledger,
        CheckpointBody {
            schema_version: DGCL_CHECKPOINT_V2_SCHEMA.into(),
            sequence: 1,
            source_revision: anchor.source_revision.clone(),
            plan_digest: anchor.plan_digest.clone(),
            authority_revision: anchor.authority_revision.clone(),
            action_key: stable_sha256("request"),
            event_kind: CheckpointEventKind::ActionObserved,
            delivery: DeliveryState::ConfirmedSuccess,
            result_digest: Some(stable_sha256("observed")),
            parent_digest: anchor.head_digest.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        inspect_resume_with_historical_anchor(
            &ledger,
            &anchor.source_revision,
            &anchor.plan_digest,
            &anchor.authority_revision,
            Some(&anchor),
            Some(&verifier),
            200
        ),
        Err(ResumeError::UnanchoredTail)
    );
}

#[test]
fn epr_11_explicit_renewal_preserves_the_exact_historical_head_and_strict_current_checks() {
    let (ledger, heads, issuer, verifier, anchor) = fixture();
    persist_checkpoint_head(&ledger, &heads, &anchor, &verifier, 100).unwrap();
    let renewed = issue_checkpoint_head_anchor(
        &issuer,
        CheckpointHeadRequest {
            source_revision: &anchor.source_revision,
            plan_digest: &anchor.plan_digest,
            authority_revision: &anchor.authority_revision,
            sequence: anchor.sequence,
            head_digest: anchor.head_digest.as_deref(),
            now_epoch: 200,
            lease_seconds: 100,
        },
    )
    .unwrap();
    persist_checkpoint_head_renewal(&ledger, &heads, &renewed, &verifier, 200).unwrap();
    let loaded = load_checkpoint_head(&ledger, &heads, &verifier, 201).unwrap();
    assert_eq!(loaded.head_digest, anchor.head_digest);
    assert_eq!(loaded.sequence, anchor.sequence);
    assert_eq!(loaded.issued_at_epoch, 200);
    let observed = inspect_resume_with_anchor(
        &ledger,
        &anchor.source_revision,
        &anchor.plan_digest,
        &anchor.authority_revision,
        Some(&loaded),
        Some(&verifier),
        201,
    )
    .unwrap();
    assert_eq!(
        observed.integrity_state,
        CheckpointIntegrityState::TrustedHeadMatched
    );
    assert!(!observed.completion_restored && !observed.authority_created);
    assert!(observed.fresh_revalidation_required);
    assert_eq!(
        persist_checkpoint_head_renewal(&ledger, &heads, &renewed, &verifier, 201),
        Err(ResumeError::SequenceMismatch)
    );
}

#[test]
fn epr_11_historical_inspection_rejects_an_authentic_but_wrong_receipt_parent() {
    let (ledger, _heads, issuer, verifier, mut anchor) = fixture();
    let payload = anchor.attestation.claims().payload_digest().to_string();
    anchor.attestation = issuer
        .issue(
            ReceiptClass::Closure,
            SubjectRevision::checked(&anchor.source_revision).unwrap(),
            ReceiptScope::checked("dgcl/checkpoint/head/0").unwrap(),
            payload,
            anchor.issued_at_epoch,
            Some(anchor.expires_at_epoch),
            Some(stable_sha256("wrong parent")),
        )
        .unwrap();
    assert_eq!(
        inspect_resume_with_historical_anchor(
            &ledger,
            &anchor.source_revision,
            &anchor.plan_digest,
            &anchor.authority_revision,
            Some(&anchor),
            Some(&verifier),
            200
        ),
        Err(ResumeError::TrustedHeadInvalid)
    );
}

#[test]
fn epr_11_expired_history_does_not_hide_physical_ledger_rollback() {
    let (ledger, heads, issuer, verifier, original) = fixture();
    persist_checkpoint_head(&ledger, &heads, &original, &verifier, 100).unwrap();
    let digest = append_checkpoint(
        &ledger,
        CheckpointBody {
            schema_version: DGCL_CHECKPOINT_V2_SCHEMA.into(),
            sequence: 1,
            source_revision: original.source_revision.clone(),
            plan_digest: original.plan_digest.clone(),
            authority_revision: original.authority_revision.clone(),
            action_key: stable_sha256("request"),
            event_kind: CheckpointEventKind::ActionObserved,
            delivery: DeliveryState::ConfirmedSuccess,
            result_digest: Some(stable_sha256("observed")),
            parent_digest: original.head_digest.clone(),
        },
    )
    .unwrap();
    let latest = issue_checkpoint_head_anchor(
        &issuer,
        CheckpointHeadRequest {
            source_revision: &original.source_revision,
            plan_digest: &original.plan_digest,
            authority_revision: &original.authority_revision,
            sequence: Some(1),
            head_digest: Some(&digest),
            now_epoch: 200,
            lease_seconds: 10,
        },
    )
    .unwrap();
    persist_checkpoint_head(&ledger, &heads, &latest, &verifier, 200).unwrap();
    let path = ledger.join("checkpoint-00000000000000000001.json");
    let saved = ledger.parent().unwrap().join("saved-checkpoint.json");
    std::fs::rename(&path, &saved).unwrap();
    assert_eq!(
        inspect_resume_with_historical_anchor(
            &ledger,
            &original.source_revision,
            &original.plan_digest,
            &original.authority_revision,
            Some(&latest),
            Some(&verifier),
            500
        ),
        Err(ResumeError::RollbackDetected)
    );
    std::fs::rename(saved, path).unwrap();
    let observed = inspect_resume_with_historical_anchor(
        &ledger,
        &original.source_revision,
        &original.plan_digest,
        &original.authority_revision,
        Some(&latest),
        Some(&verifier),
        500,
    )
    .unwrap();
    assert_eq!(
        observed.integrity_state,
        CheckpointIntegrityState::HistoricalHeadMatched
    );
    assert!(!observed.authority_created && !observed.completion_restored);
}
