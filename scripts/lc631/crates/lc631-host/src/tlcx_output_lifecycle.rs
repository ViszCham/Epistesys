//! Exact rendered-output consumer for the existing authenticated stage contract.
//! Contract receipts do not attest real host callback provenance or delivery.
#![forbid(unsafe_code)]

use crate::{
    observe_host_output_stage_attested, HostError, HostOutputCandidate, HostOutputLifecycleState,
    HostOutputStage, HostOutputStageLedger, HostOutputStageReceipt, HostOutputStageRequest,
    HostSeedEnvelope,
};
use lc631_core::stable_sha256;
use lc631_receipt_kernel::{ReceiptVerifier, ReplayGuard, UntrustedReceipt};

/// No capability in the current source proves an actual installed host provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostIntegrationState {
    HostIntegrationPending,
}

/// Stage receipt replay prevention is distinct from durable replay observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableReplayState {
    Unobserved,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LifecycleError {
    OutputBudget,
    AllocationFailed,
    OutputMismatch,
    StageOrder,
    ClockRegression,
    Host(HostError),
}

/// An external callback's reported exact output and supplied signed attestation.
/// Neither this public DTO nor a nonempty payload creates an authenticated event.
pub struct CallbackObservation<'a> {
    pub stage: HostOutputStage,
    pub observed_output: &'a str,
    pub callback_payload: &'a str,
    pub sink_id: Option<&'a str>,
    pub delivered: Option<bool>,
    pub attestation: Option<UntrustedReceipt>,
}

/// One immutable candidate and one ordered ledger, borrowing one verified seed.
/// No issuer, filesystem writer, send action, or execution permit is held here.
/// The caller must connect the actual renderer and registered host callbacks.
pub struct ExactOutputLifecycle<'a> {
    seed: &'a HostSeedEnvelope,
    candidate: HostOutputCandidate,
    ledger: HostOutputStageLedger,
    last_observed_epoch: Option<u64>,
}

impl<'a> ExactOutputLifecycle<'a> {
    /// Captures exact UTF-8 once, with a limit checked before allocation.
    /// Seed freshness is rechecked by the existing verifier at every callback.
    pub fn new(
        seed: &'a HostSeedEnvelope,
        rendered_output: &str,
        max_output_bytes: usize,
    ) -> Result<Self, LifecycleError> {
        if max_output_bytes == 0
            || max_output_bytes > 65_536
            || rendered_output.is_empty()
            || rendered_output.len() > max_output_bytes
        {
            return Err(LifecycleError::OutputBudget);
        }
        if !seed.binding_intact() {
            return Err(LifecycleError::Host(HostError::HostSeedUnbound));
        }
        let mut output_text = String::new();
        output_text
            .try_reserve_exact(rendered_output.len())
            .map_err(|_| LifecycleError::AllocationFailed)?;
        output_text.push_str(rendered_output);
        let candidate = HostOutputCandidate {
            artifact_id: seed.artifact_id,
            turn_id: seed.turn_id,
            candidate_digest: stable_sha256(&output_text),
            output_text,
        };
        let mut ledger = HostOutputStageLedger::new(&candidate).map_err(LifecycleError::Host)?;
        // Reserve the complete three-stage container before consuming any nonce.
        ledger
            .stages
            .try_reserve_exact(3)
            .map_err(|_| LifecycleError::AllocationFailed)?;
        Ok(Self {
            seed,
            candidate,
            ledger,
            last_observed_epoch: None,
        })
    }

    pub fn exact_output(&self) -> &str {
        &self.candidate.output_text
    }

    /// Immutable access enables a registered producer to bind exact receipt input.
    pub fn candidate(&self) -> &HostOutputCandidate {
        &self.candidate
    }

    pub fn state(&self) -> HostOutputLifecycleState {
        self.ledger.state()
    }

    pub fn receipts(&self) -> &[HostOutputStageReceipt] {
        self.ledger.stages()
    }

    pub fn expected_stage(&self) -> Option<HostOutputStage> {
        match self.state() {
            HostOutputLifecycleState::AwaitingPreCommit => {
                Some(HostOutputStage::PreCommitCandidate)
            }
            HostOutputLifecycleState::AwaitingPostSend => Some(HostOutputStage::PostSendObserved),
            HostOutputLifecycleState::AwaitingSinkDelivery => {
                Some(HostOutputStage::SinkDeliveryObserved)
            }
            HostOutputLifecycleState::Delivered | HostOutputLifecycleState::DeliveryFailed => None,
        }
    }

    pub fn host_integration(&self) -> HostIntegrationState {
        HostIntegrationState::HostIntegrationPending
    }

    pub fn durable_replay(&self) -> DurableReplayState {
        DurableReplayState::Unobserved
    }

    /// Reject local ordering/byte/clock mismatches before verifier replay mutation.
    /// On verifier success the supplied nonce is consumed by ReplayGuard; that is
    /// an in-memory anti-replay effect, never a durable-replay or send observation.
    pub fn accept_callback(
        &mut self,
        callback: CallbackObservation<'_>,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<(), LifecycleError> {
        if self.expected_stage() != Some(callback.stage) {
            return Err(LifecycleError::StageOrder);
        }
        if callback.observed_output.as_bytes() != self.candidate.output_text.as_bytes() {
            return Err(LifecycleError::OutputMismatch);
        }
        if self
            .last_observed_epoch
            .is_some_and(|previous| now_epoch < previous)
        {
            return Err(LifecycleError::ClockRegression);
        }
        let receipt = observe_host_output_stage_attested(
            self.seed,
            HostOutputStageRequest {
                candidate: &self.candidate,
                stage: callback.stage,
                callback_payload: callback.callback_payload,
                sink_id: callback.sink_id,
                delivered: callback.delivered,
                attestation: callback.attestation,
            },
            verifier,
            replay,
            now_epoch,
        )
        .map_err(LifecycleError::Host)?;
        self.ledger.append(receipt).map_err(LifecycleError::Host)?;
        self.last_observed_epoch = Some(now_epoch);
        Ok(())
    }
}
