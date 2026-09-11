use super::{
    host_output_receipt_basis_digest, map_receipt_error, output_scope, output_subject, HostError,
    HostOutputBindingOrigin, HostOutputReceipt, HOST_CONTRACT_SCHEMA,
};
use lc631_core::{stable_sha256, ArtifactId, TurnId};
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptVerifier, ReplayGuard,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const CODEX_STOP_HOOK_SCHEMA: &str = "lc631-codex-stop-hook.v1";
pub const CLAUDE_STOP_HOOK_SCHEMA: &str = "lc631-claude-stop-hook.v1";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CodexStopHookEvent {
    pub session_id: String,
    pub transcript_path: Option<String>,
    pub cwd: String,
    pub hook_event_name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    pub permission_mode: String,
    pub stop_hook_active: bool,
    pub last_assistant_message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StopHookObservation {
    pub schema_version: &'static str,
    pub session_digest: String,
    pub host_turn_digest: String,
    pub output_digest: String,
    pub callback_payload_digest: String,
    pub exact_binding: bool,
    pub automatic_host_callback_observed: bool,
    pub persistent_supervision_observed: bool,
    pub host: &'static str,
    pub model: String,
    pub permission_mode: String,
    pub observation_path: Option<PathBuf>,
    pub claim_boundary: &'static str,
}

pub fn bind_codex_stop_hook(
    event: &CodexStopHookEvent,
    raw_payload: &str,
) -> Result<(HostOutputReceipt, StopHookObservation), HostError> {
    let _ = (event, raw_payload);
    Err(HostError::LegacyReceiptRejected)
}

pub fn bind_codex_stop_hook_attested(
    event: &CodexStopHookEvent,
    raw_payload: &str,
    issuer: &ReceiptIssuer,
    verifier: &ReceiptVerifier,
    replay: &mut ReplayGuard,
    now_epoch: u64,
) -> Result<(HostOutputReceipt, StopHookObservation), HostError> {
    if event.hook_event_name != "Stop"
        || event.session_id.trim().is_empty()
        || raw_payload.trim().is_empty()
    {
        return Err(HostError::InvalidStopHookEvent);
    }
    let output = event
        .last_assistant_message
        .as_ref()
        .filter(|message| !message.is_empty())
        .ok_or(HostError::StopHookOutputUnavailable)?;
    let session_digest = stable_sha256(&event.session_id);
    let (host, schema_version, binding_origin, claim_boundary) = match event.turn_id.as_deref() {
        Some(turn_id) if !turn_id.trim().is_empty() => (
            "codex",
            CODEX_STOP_HOOK_SCHEMA,
            HostOutputBindingOrigin::CodexStopHook,
            "Codex supplied last_assistant_message at Stop; this binds the host callback bytes but does not authenticate user authority, prove delivery to a remote client, or validate semantic truth",
        ),
        _ => (
            "claude",
            CLAUDE_STOP_HOOK_SCHEMA,
            HostOutputBindingOrigin::ClaudeStopHook,
            "Claude supplied last_assistant_message at Stop without model or turn_id; this binds the host callback bytes but does not authenticate model identity, user authority, remote delivery, or semantic truth",
        ),
    };
    let turn_identity = event
        .turn_id
        .as_deref()
        .filter(|turn_id| !turn_id.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| stable_sha256(raw_payload));
    let host_turn_digest = stable_sha256(&format!("{}:{}", event.session_id, turn_identity));
    let output_digest = stable_sha256(output);
    let callback_payload_digest = stable_sha256(raw_payload);
    let artifact_id = ArtifactId(digest_u64(&session_digest)?);
    let turn_id = TurnId(digest_u64(&host_turn_digest)?);
    let seed_receipt_digest = stable_sha256(&format!(
        "{host}-stop-hook-no-user-authority:{}:{}",
        event.session_id, turn_identity
    ));
    let payload = host_output_receipt_basis_digest(
        artifact_id,
        turn_id,
        &output_digest,
        &output_digest,
        &callback_payload_digest,
        &seed_receipt_digest,
        binding_origin,
    );
    let subject = output_subject(artifact_id, turn_id, &output_digest)?;
    let scope = output_scope(artifact_id, turn_id)?;
    let wire = issuer
        .issue(
            ReceiptClass::HostOutput,
            subject.clone(),
            scope.clone(),
            payload.clone(),
            now_epoch,
            None,
            None,
        )
        .map_err(map_receipt_error)?;
    let verified = verifier
        .verify(
            wire,
            &ReceiptPolicy::exact(ReceiptClass::HostOutput, subject, scope, payload, now_epoch),
            replay,
        )
        .map_err(map_receipt_error)?;
    let authenticity_receipt_digest = verified.receipt_digest();
    let authenticity_attestation = verified.into_untrusted();
    let receipt = HostOutputReceipt {
        schema_version: HOST_CONTRACT_SCHEMA,
        artifact_id,
        turn_id,
        candidate_digest: output_digest.clone(),
        output_digest: output_digest.clone(),
        host_callback_digest: callback_payload_digest.clone(),
        exact_binding: true,
        durable_replay_observed: false,
        persistent_supervision_observed: true,
        host_seed_receipt_digest: seed_receipt_digest,
        automatic_host_callback_observed: true,
        binding_origin,
        authenticity_receipt_digest: Some(authenticity_receipt_digest),
        authenticity_attestation: Some(authenticity_attestation),
    };
    let observation = StopHookObservation {
        schema_version,
        session_digest,
        host_turn_digest,
        output_digest,
        callback_payload_digest,
        exact_binding: true,
        automatic_host_callback_observed: true,
        persistent_supervision_observed: true,
        host,
        model: event
            .model
            .clone()
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| "unobserved".into()),
        permission_mode: event.permission_mode.clone(),
        observation_path: None,
        claim_boundary,
    };
    Ok((receipt, observation))
}

pub fn persist_stop_hook_observation(
    owned_root: &Path,
    observation: &StopHookObservation,
) -> Result<PathBuf, HostError> {
    fs::create_dir_all(owned_root).map_err(|error| HostError::Io(error.to_string()))?;
    let canonical_root = owned_root
        .canonicalize()
        .map_err(|error| HostError::Io(error.to_string()))?;
    let observations = canonical_root.join("observations");
    fs::create_dir_all(&observations).map_err(|error| HostError::Io(error.to_string()))?;
    let file_name = format!(
        "{}-{}.json",
        digest_suffix(&observation.session_digest)?,
        digest_suffix(&observation.host_turn_digest)?
    );
    let path = observations.join(file_name);
    let mut stored = observation.clone();
    stored.observation_path = Some(path.clone());
    let mut bytes =
        serde_json::to_vec(&stored).map_err(|error| HostError::InvalidReplay(error.to_string()))?;
    bytes.push(b'\n');
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(&bytes)
                .map_err(|error| HostError::Io(error.to_string()))?;
            file.sync_all()
                .map_err(|error| HostError::Io(error.to_string()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read(&path).map_err(|error| HostError::Io(error.to_string()))?;
            if existing != bytes {
                return Err(HostError::StopHookObservationConflict);
            }
        }
        Err(error) => return Err(HostError::Io(error.to_string())),
    }
    Ok(path)
}

fn digest_u64(digest: &str) -> Result<u64, HostError> {
    let hex = digest
        .strip_prefix("sha256:")
        .and_then(|value| value.get(..16))
        .ok_or_else(|| HostError::InvalidReplay("invalid_digest_u64".into()))?;
    u64::from_str_radix(hex, 16)
        .map_err(|error| HostError::InvalidReplay(format!("invalid_digest_u64:{error}")))
}

fn digest_suffix(digest: &str) -> Result<&str, HostError> {
    digest
        .strip_prefix("sha256:")
        .and_then(|value| value.get(..16))
        .ok_or_else(|| HostError::InvalidReplay("invalid_digest_suffix".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_hook_binds_exact_host_message_without_granting_user_authority() {
        let raw = r#"{"session_id":"s","cwd":".","hook_event_name":"Stop","model":"gpt","turn_id":"t","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"exact"}"#;
        let event: CodexStopHookEvent = serde_json::from_str(raw).unwrap();
        let context = crate::HostReceiptContext::from_key_bytes(&[41_u8; 32]).unwrap();
        let (receipt, observation) = bind_codex_stop_hook_attested(
            &event,
            raw,
            context.issuer(),
            context.verifier(),
            &mut ReplayGuard::default(),
            1,
        )
        .unwrap();
        assert!(receipt.exact_binding);
        assert!(receipt.automatic_host_callback_observed);
        assert_eq!(
            receipt.binding_origin,
            HostOutputBindingOrigin::CodexStopHook
        );
        assert_eq!(receipt.output_digest, stable_sha256("exact"));
        assert!(observation.persistent_supervision_observed);
    }

    #[test]
    fn missing_host_message_is_fail_visible() {
        let event = CodexStopHookEvent {
            session_id: "s".into(),
            transcript_path: None,
            cwd: ".".into(),
            hook_event_name: "Stop".into(),
            model: Some("gpt".into()),
            turn_id: Some("t".into()),
            permission_mode: "default".into(),
            stop_hook_active: false,
            last_assistant_message: None,
        };
        let context = crate::HostReceiptContext::from_key_bytes(&[41_u8; 32]).unwrap();
        assert_eq!(
            bind_codex_stop_hook_attested(
                &event,
                "{}",
                context.issuer(),
                context.verifier(),
                &mut ReplayGuard::default(),
                1,
            ),
            Err(HostError::StopHookOutputUnavailable)
        );
    }

    #[test]
    fn claude_stop_hook_uses_payload_digest_when_turn_id_is_absent() {
        let raw = r#"{"session_id":"s","cwd":".","hook_event_name":"Stop","permission_mode":"default","stop_hook_active":false,"last_assistant_message":"exact"}"#;
        let event: CodexStopHookEvent = serde_json::from_str(raw).unwrap();
        let context = crate::HostReceiptContext::from_key_bytes(&[41_u8; 32]).unwrap();
        let (receipt, observation) = bind_codex_stop_hook_attested(
            &event,
            raw,
            context.issuer(),
            context.verifier(),
            &mut ReplayGuard::default(),
            1,
        )
        .unwrap();
        assert_eq!(
            receipt.binding_origin,
            HostOutputBindingOrigin::ClaudeStopHook
        );
        assert_eq!(observation.host, "claude");
        assert_eq!(observation.model, "unobserved");
        assert_eq!(observation.schema_version, CLAUDE_STOP_HOOK_SCHEMA);
    }
}
