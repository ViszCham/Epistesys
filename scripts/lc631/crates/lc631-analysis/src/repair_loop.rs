use crate::{
    build_dgcl_implementation_closure, validate_dgcl_evidence_lease, validate_dgcl_target_lease,
    verify_dgcl_completion_plan, DgclCompletionPlan, DgclEvidenceLease, DgclExecutionSnapshot,
    DgclImplementationClosureContext, DgclLifecycleState, DgclProgramIrReport,
    DgclTranslationProjection, ImplementationGapState,
};
use lc631_core::{
    stable_sha256, Action, AuthorityRevision, CallerOrigin, DeonticPolarity, ExecutionPermit,
    ExecutionPermitContext, PermitValidation, SourceSpan,
};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

pub const MAX_DGCL_REPAIR_ATTEMPTS: u8 = 4;
const MAX_GAP_IDS: usize = 4_096;
const MAX_TARGET_REF_BYTES: usize = 1_024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DgclRepairRequest {
    pub attempt: u8,
    pub gap_id: String,
    pub source_revision: String,
    pub plan_digest: String,
    pub target_ref: String,
    pub target_digest_before: String,
    pub candidate_digest: String,
    pub authorization_scope: String,
    #[serde(deserialize_with = "decode_repair_span")]
    pub authorization_span: SourceSpan,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_root_digest: Option<String>,
}

fn decode_repair_span<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<SourceSpan, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Coordinates {
        start: usize,
        end: usize,
    }
    let coordinates = Coordinates::deserialize(decoder)?;
    if coordinates.start > coordinates.end {
        return Err(serde::de::Error::custom("invalid repair span order"));
    }
    Ok(SourceSpan {
        start: coordinates.start,
        end: coordinates.end,
    })
}

#[derive(Clone, Debug, Serialize)]
pub struct DgclRepairApplication {
    pub schema_version: &'static str,
    pub attempt: u8,
    pub gap_id: String,
    pub request_digest: String,
    pub source_revision: String,
    pub plan_digest: String,
    pub target_ref: String,
    pub target_digest_before: String,
    pub target_digest_after: String,
    pub candidate_digest: String,
    pub delta_bytes: u64,
    pub run_digest: String,
    pub permit_digest: String,
    pub applied: bool,
    pub after_snapshot: DgclExecutionSnapshot,
    pub after_lease: DgclEvidenceLease,
    pub attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclRepairValidationOutcome {
    ImplementationClosed,
    Pending,
    Conflict,
    Unverifiable,
}

#[derive(Clone, Debug, Serialize)]
pub struct DgclRepairValidation {
    pub schema_version: &'static str,
    pub attempt: u8,
    pub gap_id: String,
    pub request_digest: String,
    pub source_revision: String,
    pub plan_digest: String,
    pub run_digest: String,
    pub snapshot_digest: String,
    pub validator_digest: String,
    pub closure_digest: String,
    pub outcome: DgclRepairValidationOutcome,
    pub remaining_gap_ids: Vec<String>,
    pub attestation: Option<UntrustedReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closure_claims: Option<Vec<crate::CodingEvidenceClaim>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DgclRepairStatus {
    Completed,
    NoWork,
    Hold,
    Conflict,
    NoProgress,
    CycleDetected,
    BudgetExceeded,
    Unauthorized,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclRepairAttemptRecord {
    pub attempt: u8,
    pub gap_id: String,
    pub target_ref: String,
    pub before_snapshot_digest: String,
    pub after_snapshot_digest: Option<String>,
    pub application_receipt_digest: Option<String>,
    pub validation_receipt_digest: Option<String>,
    pub effect_state: RepairEffectState,
    pub outcome: DgclRepairStatus,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairEffectState {
    NotAttempted,
    MayHaveApplied,
    ReceiptVerified,
    ObservedNoProgress,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DgclRepairLoopReport {
    pub schema_version: &'static str,
    pub source_revision: String,
    pub plan_digest: String,
    pub initial_snapshot_digest: String,
    pub final_snapshot_digest: String,
    pub attempts: Vec<DgclRepairAttemptRecord>,
    pub remaining_gap_ids: Vec<String>,
    pub status: DgclRepairStatus,
    pub authority_created: bool,
    pub commit_push_merge_performed: bool,
    pub candidate_only_updates_state: bool,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DgclRepairConfigError {
    InvalidSourceRevision,
    InvalidPlanDigest,
    InvalidGapSet,
    InvalidProgramArtifact,
    InvalidAttemptLimit,
    InvalidBudget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum DgclRepairDriverError {
    Unavailable,
    TimedOut,
    Failed,
}

pub trait DgclRepairDriver {
    /// Trusted adapter clock, never a timestamp from a public receipt/report.
    /// Synthetic SDK drivers can keep the supplied epoch + elapsed default.
    fn observation_epoch(&self) -> Option<u64> {
        None
    }
    fn apply(
        &mut self,
        request: &DgclRepairRequest,
        permit: &ExecutionPermit,
        remaining_time: Duration,
        remaining_delta_bytes: u64,
    ) -> Result<DgclRepairApplication, DgclRepairDriverError>;

    fn revalidate(
        &mut self,
        request: &DgclRepairRequest,
        application: &DgclRepairApplication,
        remaining_time: Duration,
    ) -> Result<DgclRepairValidation, DgclRepairDriverError>;
}

pub struct DgclRepairAuthorization<'a> {
    pub permit: &'a ExecutionPermit,
    pub authority_revision: AuthorityRevision,
    pub trusted_host_fingerprint: &'a str,
    pub now_epoch: u64,
    pub revocation_revision: u64,
    pub revoked_permit_digests: &'a [String],
}

pub struct DgclRepairLoopInput<'a> {
    pub source: &'a str,
    pub source_revision: &'a str,
    pub plan_digest: &'a str,
    pub program_ir: &'a DgclProgramIrReport,
    pub translation: &'a DgclTranslationProjection,
    pub completion_plan: &'a DgclCompletionPlan,
    pub initial_snapshot: &'a DgclExecutionSnapshot,
    pub initial_lease: &'a DgclEvidenceLease,
    pub initial_gap_ids: &'a [String],
    pub requests: &'a [DgclRepairRequest],
    pub authorization: DgclRepairAuthorization<'a>,
    pub verifier: &'a ReceiptVerifier,
    pub replay: &'a mut ReplayGuard,
    pub max_attempts: u8,
    pub max_wall_time: Duration,
    pub max_delta_bytes: u64,
}

pub fn run_dgcl_repair_loop(
    input: DgclRepairLoopInput<'_>,
    driver: &mut dyn DgclRepairDriver,
) -> Result<DgclRepairLoopReport, DgclRepairConfigError> {
    if !is_digest(input.source_revision) {
        return Err(DgclRepairConfigError::InvalidSourceRevision);
    }
    if !is_digest(input.plan_digest) {
        return Err(DgclRepairConfigError::InvalidPlanDigest);
    }
    if stable_sha256(input.source) != input.source_revision
        || input.completion_plan.source_revision != input.source_revision
        || input.completion_plan.plan_digest != input.plan_digest
        || verify_dgcl_completion_plan(
            input.source,
            input.program_ir,
            input.translation,
            input.completion_plan,
        )
        .is_err()
    {
        return Err(DgclRepairConfigError::InvalidProgramArtifact);
    }
    if !unique_digest_set(input.initial_gap_ids) {
        return Err(DgclRepairConfigError::InvalidGapSet);
    }
    if input.max_attempts == 0
        || input.max_attempts > MAX_DGCL_REPAIR_ATTEMPTS
        || input.requests.len() > usize::from(input.max_attempts)
    {
        return Err(DgclRepairConfigError::InvalidAttemptLimit);
    }
    if input.max_wall_time.is_zero() || input.max_delta_bytes == 0 {
        return Err(DgclRepairConfigError::InvalidBudget);
    }
    let baseline_closure = build_dgcl_implementation_closure(
        input.source,
        input.program_ir,
        input.translation,
        input.completion_plan,
        &[],
        DgclImplementationClosureContext {
            current_lifecycle: Some(input.initial_snapshot),
            verifier: None,
            replay: &mut ReplayGuard::default(),
            now_epoch: input.authorization.now_epoch,
        },
    )
    .map_err(|_| DgclRepairConfigError::InvalidProgramArtifact)?;
    let expected_open_gaps = baseline_closure
        .gaps
        .iter()
        .filter(|gap| {
            !matches!(
                gap.state,
                ImplementationGapState::VerifiedReceipt | ImplementationGapState::NotApplicable
            )
        })
        .map(|gap| gap.gap_id.clone())
        .collect::<Vec<_>>();
    let repairable_gap_ids = baseline_closure
        .gaps
        .iter()
        .filter(|gap| {
            matches!(
                gap.state,
                ImplementationGapState::Unavailable
                    | ImplementationGapState::Rejected
                    | ImplementationGapState::Stale
                    | ImplementationGapState::Unresolved
            )
        })
        .map(|gap| gap.gap_id.clone())
        .collect::<BTreeSet<_>>();
    if !unique_digest_set(input.initial_gap_ids)
        || input
            .initial_gap_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != expected_open_gaps.into_iter().collect::<BTreeSet<_>>()
    {
        return Err(DgclRepairConfigError::InvalidGapSet);
    }
    let started = Instant::now();
    let mut snapshot = input.initial_snapshot.clone();
    let mut current_lease = input.initial_lease.clone();
    let initial_snapshot_digest = snapshot.snapshot_digest.clone();
    let mut current_gaps = input
        .initial_gap_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut seen_snapshots = BTreeSet::from([snapshot.snapshot_digest.clone()]);
    let mut attempts = Vec::new();
    if current_gaps.is_empty() {
        return Ok(repair_report(
            input.source_revision,
            input.plan_digest,
            initial_snapshot_digest,
            snapshot.snapshot_digest,
            attempts,
            current_gaps,
            DgclRepairStatus::NoWork,
        ));
    }
    if input.requests.is_empty() {
        return Ok(repair_report(
            input.source_revision,
            input.plan_digest,
            initial_snapshot_digest,
            snapshot.snapshot_digest,
            attempts,
            current_gaps,
            DgclRepairStatus::Hold,
        ));
    }
    let initial_time = input.authorization.now_epoch;
    let initial_lease = validate_dgcl_evidence_lease(&current_lease, &snapshot, initial_time);
    if initial_lease.state != DgclLifecycleState::Current
        || snapshot.source_revision != input.source_revision
    {
        return Ok(repair_report(
            input.source_revision,
            input.plan_digest,
            initial_snapshot_digest,
            snapshot.snapshot_digest,
            attempts,
            current_gaps,
            DgclRepairStatus::Hold,
        ));
    }

    for (offset, request) in input.requests.iter().enumerate() {
        let expected_attempt = u8::try_from(offset + 1).unwrap_or(u8::MAX);
        let before_snapshot_digest = snapshot.snapshot_digest.clone();
        let mut attempt_record = DgclRepairAttemptRecord {
            attempt: expected_attempt,
            gap_id: request.gap_id.clone(),
            target_ref: request.target_ref.clone(),
            before_snapshot_digest,
            after_snapshot_digest: None,
            application_receipt_digest: None,
            validation_receipt_digest: None,
            effect_state: RepairEffectState::NotAttempted,
            outcome: DgclRepairStatus::Hold,
            reason: "repair_attempt_not_completed",
        };
        if request.attempt != expected_attempt || request.attempt > input.max_attempts {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "attempt_index_mismatch_or_limit_exceeded";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        }
        if !current_gaps.contains(&request.gap_id)
            || !repairable_gap_ids.contains(&request.gap_id)
            || request.source_revision != input.source_revision
            || request.plan_digest != input.plan_digest
            || !valid_target_ref(&request.target_ref)
            || !is_digest(&request.gap_id)
            || !is_digest(&request.target_digest_before)
            || !is_digest(&request.candidate_digest)
            || request.authorization_scope != repair_request_authorization_scope(request)
        {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "repair_request_binding_invalid";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        }
        let lifecycle_now = driver.observation_epoch().unwrap_or_else(|| {
            input
                .authorization
                .now_epoch
                .saturating_add(started.elapsed().as_secs())
        });
        let Some(target) = snapshot
            .targets
            .iter()
            .find(|target| target.target_ref == request.target_ref)
        else {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "repair_target_missing_from_snapshot";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        };
        if target.target_digest != request.target_digest_before {
            attempt_record.outcome = DgclRepairStatus::Hold;
            attempt_record.reason = "repair_target_changed_since_plan";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Hold,
            ));
        }
        let lifecycle_now = driver.observation_epoch().unwrap_or(lifecycle_now);
        if validate_dgcl_target_lease(
            &current_lease,
            &snapshot,
            &request.target_ref,
            &request.target_digest_before,
            lifecycle_now,
        )
        .state
            != DgclLifecycleState::Current
        {
            attempt_record.outcome = DgclRepairStatus::Hold;
            attempt_record.reason = "repair_evidence_lease_stale";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Hold,
            ));
        }
        if !permit_authorizes_repair(
            &input.authorization,
            request,
            input.source_revision,
            lifecycle_now,
        ) {
            attempt_record.outcome = DgclRepairStatus::Unauthorized;
            attempt_record.reason = "edit_permit_missing_expired_denied_or_scope_mismatched";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Unauthorized,
            ));
        }
        let remaining = input.max_wall_time.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            attempt_record.outcome = DgclRepairStatus::BudgetExceeded;
            attempt_record.reason = "wall_time_budget_exhausted_before_apply";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::BudgetExceeded,
            ));
        }
        attempt_record.effect_state = RepairEffectState::MayHaveApplied;
        let application = match driver.apply(
            request,
            input.authorization.permit,
            remaining,
            input.max_delta_bytes,
        ) {
            Ok(application) => application,
            Err(error) => {
                attempt_record.reason = driver_error_reason(error);
                attempts.push(attempt_record);
                return Ok(repair_report(
                    input.source_revision,
                    input.plan_digest,
                    initial_snapshot_digest,
                    snapshot.snapshot_digest,
                    attempts,
                    current_gaps,
                    DgclRepairStatus::Hold,
                ));
            }
        };
        attempt_record.after_snapshot_digest =
            Some(application.after_snapshot.snapshot_digest.clone());
        if started.elapsed() > input.max_wall_time {
            attempt_record.outcome = DgclRepairStatus::BudgetExceeded;
            attempt_record.reason = "adapter_returned_after_wall_time_budget";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::BudgetExceeded,
            ));
        }
        if !application_matches_request(
            &application,
            request,
            &snapshot,
            input.source_revision,
            input.plan_digest,
        ) || application.delta_bytes > input.max_delta_bytes
        {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "repair_application_binding_or_resource_limit_mismatch";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        }
        let lifecycle_now = driver.observation_epoch().unwrap_or(lifecycle_now);
        if validate_dgcl_target_lease(
            &application.after_lease,
            &application.after_snapshot,
            &request.target_ref,
            &application.target_digest_after,
            lifecycle_now,
        )
        .state
            != DgclLifecycleState::Current
        {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "post_apply_lifecycle_snapshot_or_lease_invalid";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        }
        if verify_application_receipt(
            &application,
            request,
            input.source_revision,
            input.verifier,
            input.replay,
            lifecycle_now,
        )
        .is_err()
        {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "repair_application_receipt_rejected";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        }
        attempt_record.application_receipt_digest = application
            .attestation
            .as_ref()
            .map(UntrustedReceipt::receipt_digest);
        attempt_record.effect_state = RepairEffectState::ReceiptVerified;
        if !application.applied
            || application.target_digest_after == application.target_digest_before
        {
            attempt_record.effect_state = RepairEffectState::ObservedNoProgress;
            attempt_record.outcome = DgclRepairStatus::NoProgress;
            attempt_record.reason = "repair_applied_no_content_change";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::NoProgress,
            ));
        }
        snapshot = application.after_snapshot.clone();
        current_lease = application.after_lease.clone();
        if seen_snapshots.contains(&application.after_snapshot.snapshot_digest) {
            attempt_record.outcome = DgclRepairStatus::CycleDetected;
            attempt_record.reason = "repair_snapshot_oscillation_detected";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::CycleDetected,
            ));
        }
        seen_snapshots.insert(application.after_snapshot.snapshot_digest.clone());
        let remaining = input.max_wall_time.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            attempt_record.outcome = DgclRepairStatus::BudgetExceeded;
            attempt_record.reason = "wall_time_budget_exhausted_before_revalidation";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::BudgetExceeded,
            ));
        }
        let validation = match driver.revalidate(request, &application, remaining) {
            Ok(validation) => validation,
            Err(error) => {
                attempt_record.reason = driver_error_reason(error);
                attempts.push(attempt_record);
                return Ok(repair_report(
                    input.source_revision,
                    input.plan_digest,
                    initial_snapshot_digest,
                    snapshot.snapshot_digest,
                    attempts,
                    current_gaps,
                    DgclRepairStatus::Hold,
                ));
            }
        };
        if started.elapsed() > input.max_wall_time {
            attempt_record.outcome = DgclRepairStatus::BudgetExceeded;
            attempt_record.reason = "validator_returned_after_wall_time_budget";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::BudgetExceeded,
            ));
        }
        let validation_now = driver.observation_epoch().unwrap_or_else(|| {
            input
                .authorization
                .now_epoch
                .saturating_add(started.elapsed().as_secs())
        });
        if !permit_authorizes_repair(
            &input.authorization,
            request,
            input.source_revision,
            validation_now,
        ) {
            attempt_record.outcome = DgclRepairStatus::Unauthorized;
            attempt_record.reason = "authority_expired_or_revoked_during_validation";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Unauthorized,
            ));
        }
        if !validation_matches_application(
            &validation,
            request,
            &application,
            input.source_revision,
            input.plan_digest,
        ) || !unique_digest_set(&validation.remaining_gap_ids)
            || !validation_closes_only_checked_gaps(
                &validation,
                &application,
                &input,
                &current_gaps,
                &request.gap_id,
                validation_now,
            )
        {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "repair_validation_binding_invalid";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        }
        if verify_validation_receipt(
            &validation,
            request,
            input.source_revision,
            input.verifier,
            input.replay,
            validation_now,
        )
        .is_err()
        {
            attempt_record.outcome = DgclRepairStatus::Conflict;
            attempt_record.reason = "repair_validation_receipt_rejected";
            attempts.push(attempt_record);
            return Ok(repair_report(
                input.source_revision,
                input.plan_digest,
                initial_snapshot_digest,
                snapshot.snapshot_digest,
                attempts,
                current_gaps,
                DgclRepairStatus::Conflict,
            ));
        }
        attempt_record.validation_receipt_digest = validation
            .attestation
            .as_ref()
            .map(UntrustedReceipt::receipt_digest);
        current_gaps = validation.remaining_gap_ids.iter().cloned().collect();
        match validation.outcome {
            DgclRepairValidationOutcome::ImplementationClosed
                if current_gaps.is_empty()
                    && validation
                        .closure_claims
                        .as_ref()
                        .is_some_and(|claims| !claims.is_empty()) =>
            {
                attempt_record.outcome = DgclRepairStatus::Completed;
                attempt_record.reason = "repair_and_full_revalidation_completed";
                attempts.push(attempt_record);
                return Ok(repair_report(
                    input.source_revision,
                    input.plan_digest,
                    initial_snapshot_digest,
                    snapshot.snapshot_digest,
                    attempts,
                    current_gaps,
                    DgclRepairStatus::Completed,
                ));
            }
            DgclRepairValidationOutcome::Conflict => {
                attempt_record.outcome = DgclRepairStatus::Conflict;
                attempt_record.reason = "revalidation_found_conflict";
                attempts.push(attempt_record);
                return Ok(repair_report(
                    input.source_revision,
                    input.plan_digest,
                    initial_snapshot_digest,
                    snapshot.snapshot_digest,
                    attempts,
                    current_gaps,
                    DgclRepairStatus::Conflict,
                ));
            }
            DgclRepairValidationOutcome::Unverifiable => {
                attempt_record.outcome = DgclRepairStatus::Hold;
                attempt_record.reason = "revalidation_not_verifiable";
                attempts.push(attempt_record);
                return Ok(repair_report(
                    input.source_revision,
                    input.plan_digest,
                    initial_snapshot_digest,
                    snapshot.snapshot_digest,
                    attempts,
                    current_gaps,
                    DgclRepairStatus::Hold,
                ));
            }
            DgclRepairValidationOutcome::ImplementationClosed => {
                attempt_record.outcome = DgclRepairStatus::Conflict;
                attempt_record.reason =
                    "closed_status_without_reverified_full_evidence_or_with_remaining_gaps";
                attempts.push(attempt_record);
                return Ok(repair_report(
                    input.source_revision,
                    input.plan_digest,
                    initial_snapshot_digest,
                    snapshot.snapshot_digest,
                    attempts,
                    current_gaps,
                    DgclRepairStatus::Conflict,
                ));
            }
            DgclRepairValidationOutcome::Pending => {
                attempt_record.outcome = DgclRepairStatus::Hold;
                attempt_record.reason = "repair_revalidated_but_gaps_remain";
                attempts.push(attempt_record);
            }
        }
    }
    Ok(repair_report(
        input.source_revision,
        input.plan_digest,
        initial_snapshot_digest,
        snapshot.snapshot_digest,
        attempts,
        current_gaps,
        DgclRepairStatus::Hold,
    ))
}

pub fn repair_authorization_scope(target_ref: &str) -> String {
    format!("dgcl/repair/{target_ref}")
}

pub fn repair_request_authorization_scope(request: &DgclRepairRequest) -> String {
    match request.repository_root_digest.as_deref() {
        Some(root) => format!("dgcl/repair/{root}/{}", request.target_ref),
        None => repair_authorization_scope(&request.target_ref),
    }
}

pub fn dgcl_repair_request_digest(request: &DgclRepairRequest) -> String {
    let legacy_payload = format!(
        "epistesys-dgcl-repair-request.v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        request.attempt,
        request.gap_id,
        request.source_revision,
        request.plan_digest,
        request.target_ref,
        request.target_digest_before,
        request.candidate_digest,
        request.authorization_scope,
        request.authorization_span.start,
        request.authorization_span.end,
    );
    if let Some(root) = &request.repository_root_digest {
        stable_sha256(&format!("{legacy_payload}\0{root}"))
    } else {
        stable_sha256(&legacy_payload)
    }
}

pub fn dgcl_repair_application_scope(request: &DgclRepairRequest) -> String {
    format!(
        "dgcl/repair/{}/{}/application",
        request.attempt, request.gap_id
    )
}

pub fn dgcl_repair_application_payload_digest(application: &DgclRepairApplication) -> String {
    stable_sha256(&format!(
        "epistesys-dgcl-repair-application.v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        application.attempt,
        application.gap_id,
        application.request_digest,
        application.source_revision,
        application.plan_digest,
        application.target_ref,
        application.target_digest_before,
        application.target_digest_after,
        application.candidate_digest,
        application.delta_bytes,
        application.run_digest,
        application.permit_digest,
        application.after_snapshot.snapshot_digest,
        application.after_lease.lease_digest,
    ))
}

pub fn dgcl_repair_validation_scope(request: &DgclRepairRequest) -> String {
    format!(
        "dgcl/repair/{}/{}/validation",
        request.attempt, request.gap_id
    )
}

pub fn dgcl_repair_validation_payload_digest(validation: &DgclRepairValidation) -> String {
    let remaining = validation.remaining_gap_ids.join("\0");
    let legacy_payload = format!(
        "epistesys-dgcl-repair-validation.v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{:?}\0{}\0{}",
        validation.attempt,
        validation.gap_id,
        validation.request_digest,
        validation.source_revision,
        validation.plan_digest,
        validation.run_digest,
        validation.snapshot_digest,
        validation.outcome,
        validation.validator_digest,
        remaining,
    );
    if let Some(claims) = &validation.closure_claims {
        stable_sha256(&format!(
            "{legacy_payload}\0{}",
            stable_sha256(&serde_json::to_string(claims).expect("claims serialize"))
        ))
    } else {
        stable_sha256(&legacy_payload)
    }
}

fn validation_closes_only_checked_gaps(
    validation: &DgclRepairValidation,
    application: &DgclRepairApplication,
    input: &DgclRepairLoopInput<'_>,
    previous: &BTreeSet<String>,
    requested: &str,
    now: u64,
) -> bool {
    let Some(claims) = &validation.closure_claims else {
        return validation_preserves_unrelated_gaps(
            previous,
            requested,
            &validation.remaining_gap_ids,
        );
    };
    if claims.len() > 8192 {
        return false;
    }
    let Ok(closure) = build_dgcl_implementation_closure(
        input.source,
        input.program_ir,
        input.translation,
        input.completion_plan,
        claims,
        DgclImplementationClosureContext {
            current_lifecycle: Some(&application.after_snapshot),
            verifier: Some(input.verifier),
            replay: &mut ReplayGuard::default(),
            now_epoch: now,
        },
    ) else {
        return false;
    };
    let gaps = closure
        .gaps
        .iter()
        .filter(|gap| {
            !matches!(
                gap.state,
                ImplementationGapState::VerifiedReceipt | ImplementationGapState::NotApplicable
            )
        })
        .map(|gap| gap.gap_id.clone())
        .collect::<BTreeSet<_>>();
    gaps == validation.remaining_gap_ids.iter().cloned().collect()
        && validation.closure_digest
            == stable_sha256(&serde_json::to_string(&closure).expect("closure serializes"))
        && (validation.outcome == DgclRepairValidationOutcome::ImplementationClosed)
            == (closure.status == crate::DgclImplementationStatus::ImplementationClosed)
}

fn permit_authorizes_repair(
    authorization: &DgclRepairAuthorization<'_>,
    request: &DgclRepairRequest,
    source_revision: &str,
    now_epoch: u64,
) -> bool {
    if authorization.permit.action() != Action::Edit
        || authorization.permit.polarity() != DeonticPolarity::Grant
        || authorization.permit.source_span() != request.authorization_span
    {
        return false;
    }
    let context = ExecutionPermitContext {
        principal: authorization.permit.principal_binding(),
        action: Action::Edit,
        scope: &request.authorization_scope,
        source_revision,
        authority_revision: authorization.authority_revision,
        now_epoch,
        revocation_revision: authorization.revocation_revision,
        revoked_permit_digests: authorization.revoked_permit_digests,
        caller_origin: CallerOrigin::HostVerifiedUser,
        trusted_host_fingerprint: authorization.trusted_host_fingerprint,
    };
    authorization.permit.validate(&context) == PermitValidation::ValidGrant
}

fn application_matches_request(
    application: &DgclRepairApplication,
    request: &DgclRepairRequest,
    before: &DgclExecutionSnapshot,
    source_revision: &str,
    plan_digest: &str,
) -> bool {
    application.schema_version == "epistesys-dgcl-repair-application.v1"
        && application.attempt == request.attempt
        && application.gap_id == request.gap_id
        && application.request_digest == dgcl_repair_request_digest(request)
        && application.source_revision == source_revision
        && application.plan_digest == plan_digest
        && application.target_ref == request.target_ref
        && application.target_digest_before == request.target_digest_before
        && application.candidate_digest == request.candidate_digest
        && application.target_digest_after == request.candidate_digest
        && is_digest(&application.run_digest)
        && is_digest(&application.permit_digest)
        && application.applied
            == (application.target_digest_after != application.target_digest_before)
        && (application.target_digest_after == application.target_digest_before
            || application.delta_bytes > 0)
        && application.after_snapshot.source_revision == before.source_revision
        // The application receipt binds a new build snapshot. Requiring the
        // old build digest here rejects genuine source repairs and preserves
        // stale compiler evidence. Unrelated target identities stay fixed.
        && is_digest(&application.after_snapshot.build_input_digest)
        && application.after_snapshot.profile_digest == before.profile_digest
        && application.after_snapshot.validator_digest == before.validator_digest
        && application.after_snapshot.authority_revision_digest == before.authority_revision_digest
        && application.after_snapshot.targets.len() == before.targets.len()
        && application.after_snapshot.targets.iter().all(|after| {
            before
                .targets
                .iter()
                .find(|prior| prior.target_ref == after.target_ref)
                .is_some_and(|prior| {
                    if after.target_ref == request.target_ref {
                        after.target_digest == request.candidate_digest
                    } else {
                        after.target_digest == prior.target_digest
                    }
                })
        })
}

fn validation_matches_application(
    validation: &DgclRepairValidation,
    request: &DgclRepairRequest,
    application: &DgclRepairApplication,
    source_revision: &str,
    plan_digest: &str,
) -> bool {
    validation.schema_version == "epistesys-dgcl-repair-validation.v1"
        && validation.attempt == request.attempt
        && validation.gap_id == request.gap_id
        && validation.request_digest == application.request_digest
        && validation.source_revision == source_revision
        && validation.plan_digest == plan_digest
        && validation.run_digest == application.run_digest
        && validation.snapshot_digest == application.after_snapshot.snapshot_digest
        && validation.validator_digest == application.after_snapshot.validator_digest
        && is_digest(&validation.closure_digest)
        && validation.remaining_gap_ids.len() <= MAX_GAP_IDS
        && unique_digest_set(&validation.remaining_gap_ids)
        && (validation.outcome != DgclRepairValidationOutcome::ImplementationClosed
            || validation.remaining_gap_ids.is_empty())
        && (validation.outcome != DgclRepairValidationOutcome::Pending
            || !validation.remaining_gap_ids.is_empty())
}

fn validation_preserves_unrelated_gaps(
    current_gaps: &BTreeSet<String>,
    repaired_gap_id: &str,
    reported_remaining: &[String],
) -> bool {
    let reported = reported_remaining.iter().collect::<BTreeSet<_>>();
    current_gaps
        .iter()
        .filter(|gap_id| gap_id.as_str() != repaired_gap_id)
        .all(|gap_id| reported.contains(gap_id))
}

fn verify_application_receipt(
    application: &DgclRepairApplication,
    request: &DgclRepairRequest,
    source_revision: &str,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<String, ()> {
    let Some(attestation) = application.attestation.clone() else {
        return Err(());
    };
    let subject = SubjectRevision::checked(source_revision).map_err(|_| ())?;
    let scope = ReceiptScope::checked(dgcl_repair_application_scope(request)).map_err(|_| ())?;
    let receipt = verifier
        .verify(
            attestation,
            &ReceiptPolicy::exact(
                ReceiptClass::ToolExecution,
                subject,
                scope,
                dgcl_repair_application_payload_digest(application),
                now_epoch,
            ),
            replay,
        )
        .map_err(|_| ())?;
    Ok(receipt.receipt_digest())
}

fn verify_validation_receipt(
    validation: &DgclRepairValidation,
    request: &DgclRepairRequest,
    source_revision: &str,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<String, ()> {
    let Some(attestation) = validation.attestation.clone() else {
        return Err(());
    };
    let subject = SubjectRevision::checked(source_revision).map_err(|_| ())?;
    let scope = ReceiptScope::checked(dgcl_repair_validation_scope(request)).map_err(|_| ())?;
    let receipt = verifier
        .verify(
            attestation,
            &ReceiptPolicy::exact(
                ReceiptClass::Validation,
                subject,
                scope,
                dgcl_repair_validation_payload_digest(validation),
                now_epoch,
            ),
            replay,
        )
        .map_err(|_| ())?;
    Ok(receipt.receipt_digest())
}

fn repair_report(
    source_revision: &str,
    plan_digest: &str,
    initial_snapshot_digest: String,
    final_snapshot_digest: String,
    attempts: Vec<DgclRepairAttemptRecord>,
    remaining: BTreeSet<String>,
    status: DgclRepairStatus,
) -> DgclRepairLoopReport {
    DgclRepairLoopReport {
        schema_version: "epistesys-dgcl-repair-loop.v1",
        source_revision: source_revision.into(),
        plan_digest: plan_digest.into(),
        initial_snapshot_digest,
        final_snapshot_digest,
        attempts,
        remaining_gap_ids: remaining.into_iter().collect(),
        status,
        authority_created: false,
        commit_push_merge_performed: false,
        candidate_only_updates_state: false,
        claim_boundary: "final_snapshot_digest is the last receipt-verified identity; an attempt may report an unverified after_snapshot_digest without an accepted application receipt; only receipt-verified apply plus fresh validation updates closure state; adapter filesystem confinement and hard cancellation remain host responsibilities",
    }
}

fn unique_digest_set(values: &[String]) -> bool {
    values.len() <= MAX_GAP_IDS
        && values.iter().all(|value| is_digest(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn valid_target_ref(target_ref: &str) -> bool {
    !target_ref.is_empty()
        && target_ref.len() <= MAX_TARGET_REF_BYTES
        && !target_ref.starts_with('/')
        && !target_ref.contains('\\')
        && !target_ref.chars().any(char::is_control)
        && target_ref
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

fn is_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn driver_error_reason(error: DgclRepairDriverError) -> &'static str {
    match error {
        DgclRepairDriverError::Unavailable => "repair_driver_unavailable",
        DgclRepairDriverError::TimedOut => "repair_driver_timeout",
        DgclRepairDriverError::Failed => "repair_driver_failed",
    }
}
