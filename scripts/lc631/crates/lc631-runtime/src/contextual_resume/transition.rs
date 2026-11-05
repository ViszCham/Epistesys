//! 実適用の履歴だけを束縛する。権限・完了・quotaを再発行しない。
//! Binds actual application history only; never reissues authority/completion/quota.
#![forbid(unsafe_code)]

use crate::contextual_repair::measurement::{CodingProbePlan, OwnedCodingProbeObservation};
use crate::journal::{
    decode_segment, inspect_chain, JournalChainDecision, JournalChainHoldReason,
    JournalChainLimits, JournalChainSummary, JournalError, JournalHead, JournalRecordKind,
};
use lc631_analysis::{
    dgcl_repair_application_payload_digest, dgcl_repair_application_scope,
    dgcl_repair_request_digest, validate_dgcl_target_lease, DgclLifecycleState,
    DgclRepairApplication, DgclRepairRequest, DgclVerifiedRepairApplication,
};
use lc631_core::{context::ContextRevision, stable_sha256};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard, SubjectRevision,
    UntrustedReceipt,
};
use lc631_substrate::{
    resource::{
        ResourceAmount, ResourceError, ResourceKind, ResourceRequest, ResourceReservationView,
    },
    Digest32,
};
use sha2::{Digest, Sha256};

const PIN_COUNT: usize = 14;
pub const TRANSITION_BYTES: usize = 16 + PIN_COUNT * 32 + 8;
pub const BOUND_TRANSITION_BYTES: usize = TRANSITION_BYTES + 32;
const DOMAIN: &[u8] = b"Epistesys/TLCX/CodingResumeTransition/v1\0";

/// Plan metadata must also match both freshly authenticated native probe registrations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionPins {
    pub context: ContextRevision,
    pub comparison_scope: Digest32,
    pub probe_plan: Digest32,
    pub goal: Digest32,
    pub policy: Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransitionError {
    InvalidPayload,
    BindingMismatch,
    MissingAttestation,
    ReceiptRejected,
    ClockRollback,
    MissingHistory,
    UnsupportedRecord,
    InputLimit,
    Resource(ResourceError),
    Chain(JournalChainHoldReason),
    Journal(JournalError),
}

/// Exact bounded signing/persistence material. Decode never authenticates it.
pub struct UntrustedTransitionCheckpoint {
    bytes: [u8; BOUND_TRANSITION_BYTES],
    length: usize,
}
impl UntrustedTransitionCheckpoint {
    /// Called while the coordinator's borrowed actual-application token is live.
    /// The caller must separately attest/persist this payload; capture does neither.
    pub fn capture(
        token: &DgclVerifiedRepairApplication<'_>,
        pins: TransitionPins,
    ) -> Result<Self, TransitionError> {
        let request = token.request();
        let application = token.application();
        check_application(request, application)?;
        if !application.applied
            || parse_digest(&request.source_revision)? != pins.context.source().digest()
            || token.validated_receipt_digest()
                != application
                    .attestation
                    .as_ref()
                    .ok_or(TransitionError::MissingAttestation)?
                    .receipt_digest()
        {
            return Err(TransitionError::BindingMismatch);
        }
        let values = [
            pins.context.source().digest(),
            pins.context.interpretation_digest(),
            pins.comparison_scope,
            pins.probe_plan,
            pins.goal,
            pins.policy,
            parse_digest(&dgcl_repair_request_digest(request))?,
            parse_digest(&request.plan_digest)?,
            parse_digest(&stable_sha256(&request.target_ref))?,
            parse_digest(&application.target_digest_before)?,
            parse_digest(&application.target_digest_after)?,
            parse_digest(&application.after_snapshot.snapshot_digest)?,
            parse_digest(&dgcl_repair_application_payload_digest(application))?,
            parse_digest(token.validated_receipt_digest())?,
        ];
        let mut bytes = [0; TRANSITION_BYTES];
        bytes[..8].copy_from_slice(b"ETRANS01");
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        for (slot, value) in bytes[16..16 + PIN_COUNT * 32]
            .as_chunks_mut::<32>()
            .0
            .iter_mut()
            .zip(values)
        {
            slot.copy_from_slice(value.as_bytes());
        }
        bytes[16 + PIN_COUNT * 32..].copy_from_slice(&token.checked_epoch().to_le_bytes());
        Self::decode(&bytes)
    }
    /// V2 adds the complete build identity from an actual live pre-repair probe
    /// owner. Caller-provided before-build strings have no admission path.
    pub fn capture_with_baseline(
        token: &DgclVerifiedRepairApplication<'_>,
        pins: TransitionPins,
        baseline: &OwnedCodingProbeObservation<'_>,
        plan: &CodingProbePlan,
        resources: &ResourceReservationView<'_>,
    ) -> Result<Self, TransitionError> {
        baseline
            .check_current(plan, resources)
            .map_err(|_| TransitionError::BindingMismatch)?;
        if baseline.task_source() != pins.context.source().digest()
            || baseline.plan_digest() != pins.probe_plan
            || plan.digest() != pins.probe_plan
            || plan.goal() != pins.goal
            || plan.policy() != pins.policy
            || baseline.target() != parse_digest(&token.application().target_digest_before)?
            || baseline.checked_epoch() > token.checked_epoch()
        {
            return Err(TransitionError::BindingMismatch);
        }
        let mut captured = Self::capture(token, pins)?;
        captured.length = BOUND_TRANSITION_BYTES;
        captured.bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
        captured.bytes[TRANSITION_BYTES..]
            .copy_from_slice(baseline.build_input_digest().as_bytes());
        Self::decode(captured.as_bytes())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, TransitionError> {
        let version_valid = (bytes.len() == TRANSITION_BYTES
            && bytes.get(8..10) == Some([1, 0].as_slice()))
            || (bytes.len() == BOUND_TRANSITION_BYTES
                && bytes.get(8..10) == Some([2, 0].as_slice()));
        if !version_valid
            || bytes.get(..8) != Some(b"ETRANS01")
            || bytes
                .get(10..16)
                .is_none_or(|reserved| reserved.iter().any(|b| *b != 0))
        {
            return Err(TransitionError::InvalidPayload);
        }
        let mut exact = [0; BOUND_TRANSITION_BYTES];
        exact[..bytes.len()].copy_from_slice(bytes);
        let decoded = Self {
            bytes: exact,
            length: bytes.len(),
        };
        if (0..PIN_COUNT).any(|index| decoded.pin(index) == Digest32::from_bytes([0; 32]))
            || decoded.pin(9) == decoded.pin(10)
            || decoded.before_build() == Some(Digest32::from_bytes([0; 32]))
        {
            return Err(TransitionError::InvalidPayload);
        }
        Ok(decoded)
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    pub fn payload_digest(&self) -> Digest32 {
        let mut hash = Sha256::new();
        hash.update(if self.length == BOUND_TRANSITION_BYTES {
            b"Epistesys/TLCX/CodingResumeTransition/v2\0".as_slice()
        } else {
            DOMAIN
        });
        hash.update(self.as_bytes());
        Digest32::from_bytes(hash.finalize().into())
    }
    pub fn receipt_scope(&self) -> String {
        let version = if self.length == BOUND_TRANSITION_BYTES {
            2
        } else {
            1
        };
        format!(
            "tlcx/coding-resume-transition/v{version}/{}",
            digest_text(self.pin(6))
        )
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "untrusted exact transition bytes only; no historical authenticity, current permission, completion or quota"
    }
    fn pin(&self, index: usize) -> Digest32 {
        let mut value = [0; 32];
        value.copy_from_slice(&self.bytes[16 + index * 32..16 + (index + 1) * 32]);
        Digest32::from_bytes(value)
    }
    fn checked_epoch(&self) -> u64 {
        let mut value = [0; 8];
        value.copy_from_slice(&self.bytes[16 + PIN_COUNT * 32..TRANSITION_BYTES]);
        u64::from_le_bytes(value)
    }
    fn before_build(&self) -> Option<Digest32> {
        if self.length != BOUND_TRANSITION_BYTES {
            return None;
        }
        let mut digest = [0; 32];
        digest.copy_from_slice(&self.bytes[TRANSITION_BYTES..]);
        Some(Digest32::from_bytes(digest))
    }
}

/// Historical signatures and local continuity only. This private owner has no
/// Clone/Deserialize or conversion into an application token/execution permit.
pub struct HistoricalRepairTransition {
    checkpoint: UntrustedTransitionCheckpoint,
    chain: JournalChainSummary,
    original_request_root: Option<Digest32>,
    after_build: Digest32,
    after_profile: Digest32,
    after_validator: Digest32,
    after_targets: Digest32,
}
impl HistoricalRepairTransition {
    pub fn chain(&self) -> JournalChainSummary {
        self.chain
    }
    pub fn task_source(&self) -> Digest32 {
        self.checkpoint.pin(0)
    }
    pub fn context(&self) -> Digest32 {
        self.checkpoint.pin(1)
    }
    pub fn comparison_scope(&self) -> Digest32 {
        self.checkpoint.pin(2)
    }
    pub fn probe_plan(&self) -> Digest32 {
        self.checkpoint.pin(3)
    }
    pub fn goal(&self) -> Digest32 {
        self.checkpoint.pin(4)
    }
    pub fn policy(&self) -> Digest32 {
        self.checkpoint.pin(5)
    }
    /// Original signed request identity; never supplied by a resume caller.
    /// Legacy missing roots remain None and cannot satisfy a current root gate.
    pub fn original_request_root(&self) -> Option<Digest32> {
        self.original_request_root
    }
    pub fn original_plan_digest(&self) -> Digest32 {
        self.checkpoint.pin(7)
    }
    pub fn target_ref_digest(&self) -> Digest32 {
        self.checkpoint.pin(8)
    }
    pub fn before_target(&self) -> Digest32 {
        self.checkpoint.pin(9)
    }
    pub fn after_target(&self) -> Digest32 {
        self.checkpoint.pin(10)
    }
    pub fn after_build(&self) -> Digest32 {
        self.after_build
    }
    pub fn after_profile(&self) -> Digest32 {
        self.after_profile
    }
    pub fn after_validator(&self) -> Digest32 {
        self.after_validator
    }
    pub fn after_targets(&self) -> Digest32 {
        self.after_targets
    }
    pub fn before_build(&self) -> Option<Digest32> {
        self.checkpoint.before_build()
    }
    pub const fn claim_boundary(&self) -> &'static str {
        "historical application/transition signatures only; Unanchored; RollbackResistanceUnavailable; later effects/delivery unknown; fresh native observations required; no authority, completion, quota, apply or retransmit"
    }
}

pub struct TransitionEvidence<'a> {
    pub request: &'a DgclRepairRequest,
    pub application: &'a DgclRepairApplication,
    pub transition_attestation: &'a UntrustedReceipt,
    pub verifier: &'a ReceiptVerifier,
    pub now_epoch: u64,
}

/// Dedicated, bounded transition-journal profile, separate from legacy checkpoint
/// and quota replay. Every record is checked; nothing is appended or applied.
pub fn inspect_transition_chain(
    segments: &[&[u8]],
    local_head: Option<&JournalHead>,
    limits: JournalChainLimits,
    evidence: TransitionEvidence<'_>,
    resources: &ResourceReservationView<'_>,
) -> Result<HistoricalRepairTransition, TransitionError> {
    resources.check().map_err(TransitionError::Resource)?;
    let amount = ResourceAmount::checked(65_536).map_err(TransitionError::Resource)?;
    let _claim = resources
        .claim_resources(&[
            ResourceRequest::new(ResourceKind::Ram, amount),
            ResourceRequest::new(ResourceKind::Output, amount),
        ])
        .map_err(TransitionError::Resource)?;
    let chain = match inspect_chain(segments.iter().copied(), local_head, limits) {
        JournalChainDecision::Inspected(chain) => chain,
        JournalChainDecision::Hold(error) => return Err(TransitionError::Chain(error)),
    };
    if chain.record_count() != 1 || chain.segment_count() != 1 {
        return Err(TransitionError::MissingHistory);
    }
    let segment = decode_segment(segments[0]).map_err(TransitionError::Journal)?;
    let record = segment
        .records()
        .first()
        .ok_or(TransitionError::MissingHistory)?;
    if record.kind() != JournalRecordKind::Evidence {
        return Err(TransitionError::UnsupportedRecord);
    }
    let checkpoint = UntrustedTransitionCheckpoint::decode(record.payload())?;
    if record.object_digest() != checkpoint.payload_digest()
        || resources.source_revision() != checkpoint.pin(0)
    {
        return Err(TransitionError::BindingMismatch);
    }
    verify_history(&checkpoint, &evidence)?;
    resources.check().map_err(TransitionError::Resource)?;
    Ok(HistoricalRepairTransition {
        checkpoint,
        chain,
        original_request_root: evidence
            .request
            .repository_root_digest
            .as_deref()
            .map(parse_digest)
            .transpose()?,
        after_build: parse_digest(&evidence.application.after_snapshot.build_input_digest)?,
        after_profile: parse_digest(&evidence.application.after_snapshot.profile_digest)?,
        after_validator: parse_digest(&evidence.application.after_snapshot.validator_digest)?,
        after_targets: parse_digest(&evidence.application.after_snapshot.targets_digest)?,
    })
}

fn verify_history(
    checkpoint: &UntrustedTransitionCheckpoint,
    evidence: &TransitionEvidence<'_>,
) -> Result<(), TransitionError> {
    let request = evidence.request;
    let application = evidence.application;
    check_application(request, application)?;
    let raw = application
        .attestation
        .as_ref()
        .ok_or(TransitionError::MissingAttestation)?;
    raw.check_size_bounds()
        .map_err(|_| TransitionError::ReceiptRejected)?;
    evidence
        .transition_attestation
        .check_size_bounds()
        .map_err(|_| TransitionError::ReceiptRejected)?;
    let expected = [
        (0, &request.source_revision),
        (6, &dgcl_repair_request_digest(request)),
        (7, &request.plan_digest),
        (8, &stable_sha256(&request.target_ref)),
        (9, &application.target_digest_before),
        (10, &application.target_digest_after),
        (11, &application.after_snapshot.snapshot_digest),
        (12, &dgcl_repair_application_payload_digest(application)),
        (13, &raw.receipt_digest()),
    ];
    for (index, value) in expected {
        if checkpoint.pin(index) != parse_digest(value)? {
            return Err(TransitionError::BindingMismatch);
        }
    }
    let checked = checkpoint.checked_epoch();
    let issued = evidence.transition_attestation.claims().issued_at_epoch();
    if evidence.now_epoch < issued || issued < checked {
        return Err(TransitionError::ClockRollback);
    }
    if validate_dgcl_target_lease(
        &application.after_lease,
        &application.after_snapshot,
        &application.target_ref,
        &application.target_digest_after,
        checked,
    )
    .state
        != DgclLifecycleState::Current
    {
        return Err(TransitionError::BindingMismatch);
    }
    // Verify at the historical checked/issued instants, explicitly NOT current
    // permission. Historical replay guards never restore security replay state.
    let subject = || {
        SubjectRevision::checked(&request.source_revision)
            .map_err(|_| TransitionError::BindingMismatch)
    };
    evidence
        .verifier
        .verify(
            raw.clone(),
            &ReceiptPolicy::exact(
                ReceiptClass::ToolExecution,
                subject()?,
                ReceiptScope::checked(dgcl_repair_application_scope(request))
                    .map_err(|_| TransitionError::BindingMismatch)?,
                dgcl_repair_application_payload_digest(application),
                checked,
            ),
            &mut ReplayGuard::default(),
        )
        .map_err(|_| TransitionError::ReceiptRejected)?;
    evidence
        .verifier
        .verify(
            evidence.transition_attestation.clone(),
            &ReceiptPolicy::exact(
                ReceiptClass::ToolExecution,
                subject()?,
                ReceiptScope::checked(checkpoint.receipt_scope())
                    .map_err(|_| TransitionError::BindingMismatch)?,
                digest_text(checkpoint.payload_digest()),
                issued,
            )
            .with_parent(raw.receipt_digest()),
            &mut ReplayGuard::default(),
        )
        .map_err(|_| TransitionError::ReceiptRejected)?;
    Ok(())
}

fn check_application(
    r: &DgclRepairRequest,
    a: &DgclRepairApplication,
) -> Result<(), TransitionError> {
    // Bound caller-owned metadata before inherited canonical String/JSON hashing.
    // This new finite profile accepts <=64 targets and <=16KiB target labels.
    let strings = [
        r.gap_id.as_str(),
        r.source_revision.as_str(),
        r.plan_digest.as_str(),
        r.target_ref.as_str(),
        r.target_digest_before.as_str(),
        r.candidate_digest.as_str(),
        r.authorization_scope.as_str(),
        a.gap_id.as_str(),
        a.request_digest.as_str(),
        a.source_revision.as_str(),
        a.plan_digest.as_str(),
        a.target_ref.as_str(),
        a.target_digest_before.as_str(),
        a.target_digest_after.as_str(),
        a.candidate_digest.as_str(),
        a.run_digest.as_str(),
        a.permit_digest.as_str(),
    ];
    let snapshot = &a.after_snapshot;
    if strings.iter().any(|s| s.len() > 1024)
        || a.schema_version.len() > 64
        || snapshot.schema_version.len() > 64
        || snapshot.observed_boundary.len() > 1024
        || snapshot.targets.len() > 64
        || snapshot
            .targets
            .iter()
            .try_fold(0_usize, |sum, t| sum.checked_add(t.target_ref.len()))
            .is_none_or(|n| n > 16_384)
        || r.repository_root_digest
            .as_ref()
            .is_some_and(|s| s.len() > 71)
        || [
            snapshot.source_revision.as_str(),
            snapshot.build_input_digest.as_str(),
            snapshot.profile_digest.as_str(),
            snapshot.validator_digest.as_str(),
            snapshot.authority_revision_digest.as_str(),
            snapshot.targets_digest.as_str(),
            snapshot.snapshot_digest.as_str(),
            a.after_lease.lease_digest.as_str(),
        ]
        .iter()
        .any(|s| s.len() > 71)
    {
        return Err(TransitionError::InputLimit);
    }
    if !a.applied
        || a.request_digest != dgcl_repair_request_digest(r)
        || a.source_revision != r.source_revision
        || a.plan_digest != r.plan_digest
        || a.attempt != r.attempt
        || a.gap_id != r.gap_id
        || a.target_ref != r.target_ref
        || a.target_digest_before != r.target_digest_before
        || a.target_digest_after != r.candidate_digest
        || a.candidate_digest != r.candidate_digest
        || a.target_digest_before == a.target_digest_after
    {
        return Err(TransitionError::BindingMismatch);
    }
    Ok(())
}

pub(super) fn parse_digest(text: &str) -> Result<Digest32, TransitionError> {
    let hex = text
        .strip_prefix("sha256:")
        .ok_or(TransitionError::BindingMismatch)?;
    if hex.len() != 64 {
        return Err(TransitionError::BindingMismatch);
    }
    let mut value = [0; 32];
    for (slot, pair) in value
        .iter_mut()
        .zip(hex.as_bytes().as_chunks::<2>().0.iter())
    {
        let nibble = |b| match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(TransitionError::BindingMismatch),
        };
        *slot = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(Digest32::from_bytes(value))
}
pub fn digest_text(value: Digest32) -> String {
    let mut text = String::from("sha256:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in value.as_bytes() {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 15) as usize] as char);
    }
    text
}
