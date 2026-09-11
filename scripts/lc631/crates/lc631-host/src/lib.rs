#![forbid(unsafe_code)]

mod stop_hook;

pub use stop_hook::{
    bind_codex_stop_hook, bind_codex_stop_hook_attested, persist_stop_hook_observation,
    CodexStopHookEvent, StopHookObservation,
};

use lc631_core::{stable_sha256, ArtifactId, SourceSpan, TurnId};
use lc631_receipt_kernel::{
    generate_key, ReceiptClass, ReceiptError, ReceiptIssuer, ReceiptPolicy, ReceiptScope,
    ReceiptVerifier, ReplayGuard, SubjectRevision, UntrustedReceipt,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
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
    verified_receipt_digest: Option<String>,
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
        Ok(Self {
            schema_version: HOST_CONTRACT_SCHEMA,
            artifact_id,
            turn_id,
            raw_user_message,
            user_span,
            binding_state: HostBindingState::VerifiedHostUser,
            host_receipt_digest: Some(receipt_digest.clone()),
            verified_receipt_digest: Some(receipt_digest),
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
            verified_receipt_digest: None,
        }
    }

    pub fn may_source_grants(&self) -> bool {
        self.binding_state == HostBindingState::VerifiedHostUser
            && self.host_receipt_digest.is_some()
            && self.host_receipt_digest == self.verified_receipt_digest
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

impl HostReceiptContext {
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
    ArtifactTurnMismatch,
    CandidateDigestMismatch,
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
