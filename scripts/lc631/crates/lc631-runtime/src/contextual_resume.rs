//! 文脈再開の準備のみ。履歴から権限・完了・配信を復元しない。
//! Contextual resume preparation only; history restores no authority/completion/delivery.
#![forbid(unsafe_code)]
/// Versioned owned budget history, separate from legacy checkpoint semantics.
pub mod budget;
/// Historical application bindings; never renewed authority or quota.
pub mod transition;
/// Fresh execution of the same fixed coding probes after history inspection.
pub mod fresh_reselection;
pub mod production_contract;

use crate::journal::{
    inspect_checkpoint_chain_resume, CheckpointChainResumeDecision, CheckpointChainResumeHold,
    JournalChainLimits, JournalHead,
};
use lc631_analysis::{
    CheckpointEventKind, CheckpointReadIdentity, DeliveryState, ResumeObservation,
};
use lc631_core::context::ContextRevision;
use lc631_substrate::resource::{ResourceError, ResourceReservationView};
use lc631_substrate::Digest32;

/// Caller reports, not authenticated checkpoint fields or verified observations.
/// Legacy checkpoint bytes do not bind these values. Matching them cannot
/// admit a solver result, restore a fairness cursor, or authorize an action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportedContextualPins {
    pub context: ContextRevision,
    pub comparison_scope: Digest32,
    pub solver: Digest32,
    pub options: Digest32,
    pub tool: Digest32,
    pub validator: Digest32,
    pub measurement_definition: Digest32,
    pub policy: Digest32,
    pub evidence: Digest32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PinKind {
    Context,
    ComparisonScope,
    Solver,
    Options,
    Tool,
    Validator,
    MeasurementDefinition,
    Policy,
    Evidence,
}

impl PinKind {
    pub const ALL: [Self; 9] = [
        Self::Context,
        Self::ComparisonScope,
        Self::Solver,
        Self::Options,
        Self::Tool,
        Self::Validator,
        Self::MeasurementDefinition,
        Self::Policy,
        Self::Evidence,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Context => 0,
            Self::ComparisonScope => 1,
            Self::Solver => 2,
            Self::Options => 3,
            Self::Tool => 4,
            Self::Validator => 5,
            Self::MeasurementDefinition => 6,
            Self::Policy => 7,
            Self::Evidence => 8,
        }
    }
}

/// No variant permits application or retransmission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectReconciliation {
    LegacyEffectUnknown,
    ReconcileActionWithoutReapplying,
    FailedActionRequiresFreshIntentPermissionAndStateCheck,
    RevalidateObservedActionWithoutReapplying,
    RebuildCandidateWithoutSending,
    ReconcileDeliveryWithoutResending,
    RevalidateHistoricalSinkWithoutResending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequiredCheck {
    ContextualPayloadBinding,
    CurrentSourceAndContext,
    CurrentToolAndSolver,
    CurrentValidatorAndMeasurement,
    CurrentAuthorityExpiryAndRevocation,
    CurrentEvidenceAndLease,
    RecomputeSelectionAndFourFacets,
    ExternalAnchorOrRollbackResistanceUnavailable,
}

const REQUIRED_CHECKS: [RequiredCheck; 8] = [
    RequiredCheck::ContextualPayloadBinding,
    RequiredCheck::CurrentSourceAndContext,
    RequiredCheck::CurrentToolAndSolver,
    RequiredCheck::CurrentValidatorAndMeasurement,
    RequiredCheck::CurrentAuthorityExpiryAndRevocation,
    RequiredCheck::CurrentEvidenceAndLease,
    RequiredCheck::RecomputeSelectionAndFourFacets,
    RequiredCheck::ExternalAnchorOrRollbackResistanceUnavailable,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextualResumeHold {
    Resource(ResourceError),
    LeaseSourceMismatch,
    ReportedSourceMismatch,
    Chain(CheckpointChainResumeHold),
    MissingHistory,
}

/// A diagnostic plan, never a verified token. All required checks remain open.
/// No caller-supplied completion, authority, delivery or reuse flag is accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextualResumePreparation {
    observation: ResumeObservation,
    changed: [bool; 9],
    reconciliation: EffectReconciliation,
}

impl ContextualResumePreparation {
    pub fn observation(&self) -> &ResumeObservation {
        &self.observation
    }

    pub const fn reported_pin_changed(&self, kind: PinKind) -> bool {
        self.changed[kind.index()]
    }

    pub const fn reconciliation(&self) -> EffectReconciliation {
        self.reconciliation
    }

    pub fn required_checks(&self) -> &[RequiredCheck] {
        &REQUIRED_CHECKS
    }

    pub const fn claim_boundary(&self) -> &'static str {
        "NeedsEvidence: contextual pins are caller reports, not bound checkpoint evidence; no result reuse, permission, completion, application, send or delivery is restored"
    }
}

/// Inspect exact bounded journal bytes rather than accepting a forgeable public
/// ResumeObservation as evidence. The borrowed lease is checked before and
/// after inspection, but its validity is not retained as a future permit.
/// This does not account for all inherited parser allocations or guarantee RSS.
pub fn prepare_contextual_resume(
    segments: &[&[u8]],
    local_head: Option<&JournalHead>,
    limits: JournalChainLimits,
    identity: CheckpointReadIdentity<'_>,
    historical: &ReportedContextualPins,
    current: &ReportedContextualPins,
    lease: &ResourceReservationView<'_>,
) -> Result<ContextualResumePreparation, ContextualResumeHold> {
    lease.check().map_err(ContextualResumeHold::Resource)?;
    if lease.source_revision() != current.context.source().digest() {
        return Err(ContextualResumeHold::LeaseSourceMismatch);
    }
    if historical.context.source() != current.context.source() {
        return Err(ContextualResumeHold::ReportedSourceMismatch);
    }
    // Bind the current reported source to the actual legacy identity read.
    let mut expected = String::from("sha256:");
    for byte in current.context.source().digest().as_bytes() {
        use std::fmt::Write as _;
        if write!(expected, "{byte:02x}").is_err() {
            return Err(ContextualResumeHold::ReportedSourceMismatch);
        }
    }
    if expected != identity.source_revision {
        return Err(ContextualResumeHold::ReportedSourceMismatch);
    }
    let observation = match inspect_checkpoint_chain_resume(segments, local_head, limits, identity)
    {
        CheckpointChainResumeDecision::Candidate { observation, .. } => observation,
        CheckpointChainResumeDecision::Hold(reason) => {
            return Err(ContextualResumeHold::Chain(reason))
        }
    };
    lease.check().map_err(ContextualResumeHold::Resource)?;
    let event = observation
        .latest_event_kind
        .ok_or(ContextualResumeHold::MissingHistory)?;
    let delivery = observation
        .latest_delivery
        .ok_or(ContextualResumeHold::MissingHistory)?;
    let changed = [
        historical.context != current.context,
        historical.comparison_scope != current.comparison_scope,
        historical.solver != current.solver,
        historical.options != current.options,
        historical.tool != current.tool,
        historical.validator != current.validator,
        historical.measurement_definition != current.measurement_definition,
        historical.policy != current.policy,
        historical.evidence != current.evidence,
    ];
    Ok(ContextualResumePreparation {
        observation,
        changed,
        reconciliation: reconcile_effect(event, delivery),
    })
}

/// Pure obligation classifier; not an event-transition validator or permission.
pub fn reconcile_effect(
    event: CheckpointEventKind,
    delivery: DeliveryState,
) -> EffectReconciliation {
    match event {
        CheckpointEventKind::LegacyUnclassified => EffectReconciliation::LegacyEffectUnknown,
        CheckpointEventKind::ActionStarted => {
            EffectReconciliation::ReconcileActionWithoutReapplying
        }
        CheckpointEventKind::ActionObserved => match delivery {
            DeliveryState::UnknownDelivery => {
                EffectReconciliation::ReconcileActionWithoutReapplying
            }
            DeliveryState::ConfirmedFailed => {
                EffectReconciliation::FailedActionRequiresFreshIntentPermissionAndStateCheck
            }
            DeliveryState::ConfirmedSuccess => {
                EffectReconciliation::RevalidateObservedActionWithoutReapplying
            }
        },
        CheckpointEventKind::CandidatePrepared => {
            EffectReconciliation::RebuildCandidateWithoutSending
        }
        CheckpointEventKind::CandidateSendAttempted => {
            EffectReconciliation::ReconcileDeliveryWithoutResending
        }
        CheckpointEventKind::SinkDeliveryObserved => {
            EffectReconciliation::RevalidateHistoricalSinkWithoutResending
        }
    }
}
