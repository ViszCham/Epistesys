use lc631_analysis::{
    inspect_checkpoint_record_bytes, CheckpointBody, CheckpointEventKind, CheckpointIntegrityState,
    CheckpointReadIdentity, DeliveryState, ResumeError,
};

fn source_revision(label: &str) -> String {
    lc631_core::stable_sha256(label)
}

fn identity<'a>(source: &'a str, plan: &'a str, authority: &'a str) -> CheckpointReadIdentity<'a> {
    CheckpointReadIdentity {
        source_revision: source,
        plan_digest: plan,
        authority_revision: authority,
    }
}

// The fixture varies every wire field independently for mutation regressions.
#[allow(clippy::too_many_arguments)]
fn body(
    schema_version: &str,
    sequence: u64,
    source: &str,
    plan: &str,
    authority: &str,
    action: &str,
    event_kind: CheckpointEventKind,
    delivery: DeliveryState,
    result_digest: Option<String>,
    parent_digest: Option<String>,
) -> CheckpointBody {
    CheckpointBody {
        schema_version: schema_version.to_string(),
        sequence,
        source_revision: source.to_string(),
        plan_digest: plan.to_string(),
        authority_revision: authority.to_string(),
        action_key: action.to_string(),
        event_kind,
        delivery,
        result_digest,
        parent_digest,
    }
}

fn stored_record(body: &CheckpointBody) -> (Vec<u8>, String) {
    let canonical_body = serde_json::to_string(body).unwrap();
    let digest = lc631_core::stable_sha256(&canonical_body);
    let stored = serde_json::json!({ "body": body, "digest": digest });
    (serde_json::to_vec(&stored).unwrap(), digest)
}

fn valid_legacy_record(source: &str, plan: &str, authority: &str) -> (Vec<u8>, String) {
    let body = body(
        "epistesys-dgcl-checkpoint.v1",
        0,
        source,
        plan,
        authority,
        "repair-1",
        CheckpointEventKind::LegacyUnclassified,
        DeliveryState::ConfirmedSuccess,
        Some(lc631_core::stable_sha256("result-1")),
        None,
    );
    stored_record(&body)
}

#[test]
fn tlcx07_checkpoint_bytes_returns_an_unanchored_fresh_revalidation_observation() {
    let source = source_revision("source");
    let plan = source_revision("plan");
    let authority = "authority-revision-1";
    let (record, _) = valid_legacy_record(&source, &plan, authority);

    let observation =
        inspect_checkpoint_record_bytes(&[record.as_slice()], identity(&source, &plan, authority))
            .unwrap();
    assert_eq!(
        observation.integrity_state,
        CheckpointIntegrityState::Unanchored
    );
    assert_eq!(observation.latest_sequence, Some(0));
    assert_eq!(
        observation.latest_delivery,
        Some(DeliveryState::ConfirmedSuccess)
    );
    assert!(observation.fresh_revalidation_required);
    assert!(!observation.authority_created);
    assert!(!observation.completion_restored);
}

#[test]
fn tlcx07_checkpoint_bytes_revalidates_source_plan_and_authority_identity() {
    let source = source_revision("source");
    let plan = source_revision("plan");
    let authority = "authority-revision-1";
    let (record, _) = valid_legacy_record(&source, &plan, authority);

    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[record.as_slice()],
            identity(&source_revision("other-source"), &plan, authority),
        ),
        Err(ResumeError::StaleSource)
    );
    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[record.as_slice()],
            identity(&source, &source_revision("other-plan"), authority),
        ),
        Err(ResumeError::StalePlan)
    );
    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[record.as_slice()],
            identity(&source, &plan, "authority-revision-2"),
        ),
        Err(ResumeError::AuthorityChanged)
    );
}

#[test]
fn tlcx07_checkpoint_bytes_holds_on_bad_digest_order_and_parent_chain() {
    let source = source_revision("source");
    let plan = source_revision("plan");
    let authority = "authority-revision-1";
    let (good, good_digest) = valid_legacy_record(&source, &plan, authority);

    let mut wrong_digest: serde_json::Value = serde_json::from_slice(&good).unwrap();
    wrong_digest["digest"] = serde_json::Value::String(lc631_core::stable_sha256("wrong"));
    let wrong_digest = serde_json::to_vec(&wrong_digest).unwrap();
    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[wrong_digest.as_slice()],
            identity(&source, &plan, authority),
        ),
        Err(ResumeError::ChainMismatch)
    );

    let later_body = body(
        "epistesys-dgcl-checkpoint.v1",
        1,
        &source,
        &plan,
        authority,
        "repair-1",
        CheckpointEventKind::LegacyUnclassified,
        DeliveryState::ConfirmedSuccess,
        Some(lc631_core::stable_sha256("result-2")),
        Some(good_digest),
    );
    let (later, _) = stored_record(&later_body);
    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[later.as_slice(), good.as_slice()],
            identity(&source, &plan, authority),
        ),
        Err(ResumeError::ChainMismatch)
    );

    let (bad_parent, _) = stored_record(&body(
        "epistesys-dgcl-checkpoint.v1",
        1,
        &source,
        &plan,
        authority,
        "repair-1",
        CheckpointEventKind::LegacyUnclassified,
        DeliveryState::ConfirmedSuccess,
        Some(lc631_core::stable_sha256("result-3")),
        Some(lc631_core::stable_sha256("wrong-parent")),
    ));
    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[good.as_slice(), bad_parent.as_slice()],
            identity(&source, &plan, authority),
        ),
        Err(ResumeError::ChainMismatch)
    );
}

#[test]
fn tlcx07_checkpoint_bytes_holds_unknown_and_invalid_event_transitions() {
    let source = source_revision("source");
    let plan = source_revision("plan");
    let authority = "authority-revision-1";
    let started = body(
        "epistesys-dgcl-checkpoint.v2",
        0,
        &source,
        &plan,
        authority,
        "repair-1",
        CheckpointEventKind::ActionStarted,
        DeliveryState::UnknownDelivery,
        None,
        None,
    );
    let (first, first_digest) = stored_record(&started);
    let invalid_next = body(
        "epistesys-dgcl-checkpoint.v2",
        1,
        &source,
        &plan,
        authority,
        "repair-1",
        CheckpointEventKind::CandidatePrepared,
        DeliveryState::UnknownDelivery,
        None,
        Some(first_digest),
    );
    let (second, _) = stored_record(&invalid_next);

    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[first.as_slice(), second.as_slice()],
            identity(&source, &plan, authority),
        ),
        Err(ResumeError::InvalidEventTransition)
    );

    let empty = CheckpointBody {
        schema_version: "epistesys-dgcl-checkpoint.v1".into(),
        sequence: 0,
        source_revision: source,
        plan_digest: plan,
        authority_revision: authority.into(),
        action_key: "repair-1".into(),
        event_kind: CheckpointEventKind::LegacyUnclassified,
        delivery: DeliveryState::UnknownDelivery,
        result_digest: None,
        parent_digest: None,
    };
    let (unknown, _) = stored_record(&empty);
    let observation = inspect_checkpoint_record_bytes(
        &[unknown.as_slice()],
        identity(
            &empty.source_revision,
            &empty.plan_digest,
            &empty.authority_revision,
        ),
    )
    .unwrap();
    assert_eq!(
        observation.latest_delivery,
        Some(DeliveryState::UnknownDelivery)
    );
    assert!(observation.fresh_revalidation_required);
    assert!(!observation.completion_restored);
}

#[test]
fn tlcx07_checkpoint_bytes_enforces_count_and_per_record_caps() {
    let source = source_revision("source");
    let plan = source_revision("plan");
    let authority = "authority-revision-1";
    let (record, _) = valid_legacy_record(&source, &plan, authority);
    let too_many = vec![record.as_slice(); 1_001];
    assert_eq!(
        inspect_checkpoint_record_bytes(&too_many, identity(&source, &plan, authority)),
        Err(ResumeError::BudgetExceeded)
    );

    let large_record = vec![b'x'; 16 * 1024 + 1];
    assert_eq!(
        inspect_checkpoint_record_bytes(
            &[large_record.as_slice()],
            identity(&source, &plan, authority)
        ),
        Err(ResumeError::BudgetExceeded)
    );
}
