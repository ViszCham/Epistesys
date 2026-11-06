//! 署名bindingと実file driverの接続。未実行を成功扱いしない。
//! Signed-binding to actual-file connection; unexecuted tests are not passes.
#[path = "tlcx32_verified_support/mod.rs"]
mod fixture;
use fixture::{parse, permit, root, Effects, EmptyValidation, Inputs, SOURCE};
use lc631_analysis::*;
use lc631_core::*;
use lc631_host::JournalBundleLimits;
use lc631_receipt_kernel::*;
use lc631_runtime::contextual_repair::{execution::*, measurement::*};
use lc631_runtime::{contextual_resume::budget::*, journal::JournalChainLimits, resource::*};
use lc631_substrate::Digest32;
use std::time::Duration;
#[path = "../../lc631-analysis/tests/tlcx31_file_repair_fixture/authority.rs"]
#[allow(dead_code)]
mod probe_authority;
const BEFORE: &str = r#"pub fn add(a:u32,b:u32)->Option<u32>{None}
fn main(){use std::io::Read;let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();for line in s.lines(){let v:Vec<u32>=line.split_whitespace().map(|x|x.parse().unwrap()).collect();match add(v[0],v[1]){Some(n)=>println!("some:{n}"),None=>println!("none")}}}
"#;
const AFTER: &str = r#"pub fn add(a:u32,b:u32)->Option<u32>{a.checked_add(b)}
fn main(){use std::io::Read;let mut s=String::new();std::io::stdin().read_to_string(&mut s).unwrap();for line in s.lines(){let v:Vec<u32>=line.split_whitespace().map(|x|x.parse().unwrap()).collect();match add(v[0],v[1]){Some(n)=>println!("some:{n}"),None=>println!("none")}}}
"#;
fn clock() -> u64 {
    100
}
fn compile_native(source: &std::path::Path, out: &std::path::Path) {
    // Test harness compilation only, not an unpermitted production fallback.
    let status = std::process::Command::new("rustc")
        .args(["--edition=2021", "-Awarnings", "-o"])
        .arg(out)
        .arg(source)
        .status()
        .unwrap();
    assert!(status.success());
}
fn observe<'v>(
    plan: &CodingProbePlan,
    snapshot: &DgclExecutionSnapshot,
    target: &std::path::Path,
    program: &std::path::Path,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    resources: &'v ResourceReservationView<'_>,
) -> OwnedCodingProbeObservation<'v> {
    let executable_digest = probe_authority::file_hash(program);
    let registration = ProbeToolRegistration {
        program: program.to_str().unwrap(),
        executable_digest: &executable_digest,
        target_path: target,
        target_ref: "src/lib.rs",
        snapshot,
    };
    let receipt = issuer
        .issue(
            ReceiptClass::ToolExecution,
            SubjectRevision::checked(&snapshot.source_revision).unwrap(),
            ReceiptScope::checked(registration.receipt_scope(plan)).unwrap(),
            registration.payload_digest(plan),
            100,
            Some(400),
            None,
        )
        .unwrap();
    let scope = registration.execution_scope(plan);
    let permission = probe_authority::permit(SOURCE, &scope, issuer, verifier, Action::Test);
    let authority = ExecutionPermitContext {
        principal: permission.principal_binding(),
        action: Action::Test,
        scope: &scope,
        source_revision: &snapshot.source_revision,
        authority_revision: AuthorityRevision(1),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: verifier.key_fingerprint(),
    };
    let lease = issue_dgcl_evidence_lease(snapshot, 100, 300).unwrap();
    observe_current(
        plan,
        ProbeInvocation {
            source: SOURCE,
            registration,
            lease: &lease,
            tool_receipt: &receipt,
            tool_verifier: verifier,
            permit: &permission,
            authority: &authority,
            clock,
            timeout_ms: 5000,
        },
        resources,
    )
    .unwrap()
}

fn exercise(reject_effect: bool) -> (BudgetSummary, String, usize, DgclRepairLoopReport) {
    let (summary, actual, calls, report, _) = exercise_with_target_swap(reject_effect, false);
    (summary, actual, calls, report)
}

fn exercise_with_target_swap(
    reject_effect: bool,
    swap_after_observation: bool,
) -> (
    BudgetSummary,
    String,
    usize,
    DgclRepairLoopReport,
    Result<Option<usize>, ProbeError>,
) {
    let root = root();
    std::fs::write(root.join("src/lib.rs"), BEFORE).unwrap();
    std::fs::write(root.join("baseline.rs"), BEFORE).unwrap();
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
        dgcl_repository_root_digest(&root).unwrap()
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
    let initial = capture_dgcl_cargo_snapshot(&root, SOURCE, &pipeline, &auth).unwrap();
    // Test-owned temporary staging observes the exact intended snapshot, then
    // restores preimage. This is not a production prediction or mutation permit.
    std::fs::write(root.join("src/lib.rs"), AFTER).unwrap();
    let intended = capture_dgcl_cargo_snapshot(&root, SOURCE, &pipeline, &auth).unwrap();
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
        repository_root_digest: Some(dgcl_repository_root_digest(&root).unwrap()),
    };
    let caps = BudgetCaps {
        attempts: 1,
        per_attempt_delta_bytes: 1000,
        total_delta_bytes: 1000,
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
        requests: vec![BudgetRequestSpec {
            request_digest: parse(&dgcl_repair_request_digest(&request)),
            before_snapshot: binding.initial_snapshot,
            intended_snapshot: parse(&intended.snapshot_digest),
            reserved_delta_bytes: 1000,
        }],
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
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Disk,
                ResourceAmount::checked(64 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(ResourceKind::Child, ResourceAmount::checked(2).unwrap()),
            ResourceRequest::new(ResourceKind::Cleanup, ResourceAmount::checked(2).unwrap()),
        ])
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
        reject: reject_effect,
        calls: 0,
    };
    let mut validator = EmptyValidation;
    let plan = CodingProbePlan::checked_add(
        Digest32::from_bytes([34; 32]),
        Digest32::from_bytes([35; 32]),
    )
    .unwrap();
    let summary;
    let report;
    let recommendation;
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
            root: &root,
            source: SOURCE,
            pipeline: &pipeline,
            initial_snapshot: &initial,
            authorization: &authorization,
            issuer: &issuer,
            verifier: &verifier,
            candidates: std::collections::BTreeMap::from([(stable_sha256(AFTER), AFTER.into())]),
            validator: &mut validator,
            epoch_clock: None,
        };
        let inner =
            BudgetedFileRepairDriver::new(config, &mut inputs, &mut effects, budget, checked)
                .unwrap();
        compile_native(
            &root.join("baseline.rs"),
            &root.join(format!("baseline{}", std::env::consts::EXE_SUFFIX)),
        );
        let baseline = observe(
            &plan,
            &initial,
            &root.join("baseline.rs"),
            &root.join(format!("baseline{}", std::env::consts::EXE_SUFFIX)),
            &issuer,
            &verifier,
            &view,
        );
        let mut callback = |token: &DgclVerifiedRepairValidation<'_>| {
            let program = root.join(format!("current{}", std::env::consts::EXE_SUFFIX));
            compile_native(&root.join("src/lib.rs"), &program);
            let observed = observe(
                &plan,
                &token.application().after_snapshot,
                &root.join("src/lib.rs"),
                &program,
                &issuer,
                &verifier,
                &view,
            );
            if swap_after_observation {
                // This is an actual successful owned observation, followed by a
                // same-length mutation of this fixture's own physical target.
                // check_current is deliberately not a continuous path watcher.
                let changed = AFTER.replace("checked_add", "checked_sub");
                assert_ne!(changed, AFTER);
                assert_eq!(changed.len(), AFTER.len());
                std::fs::write(root.join("src/lib.rs"), changed).unwrap();
            }
            Ok(observed)
        };
        let mut driver =
            RemeasuringRepairDriver::new(inner, &plan, &baseline, &view, &mut callback).unwrap();
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
                requests: &[request],
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
                max_attempts: 1,
                max_wall_time: Duration::from_secs(20),
                max_delta_bytes: 1000,
            },
            &mut driver,
        )
        .unwrap();
        if !reject_effect && !swap_after_observation {
            assert_eq!(driver.gains().len(), 1);
            assert_eq!(driver.gains()[0].gain_quarters(), 3);
            assert_eq!(driver.recommendation_current().unwrap(), Some(0));
        } else if reject_effect {
            assert!(driver.gains().is_empty());
        }
        recommendation = driver.recommendation_current();
        summary = driver.budget_summary().unwrap();
    }
    let calls = effects.calls;
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    (
        summary,
        std::fs::read_to_string(root.join("src/lib.rs")).unwrap(),
        calls,
        report,
        recommendation,
    )
}

#[test]
fn tlcx31_live_effect_cut_retains_budget_without_measurement() {
    let (s, actual, calls, r) = exercise(true);
    assert_eq!(calls, 1);
    assert_eq!(actual, BEFORE);
    assert_eq!(s.phase, BudgetPhase::PendingMayHaveApplied);
    assert_eq!(s.charged_attempts, 1);
    assert_eq!(s.charged_delta_bytes, 1000);
    assert_ne!(r.status, DgclRepairStatus::Completed);
}
#[test]
fn tlcx31_actual_repair_revalidation_native_measurement_changes_selection() {
    let (s, actual, calls, r) = exercise(false);
    assert_eq!(calls, 1);
    assert_eq!(actual, AFTER);
    assert_eq!(s.phase, BudgetPhase::QuotaReadyRequiresFreshAuthority);
    assert_eq!(s.charged_attempts, 1);
    assert_eq!(s.charged_delta_bytes, 1000);
    assert!(!r.authority_created);
    assert!(r.attempts[0].application_receipt_digest.is_some());
}

/// Independent review's source-supported gap. Central must execute this
/// behavioral negative before any production repair is authorized. Existing
/// EmptyValidation leaves coding gaps open, so the separate measured selection
/// assertion is essential: a Hold alone does not establish physical freshness.
#[test]
fn tlcx31_post_observation_same_length_target_swap_withholds_fresh_recommendation() {
    let (summary, actual, calls, report, recommendation) = exercise_with_target_swap(false, true);
    assert_eq!(calls, 1);
    assert_eq!(actual, AFTER.replace("checked_add", "checked_sub"));
    assert_eq!(actual.len(), AFTER.len());
    assert_eq!(summary.charged_attempts, 1);
    assert_eq!(summary.charged_delta_bytes, 1000);
    assert!(report.attempts[0].application_receipt_digest.is_some());
    assert!(report.attempts[0].validation_receipt_digest.is_some());
    assert_ne!(report.status, DgclRepairStatus::Completed);
    assert!(
        !matches!(&recommendation, Ok(Some(_))),
        "the physical target changed after genuine native observation; a stale owner must not yield a fresh measured recommendation: {recommendation:?}"
    );
}
