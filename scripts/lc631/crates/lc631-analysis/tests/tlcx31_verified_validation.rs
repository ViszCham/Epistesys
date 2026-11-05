//! Actual file effects and signed receipts precede every token observation.
#![forbid(unsafe_code)]
#[path = "tlcx31_file_repair_fixture/mod.rs"]
mod fixture;
use fixture::authority::permit;
use fixture::validation::{PartialFileValidator, ValidationTrace};
use fixture::{CountingFileDriver, SOURCE};
use lc631_analysis::*;
use lc631_core::{stable_sha256, Action, AuthorityRevision, CallerOrigin, ExecutionPermitContext};
use lc631_receipt_kernel::{ReceiptIssuer, ReceiptVerifier, ReplayGuard};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
enum Cut {
    None,
    Binding,
    Lease,
    Receipt,
    Hook,
    ValidationReceipt,
}

struct TokenView {
    accepted: String,
    verified: String,
    request_digest: String,
    application_request_digest: String,
    snapshot: String,
    checked_epoch: u64,
}

struct ObservingDriver<'a> {
    inner: CountingFileDriver<'a>,
    cut: Cut,
    tokens: Vec<TokenView>,
}

impl DgclRepairDriver for ObservingDriver<'_> {
    fn observation_epoch(&self) -> Option<u64> {
        self.inner.observation_epoch()
    }

    fn apply(
        &mut self,
        request: &DgclRepairRequest,
        permit: &lc631_core::ExecutionPermit,
        remaining: Duration,
        allowance: u64,
    ) -> Result<DgclRepairApplication, DgclRepairDriverError> {
        let mut application = self.inner.apply(request, permit, remaining, allowance)?;
        match self.cut {
            Cut::Binding => application.source_revision = stable_sha256("wrong source"),
            Cut::Lease => application.after_lease.expires_at_epoch = 0,
            Cut::Receipt => application.attestation = None,
            Cut::None | Cut::Hook | Cut::ValidationReceipt => {}
        }
        Ok(application)
    }

    fn on_verified_validation(
        &mut self,
        token: DgclVerifiedRepairValidation<'_>,
    ) -> Result<(), DgclRepairDriverError> {
        self.tokens.push(TokenView {
            accepted: token.validated_receipt_digest().into(),
            verified: token.validated_receipt().receipt_digest(),
            request_digest: dgcl_repair_request_digest(token.request()),
            application_request_digest: token.application().request_digest.clone(),
            snapshot: token.application().after_snapshot.snapshot_digest.clone(),
            checked_epoch: token.checked_epoch(),
        });
        if matches!(self.cut, Cut::Hook) {
            Err(DgclRepairDriverError::Unavailable)
        } else {
            Ok(())
        }
    }

    fn revalidate(
        &mut self,
        request: &DgclRepairRequest,
        application: &DgclRepairApplication,
        remaining: Duration,
    ) -> Result<DgclRepairValidation, DgclRepairDriverError> {
        let mut result = self.inner.revalidate(request, application, remaining)?;
        if matches!(self.cut, Cut::ValidationReceipt) { result.attestation = None; }
        Ok(result)
    }
}

struct Observed {
    report: DgclRepairTotalBudgetReport,
    tokens: Vec<TokenView>,
    validations: usize,
    snapshot_count: usize,
    actual: String,
    correct: String,
    backup: Vec<String>,
    initial: String,
    cost: u64,
    allowance: u64,
}

fn actual_observation(cut: Cut) -> Observed {
    let root = fixture::fixture_package();
    let correct = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let initial = correct.replace("a.checked_add(b)", "None");
    std::fs::write(root.join("src/lib.rs"), &initial).unwrap();
    let pipeline = build_dgcl_pipeline(
        SOURCE,
        DgclBackendSet {
            english: None,
            japanese: None,
        },
    )
    .unwrap();
    let issuer = ReceiptIssuer::from_key_bytes("tlcx31-verified-tool", &[95; 32]).unwrap();
    let verifier = ReceiptVerifier::from_key_bytes("tlcx31-verified-tool", &[95; 32]).unwrap();
    let author = ReceiptIssuer::from_key_bytes("tlcx31-verified-intent", &[96; 32]).unwrap();
    let author_verifier =
        ReceiptVerifier::from_key_bytes("tlcx31-verified-intent", &[96; 32]).unwrap();
    let scope = format!(
        "dgcl/repair/{}/src/lib.rs",
        dgcl_repository_root_digest(&root).unwrap()
    );
    let edit = permit(SOURCE, &scope, &issuer, &verifier, Action::Edit);
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
    let snapshot = capture_dgcl_cargo_snapshot(&root, SOURCE, &pipeline, &authority).unwrap();
    let lease = issue_dgcl_evidence_lease(&snapshot, 100, 300).unwrap();
    let requests = [fixture::request(
        &pipeline, &edit, &root, 1, &initial, &correct,
    )];
    let gaps = pipeline
        .implementation_closure
        .gaps
        .iter()
        .map(|gap| gap.gap_id.clone())
        .collect::<Vec<_>>();
    let authorization = DgclRepairAuthorization {
        permit: &edit,
        authority_revision: AuthorityRevision(1),
        trusted_host_fingerprint: verifier.key_fingerprint(),
        now_epoch: 100,
        revocation_revision: 0,
        revoked_permit_digests: &[],
    };
    let cargo = fixture::cargo_path();
    let mut trace = ValidationTrace::default();
    let mut validator = PartialFileValidator {
        root: &root,
        source: SOURCE,
        pipeline: &pipeline,
        issuer: &issuer,
        verifier: &verifier,
        author: &author,
        author_verifier: &author_verifier,
        cargo: &cargo,
        trace: &mut trace,
    };
    let inner = DgclFileRepairDriver::new(DgclFileRepairConfig {
        root: &root,
        source: SOURCE,
        pipeline: &pipeline,
        initial_snapshot: &snapshot,
        authorization: &authorization,
        issuer: &issuer,
        verifier: &verifier,
        candidates: std::collections::BTreeMap::from([(stable_sha256(&correct), correct.clone())]),
        validator: &mut validator,
        epoch_clock: None,
    })
    .unwrap();
    let mut driver = ObservingDriver {
        inner: CountingFileDriver {
            inner,
            applies: 0,
            validations: 0,
            fail_after_actual_apply: false,
        },
        cut,
        tokens: Vec::new(),
    };
    let cost = initial.len().max(correct.len()) as u64;
    let allowance = cost + 10;
    let report = run_dgcl_repair_loop_with_total_budget(
        DgclRepairLoopInput {
            source: SOURCE,
            source_revision: &pipeline.source_revision,
            plan_digest: &pipeline.completion_plan.plan_digest,
            program_ir: &pipeline.program_ir,
            translation: &pipeline.translation_projection,
            completion_plan: &pipeline.completion_plan,
            initial_snapshot: &snapshot,
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
            max_attempts: 4,
            max_wall_time: Duration::from_secs(180),
            max_delta_bytes: allowance,
        },
        &mut driver,
        allowance,
    )
    .unwrap();
    let tokens = std::mem::take(&mut driver.tokens);
    let validations = driver.inner.validations;
    drop(driver);
    let actual = std::fs::read_to_string(root.join("src/lib.rs")).unwrap();
    let backup = std::fs::read_dir(root.join(".dgcl-repair-backups"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect();
    Observed {
        report,
        tokens,
        validations,
        snapshot_count: trace.snapshots.len(),
        actual,
        correct,
        backup,
        initial,
        cost,
        allowance,
    }
}

fn diagnostic(cut: Cut, result: &Observed) {
    eprintln!("TLCX31 token cut={cut:?} actual_correct={} backups={} tokens={} validations={} status={:?} charged={} reserved={} effects={:?}",
        result.actual == result.correct, result.backup.len(), result.tokens.len(), result.validations,
        result.report.repair.status, result.report.budget.charged_bytes, result.report.budget.reserved_bytes,
        result.report.repair.attempts.iter().map(|a| a.effect_state).collect::<Vec<_>>());
}

#[test]
fn tlcx31_revalidation_actual_file_token_is_bound_to_accepted_receipt_after_settlement() {
    let result = actual_observation(Cut::None);
    diagnostic(Cut::None, &result);
    assert_eq!(result.report.repair.status, DgclRepairStatus::Completed);
    assert_eq!(result.tokens.len(), 1);
    assert_eq!(result.validations, 1);
    assert_eq!(result.snapshot_count, 1);
    assert_eq!(result.actual, result.correct);
    assert_eq!(result.backup, vec![result.initial]);
    let token = &result.tokens[0];
    assert_eq!(token.accepted, token.verified);
    assert_eq!(token.request_digest, token.application_request_digest);
    assert_eq!(
        Some(&token.snapshot),
        result.report.repair.attempts[0]
            .after_snapshot_digest
            .as_ref()
    );
    assert_eq!(
        Some(&token.accepted),
        result.report.repair.attempts[0]
            .validation_receipt_digest
            .as_ref()
    );
    assert!(token.checked_epoch >= 100);
    assert_eq!(result.report.budget.charged_bytes, result.cost);
    assert_eq!(result.report.budget.reserved_bytes, 0);
}

#[test]
fn tlcx31_revalidation_binding_lease_receipt_cuts_never_deliver_token_or_release_reservation() {
    for cut in [Cut::Binding, Cut::Lease, Cut::Receipt] {
        let result = actual_observation(cut);
        diagnostic(cut, &result);
        assert!(result.tokens.is_empty());
        assert_eq!(result.validations, 0);
        assert_eq!(result.report.repair.status, DgclRepairStatus::Conflict);
        assert_eq!(
            result.report.repair.attempts[0].effect_state,
            RepairEffectState::MayHaveApplied
        );
        assert_eq!(result.actual, result.correct);
        assert_eq!(result.backup, vec![result.initial]);
        assert_eq!(result.report.budget.charged_bytes, 0);
        assert_eq!(result.report.budget.reserved_bytes, result.allowance);
    }
}

#[test]
fn tlcx31_revalidation_hook_failure_holds_actual_effect_and_retains_uncertain_allowance() {
    let result = actual_observation(Cut::Hook);
    diagnostic(Cut::Hook, &result);
    assert_eq!(result.tokens.len(), 1);
    assert_eq!(result.validations, 1);
    assert_eq!(result.report.repair.status, DgclRepairStatus::Hold);
    assert_eq!(
        result.report.repair.attempts[0].effect_state,
        RepairEffectState::ReceiptVerified
    );
    assert_eq!(result.actual, result.correct);
    assert_eq!(result.backup, vec![result.initial]);
    assert_eq!(result.report.budget.charged_bytes, result.cost);
    assert_eq!(result.report.budget.reserved_bytes, 0);
    assert_eq!(
        Some(&result.tokens[0].accepted),
        result.report.repair.attempts[0]
            .validation_receipt_digest
            .as_ref()
    );
}

#[test]
fn tlcx31_revalidation_receipt_cut_retains_actual_effect_charge_without_callback() {
    let result = actual_observation(Cut::ValidationReceipt);
    assert_eq!(result.validations, 1);
    assert!(result.tokens.is_empty());
    assert_eq!(result.actual, result.correct);
    assert_eq!(result.backup, vec![result.initial]);
    assert_eq!(result.report.repair.status, DgclRepairStatus::Conflict);
    assert_eq!(result.report.repair.attempts[0].effect_state, RepairEffectState::ReceiptVerified);
    assert!(result.report.repair.attempts[0].application_receipt_digest.is_some());
    assert!(result.report.repair.attempts[0].validation_receipt_digest.is_none());
    assert_eq!(result.report.budget.charged_bytes, result.cost);
    assert_eq!(result.report.budget.reserved_bytes, 0);
    assert!(!result.report.repair.remaining_gap_ids.is_empty());
}
