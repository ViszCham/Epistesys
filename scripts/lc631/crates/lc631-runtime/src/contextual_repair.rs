//! Bounded proposal bookkeeping only. No apply, permit, receipt, or closure.
//! Caller feedback remains reported; normal runtime integration is pending.
#![forbid(unsafe_code)]

/// Actual finite probe execution requires separate fresh Test authority.
pub mod execution;
pub mod measurement;
pub mod snapshot_gate;

use lc631_analysis::{repair_request_authorization_scope, DgclRepairRequest};
use lc631_core::context::{ContextRevision, ContextScope, SourceRevision};
use lc631_core::stable_sha256;
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

const ATTEMPTS: usize = 4;
const MAX_REPLACEMENT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepairRunBinding {
    pub context: ContextRevision,
    pub scope: ContextScope,
    pub snapshot: Digest32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepairLimits {
    attempts: u8,
    per_attempt_delta: u64,
    run_total_delta: u64,
}

impl RepairLimits {
    pub fn checked(attempts: u8, per_attempt: u64, total: u64) -> Result<Self, RepairPlanError> {
        if attempts == 0 || usize::from(attempts) > ATTEMPTS || per_attempt == 0 || total == 0 {
            return Err(RepairPlanError::InvalidLimits);
        }
        Ok(Self {
            attempts,
            per_attempt_delta: per_attempt,
            run_total_delta: total,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairPlanError {
    InvalidLimits,
    SourceMismatch,
    BindingMismatch,
    RequestMismatch,
    InvalidTarget,
    DuplicateReplacement,
    AttemptLimit,
    DeltaLimit,
    ClockRollback,
    Deadline,
    AwaitingFeedback,
    Stopped,
    FeedbackMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairPlanState {
    Ready,
    AwaitingFreshAuthority,
    NeedsEvidence,
    NoProgressReported,
    CycleReported,
    MayHaveAppliedReported,
    RejectedReported,
}

/// This classification is supplied by the caller, never checked evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportedRepairFeedback {
    RevalidatedPending { snapshot: Digest32 },
    NoProgress,
    MayHaveApplied,
    ClosureReported,
    Rejected,
}

/// A candidate awaiting external fresh intent/Edit/Test authorization.
/// Its existence does not authorize calling a file driver.
pub struct PendingRepairAttempt<'a> {
    request: DgclRepairRequest,
    replacement: &'a str,
    binding: RepairRunBinding,
    candidate: Digest32,
    charged_delta: u64,
}

impl PendingRepairAttempt<'_> {
    pub fn request(&self) -> &DgclRepairRequest {
        &self.request
    }
    pub fn replacement(&self) -> &str {
        self.replacement
    }
    pub const fn binding(&self) -> RepairRunBinding {
        self.binding
    }
    pub const fn charged_delta(&self) -> u64 {
        self.charged_delta
    }
}

/// Fixed-capacity advisory ledger. Amounts are conservative logical byte
/// charges, not RAM/RSS reservations or observed edit distances.
pub struct ContextualRepairLedger {
    source_revision: String,
    source_bytes: usize,
    request_family: Option<(String, String, Option<String>)>,
    binding: RepairRunBinding,
    limits: RepairLimits,
    deadline: u64,
    last_epoch: u64,
    spent: u64,
    count: u8,
    candidates: [Option<Digest32>; ATTEMPTS],
    snapshots: [Option<Digest32>; ATTEMPTS + 1],
    active: Option<Digest32>,
    state: RepairPlanState,
    last_feedback: Option<ReportedRepairFeedback>,
}

impl ContextualRepairLedger {
    pub fn new(
        source: &str,
        binding: RepairRunBinding,
        limits: RepairLimits,
        start_epoch: u64,
        deadline: u64,
    ) -> Result<Self, RepairPlanError> {
        if binding.context.source() != SourceRevision::of(source) {
            return Err(RepairPlanError::SourceMismatch);
        }
        if deadline <= start_epoch {
            return Err(RepairPlanError::Deadline);
        }
        let mut snapshots = [None; ATTEMPTS + 1];
        snapshots[0] = Some(binding.snapshot);
        Ok(Self {
            source_revision: stable_sha256(source),
            source_bytes: source.len(),
            request_family: None,
            binding,
            limits,
            deadline,
            last_epoch: start_epoch,
            spent: 0,
            count: 0,
            candidates: [None; ATTEMPTS],
            snapshots,
            active: None,
            state: RepairPlanState::Ready,
            last_feedback: None,
        })
    }

    pub const fn state(&self) -> RepairPlanState {
        self.state
    }
    pub const fn charged_total_delta(&self) -> u64 {
        self.spent
    }
    pub const fn attempts(&self) -> u8 {
        self.count
    }
    pub const fn last_feedback(&self) -> Option<ReportedRepairFeedback> {
        self.last_feedback
    }

    fn sample(&mut self, now: u64) -> Result<(), RepairPlanError> {
        if now < self.last_epoch {
            self.state = RepairPlanState::NeedsEvidence;
            return Err(RepairPlanError::ClockRollback);
        }
        self.last_epoch = now;
        if now >= self.deadline {
            self.state = RepairPlanState::NeedsEvidence;
            return Err(RepairPlanError::Deadline);
        }
        Ok(())
    }

    /// The source context is fixed for this ledger. A reconstructed context
    /// requires a separately admitted run; it cannot silently become a file edit.
    pub fn prepare<'a>(
        &mut self,
        request: DgclRepairRequest,
        replacement: &'a str,
        before_bytes: u64,
        current: RepairRunBinding,
        now: u64,
    ) -> Result<PendingRepairAttempt<'a>, RepairPlanError> {
        self.sample(now)?;
        if self.active.is_some() {
            return Err(RepairPlanError::AwaitingFeedback);
        }
        if self.state != RepairPlanState::Ready {
            return Err(RepairPlanError::Stopped);
        }
        if current != self.binding {
            return Err(RepairPlanError::BindingMismatch);
        }
        if self.count >= self.limits.attempts {
            return Err(RepairPlanError::AttemptLimit);
        }
        validate_request(&request, replacement, &self.source_revision, self.count + 1)?;
        if request.authorization_span.end > self.source_bytes {
            return Err(RepairPlanError::RequestMismatch);
        }
        let family = (
            request.plan_digest.clone(),
            request.target_ref.clone(),
            request.repository_root_digest.clone(),
        );
        if self
            .request_family
            .as_ref()
            .is_some_and(|previous| previous != &family)
        {
            return Err(RepairPlanError::BindingMismatch);
        }
        let candidate = Digest32::from_bytes(Sha256::digest(replacement.as_bytes()).into());
        if self.candidates.contains(&Some(candidate)) {
            return Err(RepairPlanError::DuplicateReplacement);
        }
        let delta = before_bytes.max(replacement.len() as u64);
        let total = self
            .spent
            .checked_add(delta)
            .ok_or(RepairPlanError::DeltaLimit)?;
        if delta > self.limits.per_attempt_delta || total > self.limits.run_total_delta {
            return Err(RepairPlanError::DeltaLimit);
        }
        self.candidates[usize::from(self.count)] = Some(candidate);
        self.request_family = Some(family);
        self.count += 1;
        self.spent = total;
        self.active = Some(candidate);
        self.state = RepairPlanState::AwaitingFreshAuthority;
        Ok(PendingRepairAttempt {
            request,
            replacement,
            binding: current,
            candidate,
            charged_delta: delta,
        })
    }

    /// Consumes reported feedback without minting current evidence, completion,
    /// authority or permission. Charges remain spent even on ambiguous effects.
    pub fn record_report(
        &mut self,
        pending: PendingRepairAttempt<'_>,
        feedback: ReportedRepairFeedback,
        now: u64,
    ) -> Result<RepairPlanState, RepairPlanError> {
        self.sample(now)?;
        if self.state != RepairPlanState::AwaitingFreshAuthority {
            return Err(RepairPlanError::Stopped);
        }
        if self.active != Some(pending.candidate)
            || pending.binding != self.binding
            || pending.request.attempt != self.count
        {
            return Err(RepairPlanError::FeedbackMismatch);
        }
        self.active = None;
        self.last_feedback = Some(feedback);
        self.state = match feedback {
            ReportedRepairFeedback::RevalidatedPending { snapshot } => {
                if self.snapshots.contains(&Some(snapshot)) {
                    RepairPlanState::CycleReported
                } else {
                    self.snapshots[usize::from(self.count)] = Some(snapshot);
                    self.binding.snapshot = snapshot;
                    RepairPlanState::Ready
                }
            }
            ReportedRepairFeedback::NoProgress => RepairPlanState::NoProgressReported,
            ReportedRepairFeedback::MayHaveApplied => RepairPlanState::MayHaveAppliedReported,
            ReportedRepairFeedback::ClosureReported => RepairPlanState::NeedsEvidence,
            ReportedRepairFeedback::Rejected => RepairPlanState::RejectedReported,
        };
        Ok(self.state)
    }
}

fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn validate_request(
    request: &DgclRepairRequest,
    replacement: &str,
    source: &str,
    attempt: u8,
) -> Result<(), RepairPlanError> {
    let target = &request.target_ref;
    if target.is_empty()
        || target.len() > 1024
        || !target.ends_with(".rs")
        || target.contains(['\\', ':', '\0'])
        || target
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(RepairPlanError::InvalidTarget);
    }
    let digests = [
        &request.gap_id,
        &request.plan_digest,
        &request.target_digest_before,
    ];
    if request.attempt != attempt
        || request.source_revision != source
        || replacement.len() > MAX_REPLACEMENT_BYTES
        || request.candidate_digest != stable_sha256(replacement)
        || digests.iter().any(|value| !valid_digest(value))
        || request
            .repository_root_digest
            .as_deref()
            .is_none_or(|root| !valid_digest(root))
        || request.authorization_span.start > request.authorization_span.end
        || request.authorization_scope != repair_request_authorization_scope(request)
    {
        return Err(RepairPlanError::RequestMismatch);
    }
    Ok(())
}
