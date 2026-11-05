//! 実binary readerからのrecovery観測。fresh署名も旧counterをresetしない。
//! Recovery through actual binary readers; fresh signatures never reset old counters.
#![allow(dead_code)]
use super::{parse, permit, Effects, EmptyValidation, Inputs, AFTER, SECOND, SOURCE};
use lc631_analysis::*;
use lc631_core::*;
use lc631_host::JournalBundleLimits;
use lc631_receipt_kernel::*;
use lc631_runtime::{contextual_resume::budget::*, journal::*, resource::*};
use lc631_substrate::Digest32;
use std::{path::Path, time::Duration};

pub fn resources(source: Digest32) -> (ResourceBroker, RunResources, Reservation) {
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(
            &[ResourceKind::Ram, ResourceKind::Output, ResourceKind::Disk].map(|k| {
                ResourceRequest::new(k, ResourceAmount::checked(64 * 1024 * 1024).unwrap())
            }),
        )
        .unwrap();
    (broker, run, reservation)
}
pub fn stored_binding(root: &Path) -> (BudgetBinding, BudgetCaps) {
    let bytes =
        std::fs::read(root.join("budget-journal/segment-00000000000000000001.bin")).unwrap();
    let segment = decode_segment(&bytes).unwrap();
    let p = segment.records()[0].payload();
    assert_eq!(p.len(), 648);
    assert_eq!(&p[..8], b"ETBUD001");
    let d = |i: usize| Digest32::from_bytes(p[i..i + 32].try_into().unwrap());
    let n = |i: usize| u64::from_le_bytes(p[i..i + 8].try_into().unwrap());
    (
        BudgetBinding {
            run_family: d(16),
            source: d(48),
            context: d(80),
            scope: d(112),
            goal: d(144),
            policy: d(176),
            root_target: d(208),
            initial_snapshot: d(240),
        },
        BudgetCaps {
            attempts: n(272),
            per_attempt_delta_bytes: n(280),
            total_delta_bytes: n(288),
        },
    )
}
pub fn limits() -> JournalBundleLimits {
    JournalBundleLimits::checked(20, 1024 * 1024).unwrap()
}
pub fn expected_head(root: &Path, view: &ResourceReservationView<'_>) -> BudgetHead {
    let observed = inspect_bundle_root(
        &root.join("budget-journal"),
        limits(),
        view,
        JournalChainLimits::default(),
    )
    .unwrap();
    BudgetHead {
        sequence: observed.cursor().sequence(),
        record_digest: observed.cursor().record_digest(),
        local_head_frame_digest: observed.head_frame_digest(),
    }
}
pub fn assert_recovered(root: &Path, steps: u64) {
    let (binding, caps) = stored_binding(root);
    let (_broker, mut run, mut reservation) = resources(binding.source);
    {
        let view = reservation.lease();
        let head = expected_head(root, &view);
        assert_eq!(head.sequence, 1 + 2 * steps);
        let mut recovered = BudgetSession::resume_existing(
            &root.join("budget-journal"),
            binding,
            caps,
            head,
            65536,
            limits(),
            JournalChainLimits::default(),
            &view,
        )
        .unwrap();
        let s = recovered.summary().unwrap();
        assert_eq!(s.phase, BudgetPhase::SettledNeedsFreshValidation);
        assert_eq!(s.charged_attempts, steps);
        assert_eq!(s.charged_delta_bytes, 1000 * steps);
        assert!(s.claim_boundary().contains("RollbackResistanceUnavailable"));
        let p = ReservationProposal {
            reservation: Digest32::from_bytes([40; 32]),
            candidate: Digest32::from_bytes([41; 32]),
            before_snapshot: binding.initial_snapshot,
            intended_after_snapshot: Digest32::from_bytes([42; 32]),
            authority_binding: Digest32::from_bytes([43; 32]),
            intent_binding: Digest32::from_bytes([44; 32]),
            delta_bytes: 1,
        };
        assert!(matches!(
            recovered.reserve_before_apply(p),
            Err(BudgetError::ReconciliationRequired)
        ));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
}

/// Current physical postimage + freshly signed same-family request are real
/// inputs, but immutable old state is still settled and cannot be reset.
pub fn assert_fresh_postimage_and_new_source_cannot_reset(root: &Path) {
    let (old, caps) = stored_binding(root);
    let (_broker, mut run, mut reservation) = resources(old.source);
    let issuer = ReceiptIssuer::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap();
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let scope = format!(
        "dgcl/repair/{}/src/lib.rs",
        dgcl_repository_root_digest(root).unwrap()
    );
    let edit = permit(&scope, &issuer, &verifier);
    let authority = ExecutionPermitContext {
        principal: edit.principal_binding(),
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
    let post = capture_dgcl_cargo_snapshot(root, SOURCE, &pipeline, &authority).unwrap();
    let request = DgclRepairRequest {
        attempt: 1,
        gap_id: pipeline.implementation_closure.gaps[0].gap_id.clone(),
        source_revision: pipeline.source_revision.clone(),
        plan_digest: pipeline.completion_plan.plan_digest.clone(),
        target_ref: "src/lib.rs".into(),
        target_digest_before: stable_sha256(AFTER),
        candidate_digest: stable_sha256(SECOND),
        authorization_scope: scope,
        authorization_span: edit.source_span(),
        repository_root_digest: Some(dgcl_repository_root_digest(root).unwrap()),
    };
    let fresh = BudgetBinding {
        initial_snapshot: parse(&post.snapshot_digest),
        ..old
    };
    let manifest = BudgetRunManifest {
        binding: fresh,
        plan: parse(&request.plan_digest),
        caps,
        requests: vec![BudgetRequestSpec {
            request_digest: parse(&dgcl_repair_request_digest(&request)),
            before_snapshot: fresh.initial_snapshot,
            intended_snapshot: Digest32::from_bytes([67; 32]),
            reserved_delta_bytes: 1000,
        }],
    };
    let receipt = issuer
        .issue(
            ReceiptClass::Validation,
            SubjectRevision::checked(&request.source_revision).unwrap(),
            ReceiptScope::checked(manifest.receipt_scope()).unwrap(),
            manifest.payload_digest().unwrap(),
            100,
            Some(400),
            None,
        )
        .unwrap();
    let checked = CheckedBudgetRun::verify(
        manifest,
        receipt,
        &verifier,
        &mut ReplayGuard::default(),
        100,
    )
    .unwrap();
    let authorization = DgclRepairAuthorization {
        permit: &edit,
        authority_revision: AuthorityRevision(1),
        trusted_host_fingerprint: verifier.key_fingerprint(),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
    };
    let mut inputs = Inputs { run: &run };
    let mut effects = Effects {
        run: &run,
        reject: false,
        calls: 0,
    };
    let mut validator = EmptyValidation;
    {
        let view = reservation.lease();
        let head = expected_head(root, &view);
        let recovered = BudgetSession::resume_existing(
            &root.join("budget-journal"),
            old,
            caps,
            head,
            65536,
            limits(),
            JournalChainLimits::default(),
            &view,
        )
        .unwrap();
        let config = DgclFileRepairConfig {
            root,
            source: SOURCE,
            pipeline: &pipeline,
            initial_snapshot: &post,
            authorization: &authorization,
            issuer: &issuer,
            verifier: &verifier,
            candidates: std::collections::BTreeMap::from([(stable_sha256(SECOND), SECOND.into())]),
            validator: &mut validator,
            epoch_clock: None,
        };
        assert!(matches!(
            BudgetedFileRepairDriver::new(config, &mut inputs, &mut effects, recovered, checked),
            Err(BudgetError::BindingMismatch)
        ));
        assert!(matches!(
            BudgetSession::initialize_new(
                &root.join("budget-journal"),
                fresh,
                caps,
                65536,
                limits(),
                JournalChainLimits::default(),
                &view
            ),
            Err(BudgetError::ExistingHistory)
        ));
    }
    assert_eq!(effects.calls, 0);
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    let source = "Please edit the source and test the package again.";
    let next_pipeline = build_dgcl_pipeline(
        source,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let changed = BudgetBinding {
        source: parse(&next_pipeline.source_revision),
        ..fresh
    };
    let next_request = DgclRepairRequest {
        source_revision: next_pipeline.source_revision.clone(),
        plan_digest: next_pipeline.completion_plan.plan_digest.clone(),
        gap_id: next_pipeline.implementation_closure.gaps[0].gap_id.clone(),
        authorization_span: SourceSpan {
            start: 0,
            end: source.len(),
        },
        ..request
    };
    let manifest = BudgetRunManifest {
        binding: changed,
        plan: parse(&next_pipeline.completion_plan.plan_digest),
        caps,
        requests: vec![BudgetRequestSpec {
            request_digest: parse(&dgcl_repair_request_digest(&next_request)),
            before_snapshot: changed.initial_snapshot,
            intended_snapshot: Digest32::from_bytes([68; 32]),
            reserved_delta_bytes: 1000,
        }],
    };
    let receipt = issuer
        .issue(
            ReceiptClass::Validation,
            SubjectRevision::checked(&next_pipeline.source_revision).unwrap(),
            ReceiptScope::checked(manifest.receipt_scope()).unwrap(),
            manifest.payload_digest().unwrap(),
            100,
            Some(400),
            None,
        )
        .unwrap();
    let _checked = CheckedBudgetRun::verify(
        manifest,
        receipt,
        &verifier,
        &mut ReplayGuard::default(),
        100,
    )
    .unwrap();
    let (_broker, mut run, mut reservation) = resources(changed.source);
    {
        let view = reservation.lease();
        let head = expected_head(root, &view);
        assert!(matches!(
            BudgetSession::resume_existing(
                &root.join("budget-journal"),
                changed,
                caps,
                head,
                65536,
                limits(),
                JournalChainLimits::default(),
                &view
            ),
            Err(BudgetError::BindingMismatch)
        ));
    }
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        AFTER
    );
}
