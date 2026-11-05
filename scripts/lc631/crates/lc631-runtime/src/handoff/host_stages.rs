//! Consume externally supplied stage attestations through existing host verification.
#![forbid(unsafe_code)]

use lc631_host::{
    observe_host_output_stage_attested, HostError, HostOutputCandidate, HostOutputLifecycleState,
    HostOutputStage, HostOutputStageLedger, HostOutputStageRequest, HostSeedEnvelope,
};
use lc631_receipt_kernel::{ReceiptVerifier, ReplayGuard, UntrustedReceipt};

pub struct ExternalStage<'a> {
    pub stage: HostOutputStage,
    pub callback_payload: &'a str,
    pub sink_id: Option<&'a str>,
    pub delivered: Option<bool>,
    pub attestation: Option<UntrustedReceipt>,
}

/// Borrows immutable exact inputs; this type never issues receipts or invokes a host.
pub(super) struct HostStages<'a> {
    seed: &'a HostSeedEnvelope,
    candidate: &'a HostOutputCandidate,
    ledger: HostOutputStageLedger,
    last_accepted_epoch: Option<u64>,
}

impl<'a> HostStages<'a> {
    pub(super) fn new(
        seed: &'a HostSeedEnvelope,
        candidate: &'a HostOutputCandidate,
    ) -> Result<Self, HostError> {
        Ok(Self {
            seed,
            candidate,
            ledger: HostOutputStageLedger::new(candidate)?,
            last_accepted_epoch: None,
        })
    }

    pub(super) fn observe(
        &mut self,
        input: ExternalStage<'_>,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<(), HostError> {
        // A later stage cannot revive an expired receipt by rolling the
        // caller-supplied clock backwards. This is only a session-local check;
        // it does not authenticate the clock or establish host provenance.
        if self
            .last_accepted_epoch
            .is_some_and(|last| now_epoch < last)
        {
            return Err(HostError::HostOutputStageConflict);
        }
        let expected = match self.ledger.state() {
            HostOutputLifecycleState::AwaitingPreCommit => HostOutputStage::PreCommitCandidate,
            HostOutputLifecycleState::AwaitingPostSend => HostOutputStage::PostSendObserved,
            HostOutputLifecycleState::AwaitingSinkDelivery => HostOutputStage::SinkDeliveryObserved,
            HostOutputLifecycleState::Delivered | HostOutputLifecycleState::DeliveryFailed => {
                return Err(HostError::HostOutputStageConflict);
            }
        };
        // Reject out-of-order material before consuming any replay nonce.
        if input.stage != expected {
            return Err(HostError::HostOutputStageConflict);
        }
        let receipt = observe_host_output_stage_attested(
            self.seed,
            HostOutputStageRequest {
                candidate: self.candidate,
                stage: input.stage,
                callback_payload: input.callback_payload,
                sink_id: input.sink_id,
                delivered: input.delivered,
                attestation: input.attestation,
            },
            verifier,
            replay,
            now_epoch,
        )?;
        self.ledger.append(receipt)?;
        self.last_accepted_epoch = Some(now_epoch);
        Ok(())
    }

    pub(super) fn state(&self) -> HostOutputLifecycleState {
        self.ledger.state()
    }
    pub(super) fn receipt_count(&self) -> usize {
        self.ledger.stages().len()
    }
}
