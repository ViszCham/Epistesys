//! 準備契約の独立表。未実行の試験を成功扱いしない。
//! Independent preparation-contract table; unexecuted tests are not passes.
use lc631_runtime::contextual_resume;
use lc631_runtime::journal;

use contextual_resume::*;
use lc631_analysis::{
    CheckpointBody, CheckpointEventKind as Event, CheckpointReadIdentity, DeliveryState as Delivery,
};
use lc631_core::{
    context::{ContextRevision, SourceRevision},
    stable_sha256,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_substrate::Digest32;
use std::time::Duration;

fn pins() -> ReportedContextualPins {
    ReportedContextualPins {
        context: ContextRevision::original(SourceRevision::of("tlcx32-source")),
        comparison_scope: Digest32::from_bytes([1; 32]),
        solver: Digest32::from_bytes([2; 32]),
        options: Digest32::from_bytes([3; 32]),
        tool: Digest32::from_bytes([4; 32]),
        validator: Digest32::from_bytes([5; 32]),
        measurement_definition: Digest32::from_bytes([6; 32]),
        policy: Digest32::from_bytes([7; 32]),
        evidence: Digest32::from_bytes([8; 32]),
    }
}

fn segment(source: &str) -> Vec<u8> {
    let body = CheckpointBody {
        schema_version: "epistesys-dgcl-checkpoint.v1".into(),
        sequence: 0,
        source_revision: source.into(),
        plan_digest: stable_sha256("tlcx32-plan"),
        authority_revision: "tlcx32-authority-report".into(),
        action_key: "tlcx32-action".into(),
        event_kind: Event::LegacyUnclassified,
        delivery: Delivery::ConfirmedSuccess,
        result_digest: Some(stable_sha256("tlcx32-result")),
        parent_digest: None,
    };
    let canonical = serde_json::to_string(&body).unwrap();
    let payload = serde_json::to_vec(&serde_json::json!({
        "body": body, "digest": stable_sha256(&canonical),
    }))
    .unwrap();
    let mut builder = journal::JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            journal::JournalRecordKind::Checkpoint,
            Digest32::from_bytes([9; 32]),
            &payload,
        )
        .unwrap();
    builder.finish().unwrap()
}

#[test]
fn tlcx32_unknown_effect_or_delivery_never_becomes_retry_or_send() {
    use EffectReconciliation::*;
    // Hand-written requirements matrix, deliberately independent of classifier control flow.
    let rows = [
        (Event::LegacyUnclassified, [LegacyEffectUnknown; 3]),
        (Event::ActionStarted, [ReconcileActionWithoutReapplying; 3]),
        (
            Event::ActionObserved,
            [
                ReconcileActionWithoutReapplying,
                FailedActionRequiresFreshIntentPermissionAndStateCheck,
                RevalidateObservedActionWithoutReapplying,
            ],
        ),
        (
            Event::CandidatePrepared,
            [RebuildCandidateWithoutSending; 3],
        ),
        (
            Event::CandidateSendAttempted,
            [ReconcileDeliveryWithoutResending; 3],
        ),
        (
            Event::SinkDeliveryObserved,
            [RevalidateHistoricalSinkWithoutResending; 3],
        ),
    ];
    for (event, expected) in rows {
        for (index, delivery) in [
            Delivery::UnknownDelivery,
            Delivery::ConfirmedFailed,
            Delivery::ConfirmedSuccess,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(reconcile_effect(event, delivery), expected[index]);
        }
    }
}

#[test]
fn tlcx32_real_chain_changed_context_and_matching_reports_both_need_fresh_checks() {
    let historical = pins();
    let source = stable_sha256("tlcx32-source");
    let plan = stable_sha256("tlcx32-plan");
    let bytes = segment(&source);
    let segments = [bytes.as_slice()];
    let limits = journal::JournalChainLimits::checked(1, bytes.len()).unwrap();
    let budget = ResourceLimits::checked([65536, 0, 0, 0, 0, 0]).unwrap();
    let broker = ResourceBroker::new(budget).unwrap();
    let cancel = Cancellation::default();
    let mut run = broker
        .begin_run(
            32,
            historical.context.source().digest(),
            budget,
            Deadline::after(Duration::from_secs(10)).unwrap(),
            cancel.clone(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(&[ResourceRequest::new(
            ResourceKind::Ram,
            ResourceAmount::checked(65536).unwrap(),
        )])
        .unwrap();
    {
        let lease = reservation.lease();
        let identity = CheckpointReadIdentity {
            source_revision: &source,
            plan_digest: &plan,
            authority_revision: "tlcx32-authority-report",
        };
        let same = prepare_contextual_resume(
            &segments,
            None,
            limits,
            identity,
            &historical,
            &historical,
            &lease,
        )
        .unwrap();
        assert_eq!(same.required_checks().len(), 8);
        assert!(same
            .required_checks()
            .contains(&RequiredCheck::ContextualPayloadBinding));
        assert!(same.observation().fresh_revalidation_required);
        assert!(!same.observation().completion_restored);
        assert!(!same.observation().authority_created);
        assert!(PinKind::ALL
            .into_iter()
            .all(|kind| !same.reported_pin_changed(kind)));
        assert_eq!(
            same.reconciliation(),
            EffectReconciliation::LegacyEffectUnknown
        );
        assert!(same.claim_boundary().contains("NeedsEvidence"));
        let mut changed = historical;
        changed.context = changed.context.reconstructed([42; 32]);
        changed.validator = Digest32::from_bytes([43; 32]);
        changed.policy = Digest32::from_bytes([44; 32]);
        let stale = prepare_contextual_resume(
            &segments,
            None,
            limits,
            identity,
            &historical,
            &changed,
            &lease,
        )
        .unwrap();
        assert!(stale.reported_pin_changed(PinKind::Context));
        assert!(stale.reported_pin_changed(PinKind::Validator));
        assert!(stale.reported_pin_changed(PinKind::Policy));
        assert_eq!(stale.required_checks(), same.required_checks());
        let wrong = CheckpointReadIdentity {
            source_revision: &source,
            plan_digest: "wrong-plan",
            authority_revision: "tlcx32-authority-report",
        };
        assert!(matches!(
            prepare_contextual_resume(
                &segments,
                None,
                limits,
                wrong,
                &historical,
                &historical,
                &lease
            ),
            Err(ContextualResumeHold::Chain(_))
        ));
        cancel.cancel();
        assert!(matches!(
            prepare_contextual_resume(
                &segments,
                None,
                limits,
                identity,
                &historical,
                &historical,
                &lease
            ),
            Err(ContextualResumeHold::Resource(_))
        ));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}
