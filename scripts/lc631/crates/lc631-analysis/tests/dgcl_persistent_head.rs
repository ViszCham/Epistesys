use lc631_analysis::{
    append_checkpoint, inspect_resume_with_anchor, issue_checkpoint_head_anchor,
    load_checkpoint_head, persist_checkpoint_head, CheckpointBody, CheckpointEventKind,
    CheckpointHeadRequest, CheckpointIntegrityState, DeliveryState, ResumeError,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier};

#[test]
fn persisted_external_head_rejects_same_ledger_directory_and_checks_exact_resume() {
    let root = std::env::temp_dir().join(format!("dgcl-head-{}", std::process::id()));
    let ledger = root.join("ledger");
    let heads = root.join("trusted-heads");
    std::fs::create_dir_all(&ledger).unwrap();
    std::fs::create_dir_all(&heads).unwrap();
    let source = stable_sha256("Please test the package.");
    let plan = stable_sha256("plan");
    let key = [73; 32];
    let issuer = ReceiptIssuer::from_key_bytes("head-test", &key).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("head-test", &key).unwrap();
    let body = CheckpointBody {
        schema_version: "epistesys-dgcl-checkpoint.v2".into(),
        sequence: 0,
        source_revision: source.clone(),
        plan_digest: plan.clone(),
        authority_revision: "authority.v1".into(),
        action_key: stable_sha256("action"),
        event_kind: CheckpointEventKind::ActionStarted,
        delivery: DeliveryState::UnknownDelivery,
        result_digest: None,
        parent_digest: None,
    };
    let digest = append_checkpoint(&ledger, body).unwrap();
    let anchor = issue_checkpoint_head_anchor(
        &issuer,
        CheckpointHeadRequest {
            source_revision: &source,
            plan_digest: &plan,
            authority_revision: "authority.v1",
            sequence: Some(0),
            head_digest: Some(&digest),
            now_epoch: 100,
            lease_seconds: 120,
        },
    )
    .unwrap();
    assert_eq!(
        persist_checkpoint_head(&ledger, &ledger, &anchor, &verifier, 100),
        Err(ResumeError::RootUnsafe)
    );
    persist_checkpoint_head(&ledger, &heads, &anchor, &verifier, 100).unwrap();
    assert_eq!(
        persist_checkpoint_head(&ledger, &heads, &anchor, &verifier, 100),
        Err(ResumeError::SequenceMismatch)
    );
    let loaded = load_checkpoint_head(&ledger, &heads, &verifier, 110).unwrap();
    let observation = inspect_resume_with_anchor(
        &ledger,
        &source,
        &plan,
        "authority.v1",
        Some(&loaded),
        Some(&verifier),
        110,
    )
    .unwrap();
    assert_eq!(
        observation.integrity_state,
        CheckpointIntegrityState::TrustedHeadMatched
    );
    assert!(observation.fresh_revalidation_required);
    assert!(!observation.completion_restored);
    assert_eq!(
        load_checkpoint_head(&ledger, &heads, &verifier, 221),
        Err(ResumeError::TrustedHeadExpired)
    );
}

#[test]
fn a15_physical_checkpoint_rollback_missing_head_and_duplicate_are_not_completion() {
    let base = std::env::temp_dir().join(format!(
        "dgcl-rollback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let ledger = base.join("ledger");
    let heads = base.join("heads");
    let absent = base.join("absent-head");
    for root in [&ledger, &heads, &absent] {
        std::fs::create_dir_all(root).unwrap();
    }
    let source = stable_sha256("Please test the package.");
    let plan = stable_sha256("same-plan");
    let issuer = ReceiptIssuer::from_key_bytes("rollback-fixture", &[71; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("rollback-fixture", &[71; 32]).unwrap();
    let started = CheckpointBody {
        schema_version: "epistesys-dgcl-checkpoint.v2".into(),
        sequence: 0,
        source_revision: source.clone(),
        plan_digest: plan.clone(),
        authority_revision: "auth.v1".into(),
        action_key: stable_sha256("same-action"),
        event_kind: CheckpointEventKind::ActionStarted,
        delivery: DeliveryState::UnknownDelivery,
        result_digest: None,
        parent_digest: None,
    };
    let first = append_checkpoint(&ledger, started.clone()).unwrap();
    assert_eq!(
        append_checkpoint(&ledger, started.clone()),
        Err(ResumeError::SequenceMismatch)
    );
    let mut observed = started;
    observed.sequence = 1;
    observed.parent_digest = Some(first);
    observed.event_kind = CheckpointEventKind::ActionObserved;
    observed.delivery = DeliveryState::ConfirmedSuccess;
    observed.result_digest = Some(stable_sha256("actual local command result fixture"));
    let digest = append_checkpoint(&ledger, observed).unwrap();
    let anchor = issue_checkpoint_head_anchor(
        &issuer,
        CheckpointHeadRequest {
            source_revision: &source,
            plan_digest: &plan,
            authority_revision: "auth.v1",
            sequence: Some(1),
            head_digest: Some(&digest),
            now_epoch: 100,
            lease_seconds: 300,
        },
    )
    .unwrap();
    persist_checkpoint_head(&ledger, &heads, &anchor, &verifier, 100).unwrap();
    assert!(load_checkpoint_head(&ledger, &absent, &verifier, 110).is_err());
    let target = ledger.join("checkpoint-00000000000000000001.json");
    let saved = base.join("saved-checkpoint.json");
    std::fs::rename(&target, &saved).unwrap(); // reversible cut of test-owned bytes only
    assert_eq!(
        inspect_resume_with_anchor(
            &ledger,
            &source,
            &plan,
            "auth.v1",
            Some(&anchor),
            Some(&verifier),
            110
        ),
        Err(ResumeError::RollbackDetected)
    );
    std::fs::rename(&saved, &target).unwrap();
    let restored = inspect_resume_with_anchor(
        &ledger,
        &source,
        &plan,
        "auth.v1",
        Some(&anchor),
        Some(&verifier),
        110,
    )
    .unwrap();
    assert_eq!(
        restored.integrity_state,
        CheckpointIntegrityState::TrustedHeadMatched
    );
    assert!(restored.fresh_revalidation_required);
    assert!(!restored.completion_restored);
}
