#![forbid(unsafe_code)]

mod stop_hook;

pub use stop_hook::{
    bind_codex_stop_hook, bind_codex_stop_hook_attested, persist_stop_hook_observation,
    CodexStopHookEvent, StopHookObservation,
};

use lc631_core::{
    authority_event_payload_digest, stable_sha256, Action, ArtifactId, AuthorityEvent,
    AuthorityRevision, AuthoritySourceKind, CallerOrigin, DeonticPolarity, ExecutionPermit,
    ExecutionPermitContext, PermitValidation, PrincipalBinding, SourceSpan, TurnId,
    VerifiedAuthorityEvent,
};
use lc631_receipt_kernel::{
    generate_key, ReceiptClass, ReceiptError, ReceiptIssuer, ReceiptPolicy, ReceiptScope,
    ReceiptVerifier, ReplayGuard, SubjectRevision, UntrustedReceipt,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::ops::Range;
use std::path::{Path, PathBuf};

pub const HOST_CONTRACT_SCHEMA: &str = "lc631-host-contract.v2";
pub const HOST_REPLAY_SCHEMA: &str = "lc631-host-replay.v2";
const LEGACY_HOST_REPLAY_SCHEMA: &str = "lc631-host-replay.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostBindingState {
    Unavailable,
    SelfAttested,
    VerifiedHostUser,
    Conflict,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostSeedEnvelope {
    pub schema_version: &'static str,
    pub artifact_id: ArtifactId,
    pub turn_id: TurnId,
    pub raw_user_message: String,
    pub user_span: SourceSpan,
    pub binding_state: HostBindingState,
    pub host_receipt_digest: Option<String>,
    pub authorized_resource_scope: Option<String>,
    verified_receipt_digest: Option<String>,
    verified_host_key_fingerprint: Option<String>,
    #[serde(skip)]
    verified_receipt_expiry: Option<u64>,
}

impl HostSeedEnvelope {
    pub fn verified(
        artifact_id: ArtifactId,
        turn_id: TurnId,
        raw_user_message: String,
        user_span: SourceSpan,
        _host_receipt: &str,
    ) -> Result<Self, HostError> {
        user_span
            .slice(&raw_user_message)
            .map_err(|_| HostError::InvalidUserSpan)?;
        let _ = (artifact_id, turn_id);
        Err(HostError::LegacyReceiptRejected)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_attested(
        artifact_id: ArtifactId,
        turn_id: TurnId,
        raw_user_message: String,
        user_span: SourceSpan,
        receipt: UntrustedReceipt,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<Self, HostError> {
        user_span
            .slice(&raw_user_message)
            .map_err(|_| HostError::InvalidUserSpan)?;
        let payload = host_seed_payload_digest(artifact_id, turn_id, &raw_user_message, user_span);
        let subject = SubjectRevision::checked(stable_sha256(&raw_user_message))
            .map_err(|_| HostError::ReceiptRejected)?;
        let scope =
            ReceiptScope::checked(format!("host/user-span/{}/{}", artifact_id.0, turn_id.0))
                .map_err(|_| HostError::ReceiptRejected)?;
        let verified = verifier
            .verify(
                receipt,
                &ReceiptPolicy::exact(ReceiptClass::HostSeed, subject, scope, payload, now_epoch),
                replay,
            )
            .map_err(map_receipt_error)?;
        let receipt_digest = verified.receipt_digest();
        let host_key_fingerprint = verifier.key_fingerprint().to_string();
        Ok(Self {
            schema_version: HOST_CONTRACT_SCHEMA,
            artifact_id,
            turn_id,
            raw_user_message,
            user_span,
            binding_state: HostBindingState::VerifiedHostUser,
            host_receipt_digest: Some(receipt_digest.clone()),
            authorized_resource_scope: None,
            verified_receipt_digest: Some(receipt_digest),
            verified_host_key_fingerprint: Some(host_key_fingerprint),
            verified_receipt_expiry: verified.expires_at_epoch(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_attested_scoped(
        artifact_id: ArtifactId,
        turn_id: TurnId,
        raw_user_message: String,
        user_span: SourceSpan,
        authorized_resource_scope: String,
        receipt: UntrustedReceipt,
        verifier: &ReceiptVerifier,
        replay: &mut ReplayGuard,
        now_epoch: u64,
    ) -> Result<Self, HostError> {
        user_span
            .slice(&raw_user_message)
            .map_err(|_| HostError::InvalidUserSpan)?;
        if authorized_resource_scope.trim().is_empty() || authorized_resource_scope.len() > 4096 {
            return Err(HostError::AuthorityScopeMismatch);
        }
        let payload = host_seed_scoped_payload_digest(
            artifact_id,
            turn_id,
            &raw_user_message,
            user_span,
            &authorized_resource_scope,
        );
        let subject = SubjectRevision::checked(stable_sha256(&raw_user_message))
            .map_err(|_| HostError::ReceiptRejected)?;
        let scope = ReceiptScope::checked(format!(
            "host/user-span/{}/{}/{}",
            artifact_id.0,
            turn_id.0,
            stable_sha256(&authorized_resource_scope)
        ))
        .map_err(|_| HostError::ReceiptRejected)?;
        let verified = verifier
            .verify(
                receipt,
                &ReceiptPolicy::exact(ReceiptClass::HostSeed, subject, scope, payload, now_epoch),
                replay,
            )
            .map_err(map_receipt_error)?;
        let receipt_digest = verified.receipt_digest();
        Ok(Self {
            schema_version: HOST_CONTRACT_SCHEMA,
            artifact_id,
            turn_id,
            raw_user_message,
            user_span,
            binding_state: HostBindingState::VerifiedHostUser,
            host_receipt_digest: Some(receipt_digest.clone()),
            authorized_resource_scope: Some(authorized_resource_scope),
            verified_receipt_digest: Some(receipt_digest),
            verified_host_key_fingerprint: Some(verifier.key_fingerprint().to_string()),
            verified_receipt_expiry: verified.expires_at_epoch(),
        })
    }

    pub fn self_attested(
        artifact_id: ArtifactId,
        turn_id: TurnId,
        raw_user_message: String,
    ) -> Self {
        let end = raw_user_message.len();
        Self {
            schema_version: HOST_CONTRACT_SCHEMA,
            artifact_id,
            turn_id,
            raw_user_message,
            user_span: SourceSpan { start: 0, end },
            binding_state: HostBindingState::SelfAttested,
            host_receipt_digest: None,
            authorized_resource_scope: None,
            verified_receipt_digest: None,
            verified_host_key_fingerprint: None,
            verified_receipt_expiry: None,
        }
    }

    pub fn may_source_grants(&self) -> bool {
        self.binding_state == HostBindingState::VerifiedHostUser
            && self.host_receipt_digest.is_some()
            && self.host_receipt_digest == self.verified_receipt_digest
            && self.verified_host_key_fingerprint.is_some()
            && self.authorized_resource_scope.is_some()
    }

    pub fn authorized_resource_scope(&self) -> Option<&str> {
        self.authorized_resource_scope.as_deref()
    }

    pub fn principal_binding(&self) -> Result<PrincipalBinding, HostError> {
        if !self.may_source_grants() {
            return Err(HostError::HostSeedUnbound);
        }
        PrincipalBinding::from_attested_seed(
            self.host_receipt_digest
                .as_deref()
                .ok_or(HostError::MissingHostReceipt)?,
            self.verified_host_key_fingerprint
                .as_deref()
                .ok_or(HostError::MissingHostReceipt)?,
        )
        .map_err(|_| HostError::ReceiptRejected)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostOutputCandidate {
    pub artifact_id: ArtifactId,
    pub turn_id: TurnId,
    pub candidate_digest: String,
    pub output_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostOutputReceipt {
    pub schema_version: &'static str,
    pub artifact_id: ArtifactId,
    pub turn_id: TurnId,
    pub candidate_digest: String,
    pub output_digest: String,
    pub host_callback_digest: String,
    pub exact_binding: bool,
    pub durable_replay_observed: bool,
    pub persistent_supervision_observed: bool,
    pub host_seed_receipt_digest: String,
    pub automatic_host_callback_observed: bool,
    pub binding_origin: HostOutputBindingOrigin,
    pub authenticity_receipt_digest: Option<String>,
    authenticity_attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostOutputStage {
    PreCommitCandidate,
    PostSendObserved,
    SinkDeliveryObserved,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostOutputStageRequest<'a> {
    pub candidate: &'a HostOutputCandidate,
    pub stage: HostOutputStage,
    pub callback_payload: &'a str,
    pub sink_id: Option<&'a str>,
    pub delivered: Option<bool>,
    pub attestation: Option<UntrustedReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct HostOutputStageReceipt {
    schema_version: &'static str,
    artifact_id: ArtifactId,
    turn_id: TurnId,
    candidate_digest: String,
    output_digest: String,
    stage: HostOutputStage,
    callback_event_digest: String,
    sink_id: Option<String>,
    delivered: Option<bool>,
    host_seed_receipt_digest: String,
    authenticity_receipt_digest: String,
    authenticity_attestation: UntrustedReceipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostOutputLifecycleState {
    AwaitingPreCommit,
    AwaitingPostSend,
    AwaitingSinkDelivery,
    Delivered,
    DeliveryFailed,
}

#[derive(Clone, Debug, Serialize)]
pub struct HostOutputStageLedger {
    artifact_id: ArtifactId,
    turn_id: TurnId,
    candidate_digest: String,
    output_digest: String,
    stages: Vec<HostOutputStageReceipt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostOutputBindingOrigin {
    ExternalPreCommitPayload,
    CodexStopHook,
    ClaudeStopHook,
}

pub fn bind_host_output(
    seed: &HostSeedEnvelope,
    candidate: &HostOutputCandidate,
    callback_payload: &str,
) -> Result<HostOutputReceipt, HostError> {
    let _ = (seed, candidate, callback_payload);
    Err(HostError::LegacyReceiptRejected)
}

pub fn host_seed_payload_digest(
    artifact_id: ArtifactId,
    turn_id: TurnId,
    raw_user_message: &str,
    user_span: SourceSpan,
) -> String {
    stable_sha256(&format!(
        "{HOST_CONTRACT_SCHEMA}\0{}\0{}\0{}\0{}\0{}",
        artifact_id.0,
        turn_id.0,
        stable_sha256(raw_user_message),
        user_span.start,
        user_span.end
    ))
}

pub fn host_seed_scoped_payload_digest(
    artifact_id: ArtifactId,
    turn_id: TurnId,
    raw_user_message: &str,
    user_span: SourceSpan,
    authorized_resource_scope: &str,
) -> String {
    stable_sha256(&format!(
        "{HOST_CONTRACT_SCHEMA}\0scoped-user-span.v1\0{}\0{}\0{}\0{}\0{}\0{}",
        artifact_id.0,
        turn_id.0,
        stable_sha256(raw_user_message),
        user_span.start,
        user_span.end,
        stable_sha256(authorized_resource_scope)
    ))
}

pub fn host_output_payload_digest(
    seed: &HostSeedEnvelope,
    candidate: &HostOutputCandidate,
    callback_payload: &str,
) -> Result<String, HostError> {
    if seed.binding_state != HostBindingState::VerifiedHostUser {
        return Err(HostError::HostSeedUnbound);
    }
    if seed.artifact_id != candidate.artifact_id || seed.turn_id != candidate.turn_id {
        return Err(HostError::ArtifactTurnMismatch);
    }
    let output_digest = stable_sha256(&candidate.output_text);
    if candidate.candidate_digest != output_digest {
        return Err(HostError::CandidateDigestMismatch);
    }
    if callback_payload.trim().is_empty() {
        return Err(HostError::MissingHostReceipt);
    }
    let seed_digest = seed
        .verified_receipt_digest
        .as_ref()
        .ok_or(HostError::MissingHostReceipt)?;
    Ok(host_output_receipt_basis_digest(
        candidate.artifact_id,
        candidate.turn_id,
        &candidate.candidate_digest,
        &output_digest,
        &stable_sha256(callback_payload),
        seed_digest,
        HostOutputBindingOrigin::ExternalPreCommitPayload,
    ))
}

pub fn host_output_stage_scope(
    artifact_id: ArtifactId,
    turn_id: TurnId,
    stage: HostOutputStage,
) -> Result<String, HostError> {
    Ok(format!(
        "host/output/{}/{}/stage/{}",
        artifact_id.0,
        turn_id.0,
        host_output_stage_name(stage)
    ))
}

pub fn host_output_stage_payload_digest(
    seed: &HostSeedEnvelope,
    request: &HostOutputStageRequest<'_>,
) -> Result<String, HostError> {
    let candidate = request.candidate;
    if candidate.artifact_id != seed.artifact_id || candidate.turn_id != seed.turn_id {
        return Err(HostError::ArtifactTurnMismatch);
    }
    let output_digest = stable_sha256(&candidate.output_text);
    if candidate.candidate_digest != output_digest {
        return Err(HostError::CandidateDigestMismatch);
    }
    let seed_digest = seed
        .verified_receipt_digest
        .as_ref()
        .ok_or(HostError::MissingHostReceipt)?;
    if request.callback_payload.trim().is_empty() || request.callback_payload.len() > 65_536 {
        return Err(HostError::InvalidHostOutputStage);
    }
    match request.stage {
        HostOutputStage::PreCommitCandidate | HostOutputStage::PostSendObserved
            if request.sink_id.is_some() || request.delivered.is_some() =>
        {
            return Err(HostError::InvalidHostOutputStage)
        }
        HostOutputStage::SinkDeliveryObserved
            if request.sink_id.is_none_or(|sink| {
                sink.trim().is_empty() || sink.len() > 512 || sink.chars().any(char::is_control)
            }) || request.delivered.is_none() =>
        {
            return Err(HostError::InvalidHostOutputStage)
        }
        _ => {}
    }
    let callback_event_digest = stable_sha256(request.callback_payload);
    let sink_id = request.sink_id.unwrap_or("no-sink");
    let delivered = request
        .delivered
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_applicable".into());
    Ok(stable_sha256(&format!(
        "{HOST_CONTRACT_SCHEMA}\0host-output-stage.v1\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}\0{}",
        candidate.artifact_id.0,
        candidate.turn_id.0,
        host_output_stage_name(request.stage),
        candidate.candidate_digest,
        output_digest,
        callback_event_digest,
        sink_id,
        delivered,
        seed_digest,
    )))
}

pub fn observe_host_output_stage_attested(
    seed: &HostSeedEnvelope,
    request: HostOutputStageRequest<'_>,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<HostOutputStageReceipt, HostError> {
    let payload = host_output_stage_payload_digest(seed, &request)?;
    let wire = request.attestation.ok_or(HostError::MissingHostReceipt)?;
    let candidate = request.candidate;
    let output_digest = stable_sha256(&candidate.output_text);
    let subject = output_subject(candidate.artifact_id, candidate.turn_id, &output_digest)?;
    let scope = ReceiptScope::checked(host_output_stage_scope(
        candidate.artifact_id,
        candidate.turn_id,
        request.stage,
    )?)
    .map_err(|_| HostError::InvalidHostOutputStage)?;
    let verified = verifier
        .verify(
            wire,
            &ReceiptPolicy::exact(ReceiptClass::HostOutput, subject, scope, payload, now_epoch),
            replay,
        )
        .map_err(map_receipt_error)?;
    let receipt_digest = verified.receipt_digest();
    let callback_event_digest = stable_sha256(request.callback_payload);
    Ok(HostOutputStageReceipt {
        schema_version: HOST_CONTRACT_SCHEMA,
        artifact_id: candidate.artifact_id,
        turn_id: candidate.turn_id,
        candidate_digest: candidate.candidate_digest.clone(),
        output_digest,
        stage: request.stage,
        callback_event_digest,
        sink_id: request.sink_id.map(str::to_owned),
        delivered: request.delivered,
        host_seed_receipt_digest: seed
            .host_receipt_digest
            .clone()
            .ok_or(HostError::MissingHostReceipt)?,
        authenticity_receipt_digest: receipt_digest,
        authenticity_attestation: verified.into_untrusted(),
    })
}

impl HostOutputStageLedger {
    pub fn new(candidate: &HostOutputCandidate) -> Result<Self, HostError> {
        if candidate.output_text.is_empty()
            || stable_sha256(&candidate.output_text) != candidate.candidate_digest
        {
            return Err(HostError::CandidateDigestMismatch);
        }
        Ok(Self {
            artifact_id: candidate.artifact_id,
            turn_id: candidate.turn_id,
            candidate_digest: candidate.candidate_digest.clone(),
            output_digest: stable_sha256(&candidate.output_text),
            stages: Vec::new(),
        })
    }

    pub fn append(&mut self, receipt: HostOutputStageReceipt) -> Result<(), HostError> {
        if receipt.artifact_id != self.artifact_id
            || receipt.turn_id != self.turn_id
            || receipt.candidate_digest != self.candidate_digest
            || receipt.output_digest != self.output_digest
        {
            return Err(HostError::ArtifactTurnMismatch);
        }
        let expected = match self.stages.len() {
            0 => HostOutputStage::PreCommitCandidate,
            1 => HostOutputStage::PostSendObserved,
            2 => HostOutputStage::SinkDeliveryObserved,
            _ => return Err(HostError::HostOutputStageConflict),
        };
        if receipt.stage != expected {
            return Err(HostError::HostOutputStageConflict);
        }
        self.stages.push(receipt);
        Ok(())
    }

    pub fn state(&self) -> HostOutputLifecycleState {
        match self.stages.last() {
            None => HostOutputLifecycleState::AwaitingPreCommit,
            Some(receipt) if receipt.stage == HostOutputStage::PreCommitCandidate => {
                HostOutputLifecycleState::AwaitingPostSend
            }
            Some(receipt) if receipt.stage == HostOutputStage::PostSendObserved => {
                HostOutputLifecycleState::AwaitingSinkDelivery
            }
            Some(receipt) if receipt.delivered == Some(true) => HostOutputLifecycleState::Delivered,
            Some(_) => HostOutputLifecycleState::DeliveryFailed,
        }
    }

    pub fn stages(&self) -> &[HostOutputStageReceipt] {
        &self.stages
    }
}

impl HostOutputStageReceipt {
    pub fn stage(&self) -> HostOutputStage {
        self.stage
    }

    pub fn delivered(&self) -> Option<bool> {
        self.delivered
    }

    pub fn authenticity_receipt_digest(&self) -> &str {
        &self.authenticity_receipt_digest
    }

    pub fn authenticity_attestation(&self) -> &UntrustedReceipt {
        &self.authenticity_attestation
    }
}

fn host_output_stage_name(stage: HostOutputStage) -> &'static str {
    match stage {
        HostOutputStage::PreCommitCandidate => "precommit_candidate",
        HostOutputStage::PostSendObserved => "post_send_observed",
        HostOutputStage::SinkDeliveryObserved => "sink_delivery_observed",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn bind_host_output_attested(
    seed: &HostSeedEnvelope,
    candidate: &HostOutputCandidate,
    callback_payload: &str,
    receipt: Option<UntrustedReceipt>,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<HostOutputReceipt, HostError> {
    let payload = host_output_payload_digest(seed, candidate, callback_payload)?;
    let receipt = receipt.ok_or(HostError::MissingHostReceipt)?;
    let output_digest = stable_sha256(&candidate.output_text);
    let subject = output_subject(candidate.artifact_id, candidate.turn_id, &output_digest)?;
    let scope = output_scope(candidate.artifact_id, candidate.turn_id)?;
    let verified = verifier
        .verify(
            receipt,
            &ReceiptPolicy::exact(ReceiptClass::HostOutput, subject, scope, payload, now_epoch),
            replay,
        )
        .map_err(map_receipt_error)?;
    let receipt_digest = verified.receipt_digest();
    let wire = verified.into_untrusted();
    Ok(HostOutputReceipt {
        schema_version: HOST_CONTRACT_SCHEMA,
        artifact_id: candidate.artifact_id,
        turn_id: candidate.turn_id,
        candidate_digest: candidate.candidate_digest.clone(),
        output_digest,
        host_callback_digest: stable_sha256(callback_payload),
        exact_binding: true,
        durable_replay_observed: false,
        persistent_supervision_observed: false,
        host_seed_receipt_digest: seed
            .host_receipt_digest
            .clone()
            .ok_or(HostError::MissingHostReceipt)?,
        automatic_host_callback_observed: false,
        binding_origin: HostOutputBindingOrigin::ExternalPreCommitPayload,
        authenticity_receipt_digest: Some(receipt_digest),
        authenticity_attestation: Some(wire),
    })
}

impl HostOutputReceipt {
    pub fn legacy_fixture_for_test(
        artifact_id: ArtifactId,
        turn_id: TurnId,
        output_digest: String,
    ) -> Self {
        Self {
            schema_version: HOST_CONTRACT_SCHEMA,
            artifact_id,
            turn_id,
            candidate_digest: output_digest.clone(),
            output_digest,
            host_callback_digest: stable_sha256("legacy-callback"),
            exact_binding: true,
            durable_replay_observed: false,
            persistent_supervision_observed: false,
            host_seed_receipt_digest: stable_sha256("legacy-seed"),
            automatic_host_callback_observed: false,
            binding_origin: HostOutputBindingOrigin::ExternalPreCommitPayload,
            authenticity_receipt_digest: None,
            authenticity_attestation: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DurableReplayReceipt {
    pub schema_version: &'static str,
    pub ledger_path: PathBuf,
    pub sequence: u64,
    pub entry_sha256: String,
    pub chain_head_sha256: String,
    pub entry_count: usize,
    pub chain_verified: bool,
    pub exact_output_binding_preserved: bool,
    pub durable_replay_observed: bool,
    pub automatic_host_callback_observed: bool,
    pub authenticity_verified: bool,
    pub authenticity_receipt_digest: String,
    pub claim_boundary: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
struct ReplayEntry {
    schema_version: String,
    sequence: u64,
    previous_entry_sha256: String,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: String,
    output_digest: String,
    host_callback_digest: String,
    host_seed_receipt_digest: String,
    #[serde(default)]
    authenticity_receipt_digest: String,
    entry_sha256: String,
}

#[derive(Serialize)]
struct ReplayEntryBasisV1<'a> {
    schema_version: &'a str,
    sequence: u64,
    previous_entry_sha256: &'a str,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: &'a str,
    output_digest: &'a str,
    host_callback_digest: &'a str,
    host_seed_receipt_digest: &'a str,
}

#[derive(Serialize)]
struct ReplayEntryBasisV2<'a> {
    schema_version: &'a str,
    sequence: u64,
    previous_entry_sha256: &'a str,
    artifact_id: u64,
    turn_id: u64,
    candidate_digest: &'a str,
    output_digest: &'a str,
    host_callback_digest: &'a str,
    host_seed_receipt_digest: &'a str,
    authenticity_receipt_digest: &'a str,
}

pub fn append_durable_replay(
    owned_root: &Path,
    ledger_path: &Path,
    receipt: &HostOutputReceipt,
) -> Result<DurableReplayReceipt, HostError> {
    let _ = (owned_root, ledger_path, receipt);
    Err(HostError::LegacyReceiptRejected)
}

pub fn append_durable_replay_verified(
    owned_root: &Path,
    ledger_path: &Path,
    receipt: &HostOutputReceipt,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<DurableReplayReceipt, HostError> {
    if !receipt.exact_binding {
        return Err(HostError::ReplayRequiresExactBinding);
    }
    let attestation = receipt
        .authenticity_attestation
        .clone()
        .ok_or(HostError::LegacyReceiptRejected)?;
    let expected_digest = receipt
        .authenticity_receipt_digest
        .as_ref()
        .ok_or(HostError::LegacyReceiptRejected)?;
    let payload = host_output_receipt_basis_digest(
        receipt.artifact_id,
        receipt.turn_id,
        &receipt.candidate_digest,
        &receipt.output_digest,
        &receipt.host_callback_digest,
        &receipt.host_seed_receipt_digest,
        receipt.binding_origin,
    );
    let verified = verifier
        .verify(
            attestation,
            &ReceiptPolicy::exact(
                ReceiptClass::HostOutput,
                output_subject(receipt.artifact_id, receipt.turn_id, &receipt.output_digest)?,
                output_scope(receipt.artifact_id, receipt.turn_id)?,
                payload,
                now_epoch,
            ),
            replay,
        )
        .map_err(map_receipt_error)?;
    if &verified.receipt_digest() != expected_digest {
        return Err(HostError::ReceiptRejected);
    }
    append_durable_replay_inner(owned_root, ledger_path, receipt, expected_digest)
}

fn append_durable_replay_inner(
    owned_root: &Path,
    ledger_path: &Path,
    receipt: &HostOutputReceipt,
    authenticity_receipt_digest: &str,
) -> Result<DurableReplayReceipt, HostError> {
    fs::create_dir_all(owned_root).map_err(|error| HostError::Io(error.to_string()))?;
    let canonical_root = owned_root
        .canonicalize()
        .map_err(|error| HostError::Io(error.to_string()))?;
    let parent = ledger_path
        .parent()
        .ok_or(HostError::ReplayPathOutsideRoot)?;
    fs::create_dir_all(parent).map_err(|error| HostError::Io(error.to_string()))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| HostError::Io(error.to_string()))?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(HostError::ReplayPathOutsideRoot);
    }
    if let Ok(metadata) = fs::symlink_metadata(ledger_path) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(HostError::ReplayPathUnsafe);
        }
    }
    let lock_path = ledger_path.with_extension("lc631.lock");
    let lock = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|_| HostError::ReplayBusy)?;
    let _lock = ReplayLock {
        path: lock_path,
        _file: lock,
    };

    let mut entries = read_and_verify_ledger(ledger_path)?;
    if entries.iter().any(|entry| {
        !entry.authenticity_receipt_digest.is_empty()
            && entry.authenticity_receipt_digest == authenticity_receipt_digest
    }) {
        return Err(HostError::ReceiptReplay);
    }
    let sequence = u64::try_from(entries.len())
        .map_err(|_| HostError::ReplaySequenceExhausted)?
        .checked_add(1)
        .ok_or(HostError::ReplaySequenceExhausted)?;
    let previous = entries
        .last()
        .map(|entry| entry.entry_sha256.clone())
        .unwrap_or_else(|| stable_sha256("lc631-host-replay-genesis.v2"));
    let basis = ReplayEntryBasisV2 {
        schema_version: HOST_REPLAY_SCHEMA,
        sequence,
        previous_entry_sha256: &previous,
        artifact_id: receipt.artifact_id.0,
        turn_id: receipt.turn_id.0,
        candidate_digest: &receipt.candidate_digest,
        output_digest: &receipt.output_digest,
        host_callback_digest: &receipt.host_callback_digest,
        host_seed_receipt_digest: &receipt.host_seed_receipt_digest,
        authenticity_receipt_digest,
    };
    let canonical = serde_json::to_string(&basis)
        .map_err(|error| HostError::InvalidReplay(error.to_string()))?;
    let entry_sha256 = stable_sha256(&canonical);
    let entry = ReplayEntry {
        schema_version: HOST_REPLAY_SCHEMA.into(),
        sequence,
        previous_entry_sha256: previous,
        artifact_id: receipt.artifact_id.0,
        turn_id: receipt.turn_id.0,
        candidate_digest: receipt.candidate_digest.clone(),
        output_digest: receipt.output_digest.clone(),
        host_callback_digest: receipt.host_callback_digest.clone(),
        host_seed_receipt_digest: receipt.host_seed_receipt_digest.clone(),
        authenticity_receipt_digest: authenticity_receipt_digest.to_string(),
        entry_sha256: entry_sha256.clone(),
    };
    let mut encoded =
        serde_json::to_vec(&entry).map_err(|error| HostError::InvalidReplay(error.to_string()))?;
    encoded.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(ledger_path)
        .map_err(|error| HostError::Io(error.to_string()))?;
    file.write_all(&encoded)
        .map_err(|error| HostError::Io(error.to_string()))?;
    file.sync_all()
        .map_err(|error| HostError::Io(error.to_string()))?;
    entries = read_and_verify_ledger(ledger_path)?;
    let head = entries
        .last()
        .ok_or_else(|| HostError::InvalidReplay("ledger_empty_after_append".into()))?;
    if head.entry_sha256 != entry_sha256 {
        return Err(HostError::InvalidReplay("post_append_head_mismatch".into()));
    }
    Ok(DurableReplayReceipt {
        schema_version: HOST_REPLAY_SCHEMA,
        ledger_path: ledger_path.to_path_buf(),
        sequence,
        entry_sha256: entry_sha256.clone(),
        chain_head_sha256: entry_sha256,
        entry_count: entries.len(),
        chain_verified: true,
        exact_output_binding_preserved: receipt.exact_binding,
        durable_replay_observed: true,
        automatic_host_callback_observed: receipt.automatic_host_callback_observed,
        authenticity_verified: true,
        authenticity_receipt_digest: authenticity_receipt_digest.to_string(),
        claim_boundary: "durable replay preserves a locally verified, hash-chained candidate/output attestation; same-user key custody, remote delivery, and semantic truth remain separate",
    })
}

pub fn verify_durable_replay(ledger_path: &Path) -> Result<usize, HostError> {
    read_and_verify_ledger(ledger_path).map(|entries| entries.len())
}

fn read_and_verify_ledger(path: &Path) -> Result<Vec<ReplayEntry>, HostError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|error| HostError::Io(error.to_string()))?;
    let mut entries = Vec::new();
    let mut previous: Option<String> = None;
    for (index, line) in raw.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: ReplayEntry = serde_json::from_str(line)
            .map_err(|error| HostError::InvalidReplay(error.to_string()))?;
        let expected_sequence =
            u64::try_from(index + 1).map_err(|_| HostError::ReplaySequenceExhausted)?;
        if !matches!(
            entry.schema_version.as_str(),
            HOST_REPLAY_SCHEMA | LEGACY_HOST_REPLAY_SCHEMA
        ) || entry.sequence != expected_sequence
        {
            return Err(HostError::InvalidReplay(
                "sequence_or_previous_hash_mismatch".into(),
            ));
        }
        let expected_previous = previous.clone().unwrap_or_else(|| {
            if entry.schema_version == LEGACY_HOST_REPLAY_SCHEMA {
                stable_sha256("lc631-host-replay-genesis.v1")
            } else {
                stable_sha256("lc631-host-replay-genesis.v2")
            }
        });
        if entry.previous_entry_sha256 != expected_previous {
            return Err(HostError::InvalidReplay(
                "sequence_or_previous_hash_mismatch".into(),
            ));
        }
        let canonical = if entry.schema_version == LEGACY_HOST_REPLAY_SCHEMA {
            serde_json::to_string(&ReplayEntryBasisV1 {
                schema_version: &entry.schema_version,
                sequence: entry.sequence,
                previous_entry_sha256: &entry.previous_entry_sha256,
                artifact_id: entry.artifact_id,
                turn_id: entry.turn_id,
                candidate_digest: &entry.candidate_digest,
                output_digest: &entry.output_digest,
                host_callback_digest: &entry.host_callback_digest,
                host_seed_receipt_digest: &entry.host_seed_receipt_digest,
            })
        } else {
            serde_json::to_string(&ReplayEntryBasisV2 {
                schema_version: &entry.schema_version,
                sequence: entry.sequence,
                previous_entry_sha256: &entry.previous_entry_sha256,
                artifact_id: entry.artifact_id,
                turn_id: entry.turn_id,
                candidate_digest: &entry.candidate_digest,
                output_digest: &entry.output_digest,
                host_callback_digest: &entry.host_callback_digest,
                host_seed_receipt_digest: &entry.host_seed_receipt_digest,
                authenticity_receipt_digest: &entry.authenticity_receipt_digest,
            })
        };
        let canonical = canonical.map_err(|error| HostError::InvalidReplay(error.to_string()))?;
        let expected_hash = stable_sha256(&canonical);
        if entry.entry_sha256 != expected_hash {
            return Err(HostError::InvalidReplay("entry_hash_mismatch".into()));
        }
        previous = Some(entry.entry_sha256.clone());
        entries.push(entry);
    }
    Ok(entries)
}

pub(crate) fn host_output_receipt_basis_digest(
    artifact_id: ArtifactId,
    turn_id: TurnId,
    candidate_digest: &str,
    output_digest: &str,
    callback_digest: &str,
    seed_receipt_digest: &str,
    origin: HostOutputBindingOrigin,
) -> String {
    stable_sha256(&format!(
        "{HOST_CONTRACT_SCHEMA}\0{}\0{}\0{}\0{}\0{}\0{}\0{:?}",
        artifact_id.0,
        turn_id.0,
        candidate_digest,
        output_digest,
        callback_digest,
        seed_receipt_digest,
        origin
    ))
}

pub(crate) fn output_subject(
    artifact_id: ArtifactId,
    turn_id: TurnId,
    output_digest: &str,
) -> Result<SubjectRevision, HostError> {
    SubjectRevision::checked(stable_sha256(&format!(
        "{}:{}:{}",
        artifact_id.0, turn_id.0, output_digest
    )))
    .map_err(|_| HostError::ReceiptRejected)
}

pub(crate) fn output_scope(
    artifact_id: ArtifactId,
    turn_id: TurnId,
) -> Result<ReceiptScope, HostError> {
    ReceiptScope::checked(format!("host/output/{}/{}", artifact_id.0, turn_id.0))
        .map_err(|_| HostError::ReceiptRejected)
}

fn map_receipt_error(error: ReceiptError) -> HostError {
    if error == ReceiptError::ReplayDetected {
        HostError::ReceiptReplay
    } else {
        HostError::ReceiptRejected
    }
}

#[derive(Debug)]
pub struct HostReceiptContext {
    issuer: ReceiptIssuer,
    verifier: ReceiptVerifier,
    key_path: PathBuf,
}

pub struct ExecutionPermitValidationRequest<'a> {
    pub permit: &'a ExecutionPermit,
    pub action: Action,
    pub scope: &'a str,
    pub source_revision: &'a str,
    pub authority_revision: AuthorityRevision,
    pub now_epoch: u64,
    pub revocation_revision: u64,
    pub revoked_permit_digests: &'a [String],
}

pub struct HostExecutionPermitAdmission {
    pub action: Action,
    pub scope: String,
    pub span: SourceSpan,
    pub authority_revision: AuthorityRevision,
    pub revocation_revision: u64,
    pub now_epoch: u64,
    pub attestation: UntrustedReceipt,
}

impl HostReceiptContext {
    /// Consume an externally issued authority event. This reader never signs a
    /// Grant from parsed text, a seed envelope, a model output, or a public report.
    pub fn admit_execution_permit(
        &self,
        seed: &HostSeedEnvelope,
        admission: HostExecutionPermitAdmission,
        replay: &mut ReplayGuard,
    ) -> Result<ExecutionPermit, HostError> {
        if !seed.may_source_grants()
            || seed.verified_host_key_fingerprint.as_deref()
                != Some(self.verifier.key_fingerprint())
            || seed.authorized_resource_scope.as_deref() != Some(admission.scope.as_str())
        {
            return Err(HostError::HostSeedUnbound);
        }
        if admission.span.start < seed.user_span.start
            || admission.span.end > seed.user_span.end
            || admission.span.slice(&seed.raw_user_message).is_err()
            || !user_span_is_plain_prose(&seed.raw_user_message, admission.span)
        {
            return Err(HostError::AuthoritySourceIneligible);
        }
        let event = AuthorityEvent {
            action: admission.action,
            source: AuthoritySourceKind::VerifiedHostUserSpan,
            polarity: DeonticPolarity::Grant,
            span: admission.span,
            scope: admission.scope,
        };
        let parent = seed
            .verified_receipt_digest
            .as_deref()
            .ok_or(HostError::MissingHostReceipt)?;
        let expiry = admission
            .attestation
            .claims()
            .expires_at_epoch()
            .ok_or(HostError::AuthorityPermitRejected)?
            .min(
                seed.verified_receipt_expiry
                    .ok_or(HostError::AuthorityPermitRejected)?,
            );
        let verified = self
            .verifier
            .verify(
                admission.attestation,
                &ReceiptPolicy::exact(
                    ReceiptClass::Authority,
                    SubjectRevision::checked(stable_sha256(&seed.raw_user_message))
                        .map_err(|_| HostError::ReceiptRejected)?,
                    ReceiptScope::checked(format!(
                        "authority/{}/{}",
                        host_action_name(event.action),
                        event.scope
                    ))
                    .map_err(|_| HostError::ReceiptRejected)?,
                    authority_event_payload_digest(&event),
                    admission.now_epoch,
                )
                .with_parent(parent),
                replay,
            )
            .map_err(map_receipt_error)?;
        ExecutionPermit::from_verified_user_event(
            VerifiedAuthorityEvent::checked(event, verified)
                .map_err(|_| HostError::AuthorityPermitRejected)?,
            &seed.raw_user_message,
            admission.authority_revision,
            admission.now_epoch,
            expiry.min(admission.now_epoch.saturating_add(300)),
            admission.revocation_revision,
        )
        .map_err(|_| HostError::AuthorityPermitRejected)
    }
    #[cfg(test)]
    pub fn from_key_bytes(key: &[u8]) -> Result<Self, HostError> {
        Ok(Self {
            issuer: ReceiptIssuer::from_key_bytes("lc631-host-root", key)
                .map_err(|_| HostError::ReceiptRejected)?,
            verifier: ReceiptVerifier::from_key_bytes("lc631-host-root", key)
                .map_err(|_| HostError::ReceiptRejected)?,
            key_path: PathBuf::from("in-memory-host-root"),
        })
    }

    pub fn load_or_create(owned_root: &Path) -> Result<Self, HostError> {
        fs::create_dir_all(owned_root).map_err(|error| HostError::Io(error.to_string()))?;
        let root = owned_root
            .canonicalize()
            .map_err(|error| HostError::Io(error.to_string()))?;
        let key_path = root.join("receipt-root-v1.key");
        if let Ok(metadata) = fs::symlink_metadata(&key_path) {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(HostError::ReceiptKeyPathUnsafe);
            }
        }
        let key = if key_path.is_file() {
            read_key(&key_path)?
        } else {
            let key = generate_key().map_err(|_| HostError::ReceiptRejected)?;
            match create_key_file(&key_path, &key) {
                Ok(()) => key,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    read_key(&key_path)?
                }
                Err(error) => return Err(HostError::Io(error.to_string())),
            }
        };
        Ok(Self {
            issuer: ReceiptIssuer::from_key_bytes("lc631-host-root", &key)
                .map_err(|_| HostError::ReceiptRejected)?,
            verifier: ReceiptVerifier::from_key_bytes("lc631-host-root", &key)
                .map_err(|_| HostError::ReceiptRejected)?,
            key_path,
        })
    }

    pub fn load_existing(owned_root: &Path) -> Result<Self, HostError> {
        let metadata =
            fs::symlink_metadata(owned_root).map_err(|error| HostError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(HostError::ReceiptKeyPathUnsafe);
        }
        let root = owned_root
            .canonicalize()
            .map_err(|error| HostError::Io(error.to_string()))?;
        let key_path = root.join("receipt-root-v1.key");
        let metadata =
            fs::symlink_metadata(&key_path).map_err(|error| HostError::Io(error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(HostError::ReceiptKeyPathUnsafe);
        }
        let key = read_key(&key_path)?;
        Ok(Self {
            issuer: ReceiptIssuer::from_key_bytes("lc631-host-root", &key)
                .map_err(|_| HostError::ReceiptRejected)?,
            verifier: ReceiptVerifier::from_key_bytes("lc631-host-root", &key)
                .map_err(|_| HostError::ReceiptRejected)?,
            key_path,
        })
    }

    pub fn issuer(&self) -> &ReceiptIssuer {
        &self.issuer
    }

    pub fn verifier(&self) -> &ReceiptVerifier {
        &self.verifier
    }

    pub fn key_path(&self) -> &Path {
        &self.key_path
    }

    pub fn key_fingerprint(&self) -> String {
        self.verifier.key_fingerprint().to_string()
    }

    pub fn attest_authority_event(
        &self,
        seed: &HostSeedEnvelope,
        event: AuthorityEvent,
        replay: &mut ReplayGuard,
        now_epoch: u64,
        expires_at_epoch: u64,
    ) -> Result<VerifiedAuthorityEvent, HostError> {
        if !seed.may_source_grants() {
            return Err(HostError::HostSeedUnbound);
        }
        if seed.verified_host_key_fingerprint.as_deref() != Some(self.verifier.key_fingerprint()) {
            return Err(HostError::AuthorityHostMismatch);
        }
        if event.source != AuthoritySourceKind::VerifiedHostUserSpan
            || !matches!(
                event.polarity,
                DeonticPolarity::Grant | DeonticPolarity::Deny
            )
            || event.scope.is_empty()
            || event.scope.len() > 4096
            || event.span.start < seed.user_span.start
            || event.span.end > seed.user_span.end
            || event.span.slice(&seed.raw_user_message).is_err()
        {
            return Err(HostError::AuthoritySpanInvalid);
        }
        if seed.authorized_resource_scope.as_deref() != Some(event.scope.as_str()) {
            return Err(HostError::AuthorityScopeMismatch);
        }
        let text = event
            .span
            .slice(&seed.raw_user_message)
            .map_err(|_| HostError::AuthoritySpanInvalid)?;
        if !user_span_is_plain_prose(&seed.raw_user_message, event.span)
            || !authority_surface_matches(event.action, event.polarity, text)
        {
            return Err(HostError::AuthoritySourceIneligible);
        }
        if expires_at_epoch <= now_epoch || expires_at_epoch.saturating_sub(now_epoch) > 86_400 {
            return Err(HostError::AuthorityPermitRejected);
        }
        let seed_receipt_digest = seed
            .verified_receipt_digest
            .as_deref()
            .ok_or(HostError::MissingHostReceipt)?;
        let subject = SubjectRevision::checked(stable_sha256(&seed.raw_user_message))
            .map_err(|_| HostError::ReceiptRejected)?;
        let scope = ReceiptScope::checked(format!(
            "authority/{}/{}",
            host_action_name(event.action),
            event.scope
        ))
        .map_err(|_| HostError::ReceiptRejected)?;
        let payload = authority_event_payload_digest(&event);
        let wire = self
            .issuer
            .issue(
                ReceiptClass::Authority,
                subject.clone(),
                scope.clone(),
                payload.clone(),
                now_epoch,
                Some(expires_at_epoch),
                Some(seed_receipt_digest.to_string()),
            )
            .map_err(|_| HostError::ReceiptRejected)?;
        let verified = self
            .verifier
            .verify(
                wire,
                &ReceiptPolicy::exact(ReceiptClass::Authority, subject, scope, payload, now_epoch)
                    .with_parent(seed_receipt_digest),
                replay,
            )
            .map_err(map_receipt_error)?;
        VerifiedAuthorityEvent::checked(event, verified).map_err(|_| HostError::ReceiptRejected)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn issue_execution_permit(
        &self,
        seed: &HostSeedEnvelope,
        action: Action,
        polarity: DeonticPolarity,
        span: SourceSpan,
        scope: String,
        authority_revision: AuthorityRevision,
        now_epoch: u64,
        expires_at_epoch: u64,
        revocation_revision: u64,
        replay: &mut ReplayGuard,
    ) -> Result<ExecutionPermit, HostError> {
        let event = AuthorityEvent {
            action,
            source: AuthoritySourceKind::VerifiedHostUserSpan,
            polarity,
            span,
            scope,
        };
        let verified =
            self.attest_authority_event(seed, event, replay, now_epoch, expires_at_epoch)?;
        ExecutionPermit::from_verified_user_event(
            verified,
            &seed.raw_user_message,
            authority_revision,
            now_epoch,
            expires_at_epoch,
            revocation_revision,
        )
        .map_err(|_| HostError::AuthorityPermitRejected)
    }

    pub fn validate_execution_permit(
        &self,
        seed: &HostSeedEnvelope,
        request: &ExecutionPermitValidationRequest<'_>,
    ) -> PermitValidation {
        if !seed.may_source_grants()
            || seed.verified_host_key_fingerprint.as_deref()
                != Some(self.verifier.key_fingerprint())
        {
            return PermitValidation::Rejected(lc631_core::PermitRejection::UntrustedCallerOrigin);
        }
        let Ok(principal) = seed.principal_binding() else {
            return PermitValidation::Rejected(
                lc631_core::PermitRejection::PrincipalBindingInvalid,
            );
        };
        request.permit.validate(&ExecutionPermitContext {
            principal: &principal,
            action: request.action,
            scope: request.scope,
            source_revision: request.source_revision,
            authority_revision: request.authority_revision,
            now_epoch: request.now_epoch,
            revocation_revision: request.revocation_revision,
            revoked_permit_digests: request.revoked_permit_digests,
            caller_origin: CallerOrigin::HostVerifiedUser,
            trusted_host_fingerprint: self.verifier.key_fingerprint(),
        })
    }
}

fn authority_surface_matches(action: Action, polarity: DeonticPolarity, surface: &str) -> bool {
    let lower = surface.to_lowercase();
    let verbs = authority_action_verbs(action);
    let negative = [
        "do not",
        "must not",
        "should not",
        "never",
        "don't",
        "禁止",
        "しないで",
        "してはいけない",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    let conditional = [
        "if ",
        "only if",
        "unless ",
        "until ",
        "場合",
        "ない限り",
        "まで",
        "ただし",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if conditional {
        return false;
    }
    let words = lower
        .split_whitespace()
        .map(|word| {
            word.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '_'
            })
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    match polarity {
        DeonticPolarity::Deny => {
            let explicit_english_deny = [
                ("do", "not"),
                ("must", "not"),
                ("should", "not"),
                ("don't", ""),
                ("never", ""),
            ]
            .iter()
            .any(|(first, second)| {
                words.first() == Some(first)
                    && (second.is_empty() || words.get(1) == Some(second))
                    && words
                        .get(if second.is_empty() { 1 } else { 2 })
                        .is_some_and(|verb| verbs.contains(verb))
            });
            explicit_english_deny || japanese_authority_surface(action, polarity, surface)
        }
        DeonticPolarity::Grant => {
            if negative || words.is_empty() {
                return false;
            }
            let first_action = verbs.contains(&words[0]);
            let prefixed_action = matches!(words.first(), Some(&"please" | &"must" | &"should"))
                && words.get(1).is_some_and(|word| verbs.contains(word));
            first_action || prefixed_action || japanese_authority_surface(action, polarity, surface)
        }
        _ => false,
    }
}

fn authority_action_verbs(action: Action) -> &'static [&'static str] {
    match action {
        Action::Edit => &["edit", "modify", "change", "update"],
        Action::Test => &["test", "tests", "verify", "check"],
        Action::Commit => &["commit"],
        Action::Push => &["push"],
        Action::Merge => &["merge"],
        Action::Restart => &["restart", "reboot"],
        Action::PluginReinstall => &["reinstall"],
        Action::Delete => &["delete", "remove", "erase"],
        Action::NetworkAcquireMedia => &["download", "fetch"],
    }
}

fn japanese_authority_surface(action: Action, polarity: DeonticPolarity, surface: &str) -> bool {
    let action_terms: &[&str] = match action {
        Action::Edit => &["編集", "変更", "修正"],
        Action::Test => &["テスト", "検証", "確認"],
        Action::Commit => &["コミット"],
        Action::Push => &["プッシュ"],
        Action::Merge => &["マージ"],
        Action::Restart => &["再起動"],
        Action::PluginReinstall => &["再インストール"],
        Action::Delete => &["削除", "消去"],
        Action::NetworkAcquireMedia => &["取得", "ダウンロード"],
    };
    match polarity {
        DeonticPolarity::Grant => ["してください", "して下さい", "すること"]
            .iter()
            .any(|suffix| {
                action_terms
                    .iter()
                    .any(|term| surface.ends_with(&format!("{term}{suffix}")))
            }),
        DeonticPolarity::Deny => ["しないでください", "してはいけない", "しないこと", "禁止"]
            .iter()
            .any(|suffix| {
                action_terms
                    .iter()
                    .any(|term| surface.ends_with(&format!("{term}{suffix}")))
            }),
        _ => false,
    }
}

fn host_action_name(action: Action) -> &'static str {
    match action {
        Action::Edit => "edit",
        Action::Test => "test",
        Action::Commit => "commit",
        Action::Push => "push",
        Action::Merge => "merge",
        Action::Restart => "restart",
        Action::PluginReinstall => "plugin_reinstall",
        Action::Delete => "delete",
        Action::NetworkAcquireMedia => "network_acquire_media",
    }
}

fn user_span_is_plain_prose(source: &str, span: SourceSpan) -> bool {
    if span.start > span.end
        || span.end > source.len()
        || !source.is_char_boundary(span.start)
        || !source.is_char_boundary(span.end)
    {
        return false;
    }
    let mut in_fence: Option<char> = None;
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let line_span = offset..offset + line.len();
        let trimmed = line.trim_start();
        let fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
        if fence {
            let marker = trimmed.chars().next();
            if in_fence.is_none() {
                in_fence = marker;
            } else if marker == in_fence {
                in_fence = None;
            }
        }
        if line_span.start < span.end
            && span.start < line_span.end
            && (in_fence.is_some() || trimmed.starts_with('>'))
        {
            return false;
        }
        offset = line_span.end;
    }
    let excerpt = &source[span.start..span.end];
    if excerpt.contains('<') && excerpt.contains('>') {
        return false;
    }
    !authority_quote_ranges(source)
        .iter()
        .any(|quoted| span.start < quoted.end && quoted.start < span.end)
}

fn authority_quote_ranges(source: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut stack = Vec::<(char, char, usize)>::new();
    for (offset, character) in source.char_indices() {
        let opener = match character {
            '"' | '`' => Some((character, character)),
            '“' => Some(('“', '”')),
            '「' => Some(('「', '」')),
            '『' => Some(('『', '』')),
            _ => None,
        };
        if let Some((open, close)) = opener {
            if open == close
                && stack
                    .last()
                    .is_some_and(|(_, expected, _)| *expected == close)
            {
                if let Some((_, _, start)) = stack.pop() {
                    ranges.push(start..offset + character.len_utf8());
                }
            } else {
                stack.push((open, close, offset));
            }
        } else if stack
            .last()
            .is_some_and(|(_, expected, _)| *expected == character)
        {
            if let Some((_, _, start)) = stack.pop() {
                ranges.push(start..offset + character.len_utf8());
            }
        }
    }
    for (_, _, start) in stack {
        ranges.push(start..source.len());
    }
    ranges
}

fn read_key(path: &Path) -> Result<[u8; 32], HostError> {
    let encoded = fs::read_to_string(path).map_err(|error| HostError::Io(error.to_string()))?;
    decode_key(encoded.trim())
}

fn decode_key(encoded: &str) -> Result<[u8; 32], HostError> {
    if encoded.len() != 64 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HostError::ReceiptKeyInvalid);
    }
    let mut key = [0_u8; 32];
    for (index, slot) in key.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&encoded[index * 2..index * 2 + 2], 16)
            .map_err(|_| HostError::ReceiptKeyInvalid)?;
    }
    Ok(key)
}

fn create_key_file(path: &Path, key: &[u8; 32]) -> Result<(), std::io::Error> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    for byte in key {
        write!(file, "{byte:02x}")?;
    }
    writeln!(file)?;
    file.sync_all()
}

struct ReplayLock {
    path: PathBuf,
    _file: fs::File,
}

impl Drop for ReplayLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum HostError {
    AuthorityHostMismatch,
    AuthorityPermitRejected,
    AuthorityScopeMismatch,
    AuthoritySourceIneligible,
    AuthoritySpanInvalid,
    ArtifactTurnMismatch,
    CandidateDigestMismatch,
    InvalidHostOutputStage,
    HostOutputStageConflict,
    HostSeedUnbound,
    InvalidUserSpan,
    MissingHostReceipt,
    LegacyReceiptRejected,
    ReceiptRejected,
    ReceiptReplay,
    ReceiptKeyInvalid,
    ReceiptKeyPathUnsafe,
    InvalidReplay(String),
    Io(String),
    ReplayBusy,
    ReplayPathOutsideRoot,
    ReplayPathUnsafe,
    ReplayRequiresExactBinding,
    ReplaySequenceExhausted,
    InvalidStopHookEvent,
    StopHookObservationConflict,
    StopHookOutputUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_existing_never_creates_a_receipt_root() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("lc631-missing-receipt-root-{nonce}"));
        assert!(matches!(
            HostReceiptContext::load_existing(&root),
            Err(HostError::Io(_))
        ));
        assert!(!root.exists());
    }

    #[test]
    fn self_attested_seed_cannot_bind_output_or_source_grants() {
        let seed = HostSeedEnvelope::self_attested(ArtifactId(1), TurnId(1), "edit".into());
        assert!(!seed.may_source_grants());
        let result = bind_host_output(
            &seed,
            &HostOutputCandidate {
                artifact_id: ArtifactId(1),
                turn_id: TurnId(1),
                candidate_digest: stable_sha256("candidate"),
                output_text: "done".into(),
            },
            "callback",
        );
        assert_eq!(result, Err(HostError::LegacyReceiptRejected));
    }

    #[test]
    fn legacy_nonempty_receipt_is_rejected() {
        let raw = "please explain".to_string();
        assert_eq!(
            HostSeedEnvelope::verified(
                ArtifactId(7),
                TurnId(9),
                raw.clone(),
                SourceSpan::checked(&raw, 0, raw.len()).unwrap(),
                "signed-looking-host-receipt",
            ),
            Err(HostError::LegacyReceiptRejected)
        );
    }
}
