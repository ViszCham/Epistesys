//! 実file repairと再開SDKのfixture接続。保存するのは履歴で、観測ownerではない。
//! Actual file-repair/resume SDK fixture; persists history, never observation owners.
use super::{coding_support as coding, file_fixture as file};
use lc631_analysis::*;
use lc631_core::*;
use lc631_host::JournalBundleLimits;
use lc631_receipt_kernel::*;
use lc631_runtime::{
    contextual_repair::{execution::*, measurement::*, snapshot_gate::*},
    contextual_resume::{budget::*, fresh_reselection::*, transition::*},
    journal::*, resource::*,
};
use lc631_substrate::Digest32;
use std::{io::{Read, Write}, path::Path, time::Duration};

fn clock100() -> u64 { 100 }
fn clock200() -> u64 { 200 }
fn signing() -> (ReceiptIssuer, ReceiptVerifier) {
    // Synthetic trusted test provider; no real host ingress/human independence.
    (ReceiptIssuer::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap(),
     ReceiptVerifier::from_key_bytes("tlcx32-tool-fixture", &[81; 32]).unwrap())
}
fn plan() -> CodingProbePlan {
    CodingProbePlan::checked_add(Digest32::from_bytes([34; 32]), Digest32::from_bytes([35; 32])).unwrap()
}
fn native(root: &Path, name: &str) -> std::path::PathBuf {
    root.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
fn q(kind: ResourceKind, amount: u64) -> ResourceRequest {
    ResourceRequest::new(kind, ResourceAmount::checked(amount).unwrap())
}
fn resources() -> (ResourceBroker, RunResources, Reservation) {
    // Two bundle scratches, replay work/inventory, capped segment/index/head
    // reads and budget work conservatively total 31_277_261 Output bytes.
    // Keep this fixture reservation within the existing 128 MiB broker cap.
    const OUTPUT_BYTES: u64 = 32 * 1024 * 1024;
    let limits = ResourceLimits::uniform(128 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker.begin_run(32, context::SourceRevision::of(coding::SOURCE).digest(), limits,
        Deadline::after(Duration::from_secs(90)).unwrap(), Cancellation::default()).unwrap();
    let reservation = run.reserve(&[
        q(ResourceKind::Ram, 64 * 1024 * 1024), q(ResourceKind::Output, OUTPUT_BYTES),
        q(ResourceKind::Disk, 64 * 1024 * 1024), q(ResourceKind::Child, 2), q(ResourceKind::Cleanup, 2),
    ]).unwrap();
    (broker, run, reservation)
}
pub fn read_owned(path: &Path, limit: usize) -> Vec<u8> {
    let file = std::fs::File::open(path).unwrap();
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= limit); bytes
}
fn write_new(path: &Path, bytes: &[u8], limit: usize) {
    assert!(bytes.len() <= limit);
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).unwrap();
    file.write_all(bytes).unwrap(); file.sync_all().unwrap();
}
#[derive(Clone, Copy, PartialEq)]
enum ResumeCase { Plain, Revoked, Physical, Cargo, OtherSource, LateAuthority }

pub fn create_checkpoint(root: &Path) {
    assert_eq!(file::SOURCE, coding::SOURCE);
    std::fs::write(root.join("src/lib.rs"), coding::BEFORE).unwrap();
    write_new(&root.join("baseline.rs"), coding::BEFORE.as_bytes(), 16_384);
    write_new(&root.join("src/extra.rs"), b"pub const EXTRA: u8 = 1;\n", 4096);
    let pipeline = build_dgcl_pipeline(coding::SOURCE, DgclBackendSet { english: None, japanese: None }).unwrap();
    let (issuer, verifier) = signing();
    let edit_scope = format!("dgcl/repair/{}/src/lib.rs", dgcl_repository_root_digest(root).unwrap());
    let edit = file::permit(&edit_scope, &issuer, &verifier);
    let authority = ExecutionPermitContext { principal: edit.principal_binding(), action: Action::Edit,
        scope: &edit_scope, source_revision: &pipeline.source_revision, authority_revision: AuthorityRevision(1),
        now_epoch: 100, revocation_revision: 0, revoked_permit_digests: &[],
        caller_origin: CallerOrigin::HostVerifiedUser, trusted_host_fingerprint: verifier.key_fingerprint() };
    let initial = capture_dgcl_cargo_snapshot(root, coding::SOURCE, &pipeline, &authority).unwrap();
    // Test-only staging computes the exact intended snapshot, then restores the
    // preimage before the actual owned file driver executes its authorized edit.
    std::fs::write(root.join("src/lib.rs"), coding::AFTER).unwrap();
    let intended = capture_dgcl_cargo_snapshot(root, coding::SOURCE, &pipeline, &authority).unwrap();
    std::fs::write(root.join("src/lib.rs"), coding::BEFORE).unwrap();
    let request = DgclRepairRequest { attempt: 1, gap_id: pipeline.implementation_closure.gaps[0].gap_id.clone(),
        source_revision: pipeline.source_revision.clone(), plan_digest: pipeline.completion_plan.plan_digest.clone(),
        target_ref: "src/lib.rs".into(), target_digest_before: stable_sha256(coding::BEFORE),
        candidate_digest: stable_sha256(coding::AFTER), authorization_scope: edit_scope.clone(),
        authorization_span: edit.source_span(), repository_root_digest: Some(dgcl_repository_root_digest(root).unwrap()) };
    let caps = BudgetCaps { attempts: 1, per_attempt_delta_bytes: 1000, total_delta_bytes: 1000 };
    let binding = BudgetBinding { run_family: Digest32::from_bytes([31; 32]),
        source: file::parse(&pipeline.source_revision), context: context::SourceRevision::of(coding::SOURCE).digest(),
        scope: Digest32::from_bytes([33; 32]), goal: Digest32::from_bytes([34; 32]),
        policy: Digest32::from_bytes([35; 32]), root_target: file::parse(request.repository_root_digest.as_ref().unwrap()),
        initial_snapshot: file::parse(&initial.snapshot_digest) };
    let manifest = BudgetRunManifest { binding, plan: file::parse(&request.plan_digest), caps,
        requests: vec![BudgetRequestSpec { request_digest: file::parse(&dgcl_repair_request_digest(&request)),
            before_snapshot: binding.initial_snapshot, intended_snapshot: file::parse(&intended.snapshot_digest),
            reserved_delta_bytes: 1000 }] };
    let receipt = issuer.issue(ReceiptClass::Validation, SubjectRevision::checked(&pipeline.source_revision).unwrap(),
        ReceiptScope::checked(manifest.receipt_scope()).unwrap(), manifest.payload_digest().unwrap(), 100, Some(400), None).unwrap();
    let run_binding = CheckedBudgetRun::verify(manifest, receipt, &verifier, &mut ReplayGuard::default(), 100).unwrap();
    let (_broker, mut run, mut reservation) = resources();
    let initial_lease = issue_dgcl_evidence_lease(&initial, 100, 300).unwrap();
    let authorization = DgclRepairAuthorization { permit: &edit, authority_revision: AuthorityRevision(1),
        trusted_host_fingerprint: verifier.key_fingerprint(), now_epoch: 100,
        revocation_revision: 0, revoked_permit_digests: &[] };
    let mut inputs = file::Inputs { run: &run };
    let mut effects = file::Effects { run: &run, reject: false, calls: 0 };
    let mut validator = file::EmptyValidation;
    let plan = plan();
    let mut captured = None;
    {
        let view = reservation.lease();
        let budget = BudgetSession::initialize_new(&root.join("budget-journal"), binding, caps, 65_536,
            JournalBundleLimits::checked(20, 1024 * 1024).unwrap(), JournalChainLimits::default(), &view).unwrap();
        let config = DgclFileRepairConfig { root, source: coding::SOURCE, pipeline: &pipeline,
            initial_snapshot: &initial, authorization: &authorization, issuer: &issuer, verifier: &verifier,
            candidates: std::collections::BTreeMap::from([(stable_sha256(coding::AFTER), coding::AFTER.into())]),
            validator: &mut validator, epoch_clock: Some(clock100) };
        let inner = BudgetedFileRepairDriver::new(config, &mut inputs, &mut effects, budget, run_binding).unwrap();
        coding::compile_native(&root.join("baseline.rs"), &native(root, "baseline"));
        let baseline = coding::observe(&plan, &initial, &root.join("baseline.rs"), &native(root, "baseline"),
            &issuer, &verifier, &view);
        assert_eq!(baseline.correct_probes(), 1);
        let mut application_observer = |token: &DgclVerifiedRepairApplication<'_>| {
            let pins = TransitionPins { context: context::ContextRevision::original(context::SourceRevision::of(coding::SOURCE)),
                comparison_scope: binding.scope, probe_plan: plan.digest(), goal: plan.goal(), policy: plan.policy() };
            let checkpoint = UntrustedTransitionCheckpoint::capture_with_baseline(token, pins, &baseline, &plan, &view)
                .map_err(|_| ProbeError::BindingMismatch)?;
            captured = Some((checkpoint, token.request().clone(), token.application().clone(), token.checked_epoch()));
            Ok(())
        };
        let mut accepted_validation = |token: &DgclVerifiedRepairValidation<'_>| {
            coding::compile_native(&root.join("src/lib.rs"), &native(root, "current"));
            Ok(coding::observe(&plan, &token.application().after_snapshot, &root.join("src/lib.rs"),
                &native(root, "current"), &issuer, &verifier, &view))
        };
        let mut driver = RemeasuringRepairDriver::new(inner, &plan, &baseline, &view, &mut accepted_validation)
            .unwrap().with_application_observer(&mut application_observer);
        let gaps = pipeline.implementation_closure.gaps.iter().filter(|g| !matches!(g.state,
            ImplementationGapState::VerifiedReceipt | ImplementationGapState::NotApplicable))
            .map(|g| g.gap_id.clone()).collect::<Vec<_>>();
        let report = run_dgcl_repair_loop(DgclRepairLoopInput { source: coding::SOURCE,
            source_revision: &pipeline.source_revision, plan_digest: &pipeline.completion_plan.plan_digest,
            program_ir: &pipeline.program_ir, translation: &pipeline.translation_projection,
            completion_plan: &pipeline.completion_plan, initial_snapshot: &initial, initial_lease: &initial_lease,
            initial_gap_ids: &gaps, requests: &[request],
            authorization: DgclRepairAuthorization { permit: &edit, authority_revision: AuthorityRevision(1),
                trusted_host_fingerprint: verifier.key_fingerprint(), now_epoch: 100,
                revocation_revision: 0, revoked_permit_digests: &[] },
            verifier: &verifier, replay: &mut ReplayGuard::default(), max_attempts: 1,
            max_wall_time: Duration::from_secs(30), max_delta_bytes: 1000 }, &mut driver).unwrap();
        assert_eq!(driver.gains().len(), 1);
        assert_eq!(driver.gains()[0].gain_quarters(), 3);
        assert_eq!(driver.recommendation_current().unwrap(), Some(0));
        assert_ne!(report.status, DgclRepairStatus::Completed);
        assert!(!report.authority_created);
        assert_eq!(driver.budget_summary().unwrap().charged_attempts, 1);
    }
    assert_eq!(effects.calls, 1);
    let (checkpoint, request, application, checked_epoch) = captured.unwrap();
    assert_eq!(checkpoint.as_bytes().len(), BOUND_TRANSITION_BYTES);
    let attestation = issuer.issue(ReceiptClass::ToolExecution,
        SubjectRevision::checked(&request.source_revision).unwrap(),
        ReceiptScope::checked(checkpoint.receipt_scope()).unwrap(), lc631_runtime::contextual_resume::transition::digest_text(checkpoint.payload_digest()),
        checked_epoch, Some(400), Some(application.attestation.as_ref().unwrap().receipt_digest())).unwrap();
    let mut builder = JournalSegmentBuilder::genesis();
    builder.append_at(1, JournalRecordKind::Evidence, checkpoint.payload_digest(), checkpoint.as_bytes()).unwrap();
    write_new(&root.join("transition.bin"), &builder.finish().unwrap(), 4096);
    let history = serde_json::to_vec(&serde_json::json!({"request":request,"application":application,
        "transition_attestation":attestation,"baseline_snapshot":initial})).unwrap();
    write_new(&root.join("history.json"), &history, 65_536);
    reservation.release_confirmed().unwrap(); run.close_confirmed().unwrap();
}

fn application(value: &serde_json::Value) -> DgclRepairApplication {
    let text = |name: &str| value[name].as_str().unwrap().to_owned();
    DgclRepairApplication { schema_version: "epistesys-dgcl-repair-application.v1",
        attempt: u8::try_from(value["attempt"].as_u64().unwrap()).unwrap(), gap_id: text("gap_id"),
        request_digest: text("request_digest"), source_revision: text("source_revision"), plan_digest: text("plan_digest"),
        target_ref: text("target_ref"), target_digest_before: text("target_digest_before"), target_digest_after: text("target_digest_after"),
        candidate_digest: text("candidate_digest"), delta_bytes: value["delta_bytes"].as_u64().unwrap(),
        run_digest: text("run_digest"), permit_digest: text("permit_digest"), applied: value["applied"].as_bool().unwrap(),
        after_snapshot: serde_json::from_value(value["after_snapshot"].clone()).unwrap(),
        after_lease: serde_json::from_value(value["after_lease"].clone()).unwrap(),
        attestation: serde_json::from_value(value["attestation"].clone()).unwrap() }
}

pub fn resume_checkpoint(root: &Path, cut: bool) {
    resume(root, if cut { ResumeCase::Revoked } else { ResumeCase::Plain })
}
pub fn resume_checkpoint_with_physical_cut(root: &Path) {
    resume(root, ResumeCase::Physical)
}
pub fn resume_checkpoint_with_build_cut(root: &Path, cargo: bool) {
    resume(root, if cargo { ResumeCase::Cargo } else { ResumeCase::OtherSource })
}
pub fn resume_checkpoint_with_late_authority_cut(root: &Path) {
    resume(root, ResumeCase::LateAuthority)
}
fn resume(root: &Path, case: ResumeCase) {
    let wire: serde_json::Value = serde_json::from_slice(&read_owned(&root.join("history.json"), 65_536)).unwrap();
    let request: DgclRepairRequest = serde_json::from_value(wire["request"].clone()).unwrap();
    let application = application(&wire["application"]);
    let attestation: UntrustedReceipt = serde_json::from_value(wire["transition_attestation"].clone()).unwrap();
    let before: DgclExecutionSnapshot = serde_json::from_value(wire["baseline_snapshot"].clone()).unwrap();
    let (issuer, verifier) = signing();
    let plan = plan();
    let (_broker, mut run, mut reservation) = resources();
    {
        let view = reservation.lease();
        let bytes = read_owned(&root.join("transition.bin"), 4096);
        let history = inspect_transition_chain(&[&bytes], None, JournalChainLimits::checked(1, bytes.len()).unwrap(),
            TransitionEvidence { request: &request, application: &application, transition_attestation: &attestation,
                verifier: &verifier, now_epoch: 200 }, &view).unwrap();
        assert_eq!(history.before_build(), Some(file::parse(&before.build_input_digest)));
        let store_limits = JournalBundleLimits::checked(20, 1024 * 1024).unwrap();
        let journal_root = root.join("budget-journal");
        let observed = inspect_bundle_root(&journal_root, store_limits, &view, JournalChainLimits::default()).unwrap();
        let binding = BudgetBinding { run_family: Digest32::from_bytes([31; 32]),
            source: file::parse(&request.source_revision), context: context::SourceRevision::of(coding::SOURCE).digest(),
            scope: Digest32::from_bytes([33; 32]), goal: plan.goal(), policy: plan.policy(),
            root_target: file::parse(request.repository_root_digest.as_ref().unwrap()),
            initial_snapshot: file::parse(&before.snapshot_digest) };
        let recovered = BudgetSession::resume_existing(&journal_root, binding,
            BudgetCaps { attempts: 1, per_attempt_delta_bytes: 1000, total_delta_bytes: 1000 },
            BudgetHead { sequence: observed.cursor().sequence(), record_digest: observed.cursor().record_digest(),
                local_head_frame_digest: observed.head_frame_digest() }, 65_536, store_limits,
            JournalChainLimits::default(), &view).unwrap();
        assert_eq!(recovered.summary().unwrap().phase, BudgetPhase::SettledNeedsFreshValidation);
        assert_eq!(recovered.summary().unwrap().charged_attempts, 1);
        drop(recovered);
        let baseline = coding::NativeProbeFixture::registered_at(&plan, before, root.join("baseline.rs"),
            native(root, "baseline"), &issuer, &verifier, 200, clock200);
        let pipeline = build_dgcl_pipeline(coding::SOURCE, DgclBackendSet { english: None, japanese: None }).unwrap();
        // Independent current filesystem capture; the historical after-snapshot
        // is not accepted as the current observation or execution permit.
        let scope = request.authorization_scope.clone();
        let edit = file::permit(&scope, &issuer, &verifier);
        let authority = ExecutionPermitContext { principal: edit.principal_binding(), action: Action::Edit,
            scope: &scope, source_revision: &pipeline.source_revision, authority_revision: AuthorityRevision(1),
            now_epoch: 200, revocation_revision: 0, revoked_permit_digests: &[],
            caller_origin: CallerOrigin::HostVerifiedUser, trusted_host_fingerprint: verifier.key_fingerprint() };
        let current_snapshot = capture_dgcl_cargo_snapshot(root, coding::SOURCE, &pipeline, &authority).unwrap();
        assert_eq!(current_snapshot.build_input_digest, application.after_snapshot.build_input_digest);
        let current = coding::NativeProbeFixture::registered_at(&plan, current_snapshot,
            root.join("src/lib.rs"), native(root, "current"), &issuer, &verifier, 200, clock200);
        baseline.with_invocation(&verifier, |baseline| current.with_invocation(&verifier, |current| {
            let revoked = vec![current.permit.digest().to_owned()];
            let cut_authority = ExecutionPermitContext { principal: current.authority.principal,
                action: current.authority.action, scope: current.authority.scope,
                source_revision: current.authority.source_revision, authority_revision: current.authority.authority_revision,
                now_epoch: current.authority.now_epoch, revocation_revision: current.authority.revocation_revision,
                revoked_permit_digests: &revoked, caller_origin: current.authority.caller_origin,
                trusted_host_fingerprint: current.authority.trusted_host_fingerprint };
            let current = if case == ResumeCase::Revoked { ProbeInvocation { authority: &cut_authority, ..current } } else { current };
            let outcome = resume_reselect_coding(FreshCodingResumeRequest { history,
                context: context::ContextRevision::original(context::SourceRevision::of(coding::SOURCE)),
                comparison_scope: Digest32::from_bytes([33; 32]), plan: &plan, baseline, current }, &view);
            if case == ResumeCase::Revoked {
                assert!(matches!(outcome, Err(FreshResumeError::Probe(ProbeError::Unauthorized))));
                write_new(&root.join("cut-result.txt"), b"revoked_current_no_winner\n", 4096);
            } else {
                let selected = outcome.unwrap();
                assert_eq!(selected.baseline().actual_outputs(), &[None, None, None, None]);
                assert_eq!(selected.current().actual_outputs(), &[Some(0), Some(3), None, Some(u32::MAX)]);
                assert_eq!(selected.baseline().correct_probes(), 1);
                assert_eq!(selected.current().correct_probes(), 4);
                assert_eq!(selected.gain().gain_quarters(), 3);
                let mut reader = file::Inputs { run: &run };
                let mut port = CodingBuildPort::checked(root, coding::SOURCE, &pipeline,
                    selected.history().original_request_root().unwrap(),
                    file::parse(&pipeline.program_ir.program_digest), selected.history().original_plan_digest(),
                    &mut reader).unwrap();
                // This is a new valid Test grant at revision2. Historical
                // application snapshots retain revision1; content still matches.
                let mut fresh_authority = coding::snapshot_authority(root, &issuer, &verifier, 200, AuthorityRevision(2));
                let fresh = selected.selected_current(&plan, &mut port, &mut fresh_authority, &view);
                assert!(fresh.selection(&view).unwrap());
                let actual = fresh.capture_attempt().unwrap().current().unwrap();
                assert_eq!(actual.authority_before().authority_revision, AuthorityRevision(2));
                assert_eq!(actual.authority_after().unwrap().authority_revision, AuthorityRevision(2));
                assert_ne!(actual.snapshot_digest(), file::parse(&application.after_snapshot.snapshot_digest));
                assert_eq!(selected.baseline_build_correspondence(), BaselineBuildCorrespondence::BoundByActualBaselineProducer);
                if case == ResumeCase::Physical {
                    let changed = coding::AFTER.replace("checked_add", "checked_sub");
                    assert_eq!(changed.len(), coding::AFTER.len());
                    std::fs::write(root.join("src/lib.rs"), changed).unwrap();
                    let rejected = selected.selected_current(&plan, &mut port, &mut fresh_authority, &view);
                    assert!(matches!(rejected.selection(&view),
                        Err(FreshResumeError::Probe(ProbeError::PhysicalTargetChanged))));
                    write_new(&root.join("physical-cut-result.txt"), b"changed_current_no_fresh_winner\n", 4096);
                } else if matches!(case, ResumeCase::Cargo | ResumeCase::OtherSource) {
                    let (path, output) = if case == ResumeCase::Cargo {
                        (root.join("Cargo.toml"), "cargo-cut-result.txt")
                    } else { (root.join("src/extra.rs"), "source-cut-result.txt") };
                    let before_bytes = read_owned(&path, 16_384);
                    let mut changed = before_bytes.clone();
                    changed.extend_from_slice(if case == ResumeCase::Cargo {
                        b"\n# owned build-input drift\n".as_slice()
                    } else { b"\n// owned build-input drift\n".as_slice() });
                    std::fs::write(&path, changed).unwrap();
                    let rejected = selected.selected_current(&plan, &mut port, &mut fresh_authority, &view);
                    assert!(matches!(rejected.selection(&view), Err(FreshResumeError::Build(SnapshotGateError::ContentMismatch))));
                    let actual = rejected.capture_attempt().unwrap().capture().unwrap();
                    assert_ne!(actual.content_pins().build, selected.history().after_build());
                    // Restore only this fixture-owned injected input. Neither
                    // the file driver nor quota owner is invoked by this cut.
                    std::fs::write(&path, before_bytes).unwrap();
                    write_new(&root.join(output), b"actual_declared_build_changed_no_winner\n", 4096);
                } else if case == ResumeCase::LateAuthority {
                    let mut late = coding::snapshot_authority(root, &issuer, &verifier, 200, AuthorityRevision(2));
                    late.reject_after = Some(1);
                    let rejected = selected.selected_current(&plan, &mut port, &mut late, &view);
                    assert!(matches!(rejected.selection(&view), Err(FreshResumeError::Build(SnapshotGateError::Capture(
                        lc631_analysis::declared_build_capture::SnapshotCaptureError::AuthorityUnavailable)))));
                    assert!(rejected.capture_attempt().unwrap().capture().is_some());
                    assert!(rejected.capture_attempt().unwrap().current().is_err());
                    write_new(&root.join("late-cut-result.txt"), b"late_authority_failure_retains_actual_capture\n", 4096);
                } else {
                    let result = serde_json::to_vec(&serde_json::json!({"baseline_correct":1,"current_correct":4,
                        "gain_quarters":3,"selected_current":true,"before_build_bound":true,"fresh_authority_revision":2})).unwrap();
                    write_new(&root.join("fresh-result.json"), &result, 4096);
                }
            }
        }));
    }
    reservation.release_confirmed().unwrap(); run.close_confirmed().unwrap();
}
