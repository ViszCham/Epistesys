//! 実2段階とprocess checkpoint用の有限fixture。proof/Completedを作らない。
//! Bounded actual two-step/checkpoint fixture, never compiler proof or Completed.
#![allow(dead_code)]
#[path = "../tlcx32_verified_support/mod.rs"]
mod file_fixture;
pub use file_fixture::*;
use lc631_analysis::*;
use lc631_core::*;
use lc631_host::JournalBundleLimits;
use lc631_receipt_kernel::*;
use lc631_runtime::{contextual_resume::budget::*, journal::JournalChainLimits, resource::*};
use lc631_substrate::Digest32;
use std::time::Duration;
pub const SECOND: &str = "pub fn value() -> u8 { 2 }\n";
pub mod recovery;

pub fn exercise(
    root: &std::path::Path,
    steps: usize,
    exit_after_v2: bool,
) -> (BudgetSummary, String, usize, DgclRepairLoopReport) {
    assert!((1..=2).contains(&steps));
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap();
    let scope = format!(
        "dgcl/repair/{}/src/lib.rs",
        dgcl_repository_root_digest(root).unwrap()
    );
    let edit = permit(&scope, &issuer, &verifier);
    let auth = ExecutionPermitContext {
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
    let initial = capture_dgcl_cargo_snapshot(root, SOURCE, &pipeline, &auth).unwrap();
    // Test-owned temporary staging observes the exact intended snapshot, then
    // restores preimage. This is not a production prediction or mutation permit.
    std::fs::write(root.join("src/lib.rs"), AFTER).unwrap();
    let intended = capture_dgcl_cargo_snapshot(root, SOURCE, &pipeline, &auth).unwrap();
    std::fs::write(root.join("src/lib.rs"), SECOND).unwrap();
    let final_snapshot = capture_dgcl_cargo_snapshot(root, SOURCE, &pipeline, &auth).unwrap();
    std::fs::write(root.join("src/lib.rs"), BEFORE).unwrap();
    let request = DgclRepairRequest {
        attempt: 1,
        gap_id: pipeline.implementation_closure.gaps[0].gap_id.clone(),
        source_revision: pipeline.source_revision.clone(),
        plan_digest: pipeline.completion_plan.plan_digest.clone(),
        target_ref: "src/lib.rs".into(),
        target_digest_before: stable_sha256(BEFORE),
        candidate_digest: stable_sha256(AFTER),
        authorization_scope: scope,
        authorization_span: edit.source_span(),
        repository_root_digest: Some(dgcl_repository_root_digest(root).unwrap()),
    };
    let second = DgclRepairRequest {
        attempt: 2,
        target_digest_before: stable_sha256(AFTER),
        candidate_digest: stable_sha256(SECOND),
        ..request.clone()
    };
    let requests = vec![request.clone(), second.clone()]
        .into_iter()
        .take(steps)
        .collect::<Vec<_>>();
    let caps = BudgetCaps {
        attempts: 2,
        per_attempt_delta_bytes: 1000,
        total_delta_bytes: 2000,
    };
    let binding = BudgetBinding {
        run_family: Digest32::from_bytes([31; 32]),
        source: parse(&pipeline.source_revision),
        context: Digest32::from_bytes([32; 32]),
        scope: Digest32::from_bytes([33; 32]),
        goal: Digest32::from_bytes([34; 32]),
        policy: Digest32::from_bytes([35; 32]),
        root_target: parse(request.repository_root_digest.as_deref().unwrap()),
        initial_snapshot: parse(&initial.snapshot_digest),
    };
    let manifest = BudgetRunManifest {
        binding,
        plan: parse(&request.plan_digest),
        caps,
        requests: vec![
            BudgetRequestSpec {
                request_digest: parse(&dgcl_repair_request_digest(&request)),
                before_snapshot: binding.initial_snapshot,
                intended_snapshot: parse(&intended.snapshot_digest),
                reserved_delta_bytes: 1000,
            },
            BudgetRequestSpec {
                request_digest: parse(&dgcl_repair_request_digest(&second)),
                before_snapshot: parse(&intended.snapshot_digest),
                intended_snapshot: parse(&final_snapshot.snapshot_digest),
                reserved_delta_bytes: 1000,
            },
        ]
        .into_iter()
        .take(steps)
        .collect(),
    };
    let raw = issuer
        .issue(
            ReceiptClass::Validation,
            SubjectRevision::checked(&pipeline.source_revision).unwrap(),
            ReceiptScope::checked(manifest.receipt_scope()).unwrap(),
            manifest.payload_digest().unwrap(),
            100,
            Some(400),
            None,
        )
        .unwrap();
    let checked =
        CheckedBudgetRun::verify(manifest, raw, &verifier, &mut ReplayGuard::default(), 100)
            .unwrap();
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker
        .begin_run(
            1,
            binding.source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let mut reservation = run
        .reserve(
            &[ResourceKind::Ram, ResourceKind::Output, ResourceKind::Disk].map(|kind| {
                ResourceRequest::new(kind, ResourceAmount::checked(64 * 1024 * 1024).unwrap())
            }),
        )
        .unwrap();
    let lease = issue_dgcl_evidence_lease(&initial, 100, 300).unwrap();
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
    let summary;
    let report;
    {
        let view = reservation.lease();
        let budget = BudgetSession::initialize_new(
            &root.join("budget-journal"),
            binding,
            caps,
            65536,
            JournalBundleLimits::checked(20, 1024 * 1024).unwrap(),
            JournalChainLimits::default(),
            &view,
        )
        .unwrap();
        let config = DgclFileRepairConfig {
            root,
            source: SOURCE,
            pipeline: &pipeline,
            initial_snapshot: &initial,
            authorization: &authorization,
            issuer: &issuer,
            verifier: &verifier,
            candidates: std::collections::BTreeMap::from([
                (stable_sha256(AFTER), AFTER.into()),
                (stable_sha256(SECOND), SECOND.into()),
            ]),
            validator: &mut validator,
            epoch_clock: None,
        };
        let mut driver =
            BudgetedFileRepairDriver::new(config, &mut inputs, &mut effects, budget, checked)
                .unwrap();
        let gaps = pipeline
            .implementation_closure
            .gaps
            .iter()
            .filter(|g| {
                !matches!(
                    g.state,
                    ImplementationGapState::VerifiedReceipt | ImplementationGapState::NotApplicable
                )
            })
            .map(|g| g.gap_id.clone())
            .collect::<Vec<_>>();
        report = run_dgcl_repair_loop(
            DgclRepairLoopInput {
                source: SOURCE,
                source_revision: &pipeline.source_revision,
                plan_digest: &pipeline.completion_plan.plan_digest,
                program_ir: &pipeline.program_ir,
                translation: &pipeline.translation_projection,
                completion_plan: &pipeline.completion_plan,
                initial_snapshot: &initial,
                initial_lease: &lease,
                initial_gap_ids: &gaps,
                requests: &requests,
                authorization: DgclRepairAuthorization {
                    permit: &edit,
                    authority_revision: AuthorityRevision(1),
                    trusted_host_fingerprint: verifier.key_fingerprint(),
                    now_epoch: 100,
                    revocation_revision: 0,
                    revoked_permit_digests: &[],
                },
                verifier: &verifier,
                replay: &mut ReplayGuard::default(),
                max_attempts: 2,
                max_wall_time: Duration::from_secs(20),
                max_delta_bytes: 1000,
            },
            &mut driver,
        )
        .unwrap();
        summary = driver.budget_summary().unwrap();
        if exit_after_v2 {
            assert_eq!(steps, 1);
            assert_eq!(summary.phase, BudgetPhase::QuotaReadyRequiresFreshAuthority);
            assert_eq!(report.attempts.len(), 1);
            assert!(report.attempts[0].application_receipt_digest.is_some());
            assert_eq!(
                std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
                AFTER
            );
            // Actual v2 handoff completed; bypass Rust Drop while writer lock is live.
            std::process::exit(86);
        }
    }
    let calls = effects.calls;
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    (
        summary,
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        calls,
        report,
    )
}
