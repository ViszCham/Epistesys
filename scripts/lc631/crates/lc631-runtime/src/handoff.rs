//! Bounded TLCX-43 handoff inspection; no installation, receipt issuance or release grant.
#![forbid(unsafe_code)]

mod host_stages;
pub mod identity;
pub use host_stages::ExternalStage;

use identity::{compare_reported_identity, IdentityError, ReportedPackageIdentity};
use lc631_core::{stable_sha256, ArtifactId, TurnId};
use lc631_host::{HostError, HostOutputCandidate, HostOutputLifecycleState, HostSeedEnvelope};
use lc631_receipt_kernel::{ReceiptVerifier, ReplayGuard};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};

const MAX_SEED_BYTES: usize = 256 * 1024;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffError {
    Identity(IdentityError),
    SeedSize,
    OutputSize,
    SeedMismatch,
    OutputMismatch,
    ArtifactTurnMismatch,
    Host(HostError),
}

impl From<IdentityError> for HandoffError {
    fn from(error: IdentityError) -> Self {
        Self::Identity(error)
    }
}
impl From<HostError> for HandoffError {
    fn from(error: HostError) -> Self {
        Self::Host(error)
    }
}

/// Expected exact seed/output and package identity are caller-supplied comparison anchors.
pub struct HandoffExpectation {
    package: ReportedPackageIdentity,
    seed: Digest32,
    output: Digest32,
    artifact: ArtifactId,
    turn: TurnId,
}

fn digest(bytes: &[u8]) -> Digest32 {
    Digest32::from_bytes(Sha256::digest(bytes).into())
}

impl HandoffExpectation {
    pub fn new(
        package: ReportedPackageIdentity,
        seed: &str,
        output: &str,
        artifact: ArtifactId,
        turn: TurnId,
    ) -> Result<Self, HandoffError> {
        if seed.is_empty() || seed.len() > MAX_SEED_BYTES {
            return Err(HandoffError::SeedSize);
        }
        if output.is_empty() || output.len() > MAX_OUTPUT_BYTES {
            return Err(HandoffError::OutputSize);
        }
        Ok(Self {
            package,
            seed: digest(seed.as_bytes()),
            output: digest(output.as_bytes()),
            artifact,
            turn,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingInput {
    PackageIngressProvenance,
    FreshSessionProvenance,
    HostStage(HostOutputLifecycleState),
    DurableReplayConsumer,
    ResumeConsumer,
    RequiredEdgesAndGates,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HandoffDisposition {
    Pending(Vec<PendingInput>),
}

pub struct HandoffReport {
    pub reported_package_label: Digest32,
    pub host_stage_state: HostOutputLifecycleState,
    pub authenticated_stage_count: usize,
    pub disposition: HandoffDisposition,
}

/// A private ledger admits only current attested stage receipts from the existing host API.
/// Even delivered stages cannot certify fresh package ingress, replay, edges or release.
pub struct HandoffSession<'a> {
    package_label: Digest32,
    host: host_stages::HostStages<'a>,
}

impl<'a> HandoffSession<'a> {
    pub fn begin(
        expected: &HandoffExpectation,
        observed_package: &ReportedPackageIdentity,
        seed: &'a HostSeedEnvelope,
        candidate: &'a HostOutputCandidate,
    ) -> Result<Self, HandoffError> {
        compare_reported_identity(&expected.package, observed_package)?;
        if seed.artifact_id != expected.artifact
            || seed.turn_id != expected.turn
            || candidate.artifact_id != expected.artifact
            || candidate.turn_id != expected.turn
        {
            return Err(HandoffError::ArtifactTurnMismatch);
        }
        if seed.raw_user_message.len() > MAX_SEED_BYTES
            || digest(seed.raw_user_message.as_bytes()) != expected.seed
        {
            return Err(HandoffError::SeedMismatch);
        }
        if candidate.output_text.len() > MAX_OUTPUT_BYTES
            || digest(candidate.output_text.as_bytes()) != expected.output
            || candidate.candidate_digest != stable_sha256(&candidate.output_text)
        {
            return Err(HandoffError::OutputMismatch);
        }
        Ok(Self {
            package_label: observed_package.content_label(),
            host: host_stages::HostStages::new(seed, candidate)?,
        })
    }

    pub fn observe_external_stage(
        &mut self,
        input: ExternalStage<'_>,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<(), HandoffError> {
        self.host
            .observe(input, verifier, replay, now_epoch)
            .map_err(HandoffError::Host)
    }

    pub fn inspect(&self) -> HandoffReport {
        let state = self.host.state();
        let mut pending = vec![
            PendingInput::PackageIngressProvenance,
            PendingInput::FreshSessionProvenance,
            PendingInput::DurableReplayConsumer,
            PendingInput::ResumeConsumer,
            PendingInput::RequiredEdgesAndGates,
        ];
        if state != HostOutputLifecycleState::Delivered {
            pending.push(PendingInput::HostStage(state));
        }
        HandoffReport {
            reported_package_label: self.package_label,
            host_stage_state: state,
            authenticated_stage_count: self.host.receipt_count(),
            disposition: HandoffDisposition::Pending(pending),
        }
    }
}
