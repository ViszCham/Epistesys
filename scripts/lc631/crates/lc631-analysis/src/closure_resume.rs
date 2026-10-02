use lc631_core::stable_sha256;
use lc631_receipt_kernel::{
    ReceiptClass, ReceiptIssuer, ReceiptPolicy, ReceiptScope, ReceiptVerifier, ReplayGuard,
    SubjectRevision, UntrustedReceipt,
};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

pub const DGCL_CHECKPOINT_V1_SCHEMA: &str = "epistesys-dgcl-checkpoint.v1";
pub const DGCL_CHECKPOINT_V2_SCHEMA: &str = "epistesys-dgcl-checkpoint.v2";
const MAX_RECORD_BYTES: u64 = 16_384;
const MAX_RECORDS: usize = 1_000;
const MAX_HEAD_ANCHOR_SECONDS: u64 = 24 * 60 * 60;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    UnknownDelivery,
    ConfirmedFailed,
    ConfirmedSuccess,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointEventKind {
    #[default]
    LegacyUnclassified,
    ActionStarted,
    ActionObserved,
    CandidatePrepared,
    CandidateSendAttempted,
    SinkDeliveryObserved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointBody {
    pub schema_version: String,
    pub sequence: u64,
    pub source_revision: String,
    pub plan_digest: String,
    pub authority_revision: String,
    pub action_key: String,
    #[serde(default, skip_serializing_if = "legacy_event_kind")]
    pub event_kind: CheckpointEventKind,
    pub delivery: DeliveryState,
    pub result_digest: Option<String>,
    pub parent_digest: Option<String>,
}

fn legacy_event_kind(event_kind: &CheckpointEventKind) -> bool {
    *event_kind == CheckpointEventKind::LegacyUnclassified
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckpointHeadAnchor {
    pub schema_version: String,
    pub source_revision: String,
    pub plan_digest: String,
    pub authority_revision: String,
    pub sequence: Option<u64>,
    pub head_digest: Option<String>,
    pub issued_at_epoch: u64,
    pub expires_at_epoch: u64,
    pub attestation: UntrustedReceipt,
}

#[derive(Clone, Copy, Debug)]
pub struct CheckpointHeadRequest<'a> {
    pub source_revision: &'a str,
    pub plan_digest: &'a str,
    pub authority_revision: &'a str,
    pub sequence: Option<u64>,
    pub head_digest: Option<&'a str>,
    pub now_epoch: u64,
    pub lease_seconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointIntegrityState {
    Unanchored,
    TrustedHeadMatched,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredCheckpoint {
    body: CheckpointBody,
    digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ResumeObservation {
    pub integrity_state: CheckpointIntegrityState,
    pub latest_sequence: Option<u64>,
    pub latest_digest: Option<String>,
    pub latest_delivery: Option<DeliveryState>,
    pub latest_event_kind: Option<CheckpointEventKind>,
    pub retry_permitted_by_prior_result: bool,
    pub fresh_revalidation_required: bool,
    pub authority_created: bool,
    pub completion_restored: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum ResumeError {
    RootUnsafe,
    Io,
    CorruptCheckpoint,
    ChainMismatch,
    StaleSource,
    StalePlan,
    AuthorityChanged,
    SequenceMismatch,
    DuplicateAction,
    UnknownDelivery,
    TrustedHeadMissing,
    TrustedHeadInvalid,
    TrustedHeadExpired,
    RollbackDetected,
    UnanchoredTail,
    TrustedHeadMismatch,
    InvalidEventTransition,
    BudgetExceeded,
    ConcurrentWriter,
}

// The root and its ancestors must be real directories. A local administrator
// can still replace them; this is not an OS sandbox or a remote freshness root.
fn checked_directory(root: &Path) -> Result<std::path::PathBuf, ResumeError> {
    let absolute = std::path::absolute(root).map_err(|_| ResumeError::RootUnsafe)?;
    for ancestor in absolute.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| ResumeError::RootUnsafe)?;
        if !metadata.is_dir() || is_reparse(&metadata) {
            return Err(ResumeError::RootUnsafe);
        }
    }
    absolute.canonicalize().map_err(|_| ResumeError::RootUnsafe)
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn writer_lock(root: &Path) -> Result<File, ResumeError> {
    let path = root.join(".writer.lock");
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if !metadata.is_file() || is_reparse(&metadata) {
            return Err(ResumeError::RootUnsafe);
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| ResumeError::Io)?;
    file.try_lock().map_err(|_| ResumeError::ConcurrentWriter)?;
    Ok(file)
}

fn distinct_head_root(ledger: &Path, heads: &Path) -> Result<std::path::PathBuf, ResumeError> {
    let ledger = checked_directory(ledger)?;
    let heads = checked_directory(heads)?;
    if ledger.starts_with(&heads) || heads.starts_with(&ledger) {
        return Err(ResumeError::RootUnsafe);
    }
    Ok(heads)
}

/// Append-only head history in a separately provisioned, host-controlled root.
/// A crash between ledger and head writes remains an unanchored tail, not success.
pub fn persist_checkpoint_head(
    ledger: &Path,
    heads: &Path,
    anchor: &CheckpointHeadAnchor,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> Result<(), ResumeError> {
    let heads = distinct_head_root(ledger, heads)?;
    let _lock = writer_lock(&heads)?;
    inspect_resume_with_anchor(
        ledger,
        &anchor.source_revision,
        &anchor.plan_digest,
        &anchor.authority_revision,
        Some(anchor),
        Some(verifier),
        now_epoch,
    )?;
    let index = anchor
        .sequence
        .map_or(0, |sequence| sequence.saturating_add(1));
    if let Some(previous) = read_latest_head(&heads)? {
        if previous.source_revision != anchor.source_revision
            || previous.plan_digest != anchor.plan_digest
            || previous.authority_revision != anchor.authority_revision
            || previous.sequence >= anchor.sequence
        {
            return Err(ResumeError::SequenceMismatch);
        }
        verify_checkpoint_head_anchor(
            &previous,
            verifier,
            &anchor.source_revision,
            &anchor.plan_digest,
            &anchor.authority_revision,
            previous.issued_at_epoch,
        )?;
    }
    let bytes = serde_json::to_vec(anchor).map_err(|_| ResumeError::TrustedHeadInvalid)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(ResumeError::BudgetExceeded);
    }
    let path = heads.join(format!("head-{index:020}.json"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| ResumeError::Io)?;
    file.write_all(&bytes).map_err(|_| ResumeError::Io)?;
    file.sync_all().map_err(|_| ResumeError::Io)
}

pub fn load_checkpoint_head(
    ledger: &Path,
    heads: &Path,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    let heads = distinct_head_root(ledger, heads)?;
    let path = heads.join(".writer.lock");
    let metadata = fs::symlink_metadata(&path).map_err(|_| ResumeError::TrustedHeadMissing)?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(ResumeError::RootUnsafe);
    }
    let lock = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|_| ResumeError::Io)?;
    lock.try_lock_shared()
        .map_err(|_| ResumeError::ConcurrentWriter)?;
    let anchor = read_latest_head(&heads)?.ok_or(ResumeError::TrustedHeadMissing)?;
    verify_checkpoint_head_anchor(
        &anchor,
        verifier,
        &anchor.source_revision,
        &anchor.plan_digest,
        &anchor.authority_revision,
        now_epoch,
    )?;
    Ok(anchor)
}

fn read_latest_head(root: &Path) -> Result<Option<CheckpointHeadAnchor>, ResumeError> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root).map_err(|_| ResumeError::Io)? {
        let path = entry.map_err(|_| ResumeError::Io)?.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(".writer.lock") {
            continue;
        }
        if paths.len() >= MAX_RECORDS {
            return Err(ResumeError::BudgetExceeded);
        }
        paths.push(path);
    }
    paths.sort();
    let mut latest: Option<CheckpointHeadAnchor> = None;
    for path in paths {
        let metadata = fs::symlink_metadata(&path).map_err(|_| ResumeError::Io)?;
        if !metadata.is_file()
            || is_reparse(&metadata)
            || metadata.len() == 0
            || metadata.len() > MAX_RECORD_BYTES
        {
            return Err(ResumeError::TrustedHeadInvalid);
        }
        let anchor: CheckpointHeadAnchor =
            serde_json::from_slice(&fs::read(&path).map_err(|_| ResumeError::Io)?)
                .map_err(|_| ResumeError::TrustedHeadInvalid)?;
        let index = anchor
            .sequence
            .map_or(0, |sequence| sequence.saturating_add(1));
        if path.file_name().and_then(|name| name.to_str())
            != Some(format!("head-{index:020}.json").as_str())
            || latest.as_ref().is_some_and(|prior| {
                prior.sequence >= anchor.sequence
                    || prior.source_revision != anchor.source_revision
                    || prior.plan_digest != anchor.plan_digest
                    || prior.authority_revision != anchor.authority_revision
            })
        {
            return Err(ResumeError::TrustedHeadInvalid);
        }
        latest = Some(anchor);
    }
    Ok(latest)
}

pub fn issue_checkpoint_head_anchor(
    issuer: &ReceiptIssuer,
    request: CheckpointHeadRequest<'_>,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    let CheckpointHeadRequest {
        source_revision,
        plan_digest,
        authority_revision,
        sequence,
        head_digest,
        now_epoch,
        lease_seconds,
    } = request;
    if !valid_digest(source_revision)
        || !valid_digest(plan_digest)
        || authority_revision.trim().is_empty()
        || authority_revision.len() > 512
        || authority_revision.chars().any(char::is_control)
        || sequence.is_some() != head_digest.is_some()
        || head_digest.is_some_and(|digest| !valid_digest(digest))
        || lease_seconds == 0
        || lease_seconds > MAX_HEAD_ANCHOR_SECONDS
    {
        return Err(ResumeError::TrustedHeadInvalid);
    }
    let expires_at_epoch = now_epoch
        .checked_add(lease_seconds)
        .ok_or(ResumeError::TrustedHeadInvalid)?;
    let payload_digest = checkpoint_head_payload_digest(
        source_revision,
        plan_digest,
        authority_revision,
        sequence,
        head_digest,
        now_epoch,
        expires_at_epoch,
    );
    let attestation = issuer
        .issue(
            ReceiptClass::Closure,
            SubjectRevision::checked(source_revision)
                .map_err(|_| ResumeError::TrustedHeadInvalid)?,
            ReceiptScope::checked(checkpoint_head_scope(sequence))
                .map_err(|_| ResumeError::TrustedHeadInvalid)?,
            payload_digest,
            now_epoch,
            Some(expires_at_epoch),
            None,
        )
        .map_err(|_| ResumeError::TrustedHeadInvalid)?;
    Ok(CheckpointHeadAnchor {
        schema_version: "epistesys-dgcl-checkpoint-head.v1".into(),
        source_revision: source_revision.into(),
        plan_digest: plan_digest.into(),
        authority_revision: authority_revision.into(),
        sequence,
        head_digest: head_digest.map(str::to_owned),
        issued_at_epoch: now_epoch,
        expires_at_epoch,
        attestation,
    })
}

pub fn inspect_resume_with_anchor(
    root: &Path,
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
    anchor: Option<&CheckpointHeadAnchor>,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
) -> Result<ResumeObservation, ResumeError> {
    let mut observation = inspect_resume(root, source_revision, plan_digest, authority_revision)?;
    let Some(anchor) = anchor else {
        return Ok(observation);
    };
    let Some(verifier) = verifier else {
        return Err(ResumeError::TrustedHeadMissing);
    };
    verify_checkpoint_head_anchor(
        anchor,
        verifier,
        source_revision,
        plan_digest,
        authority_revision,
        now_epoch,
    )?;
    match (anchor.sequence, observation.latest_sequence) {
        (None, None) if anchor.head_digest.is_none() => {}
        (Some(expected), Some(actual)) if expected == actual => {
            if anchor.head_digest != observation.latest_digest {
                return Err(ResumeError::TrustedHeadMismatch);
            }
        }
        (Some(_), None) => return Err(ResumeError::RollbackDetected),
        (Some(expected), Some(actual)) if actual < expected => {
            return Err(ResumeError::RollbackDetected)
        }
        (Some(expected), Some(actual)) if actual > expected => {
            return Err(ResumeError::UnanchoredTail)
        }
        (None, Some(_)) => return Err(ResumeError::UnanchoredTail),
        _ => return Err(ResumeError::TrustedHeadMismatch),
    }
    observation.integrity_state = CheckpointIntegrityState::TrustedHeadMatched;
    Ok(observation)
}

fn verify_checkpoint_head_anchor(
    anchor: &CheckpointHeadAnchor,
    verifier: &ReceiptVerifier,
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
    now_epoch: u64,
) -> Result<(), ResumeError> {
    if anchor.schema_version != "epistesys-dgcl-checkpoint-head.v1"
        || anchor.source_revision != source_revision
        || anchor.plan_digest != plan_digest
        || anchor.authority_revision != authority_revision
        || anchor.authority_revision.len() > 512
        || anchor.authority_revision.chars().any(char::is_control)
        || anchor.sequence.is_some() != anchor.head_digest.is_some()
        || anchor
            .head_digest
            .as_deref()
            .is_some_and(|digest| !valid_digest(digest))
        || anchor.expires_at_epoch <= anchor.issued_at_epoch
        || anchor.expires_at_epoch - anchor.issued_at_epoch > MAX_HEAD_ANCHOR_SECONDS
    {
        return Err(ResumeError::TrustedHeadInvalid);
    }
    if now_epoch < anchor.issued_at_epoch || now_epoch >= anchor.expires_at_epoch {
        return Err(ResumeError::TrustedHeadExpired);
    }
    let payload_digest = checkpoint_head_payload_digest(
        &anchor.source_revision,
        &anchor.plan_digest,
        &anchor.authority_revision,
        anchor.sequence,
        anchor.head_digest.as_deref(),
        anchor.issued_at_epoch,
        anchor.expires_at_epoch,
    );
    let policy = ReceiptPolicy::exact(
        ReceiptClass::Closure,
        SubjectRevision::checked(source_revision).map_err(|_| ResumeError::TrustedHeadInvalid)?,
        ReceiptScope::checked(checkpoint_head_scope(anchor.sequence))
            .map_err(|_| ResumeError::TrustedHeadInvalid)?,
        payload_digest,
        now_epoch,
    );
    verifier
        .verify(
            anchor.attestation.clone(),
            &policy,
            &mut ReplayGuard::default(),
        )
        .map_err(|_| ResumeError::TrustedHeadInvalid)?;
    Ok(())
}

fn checkpoint_head_scope(sequence: Option<u64>) -> String {
    match sequence {
        Some(sequence) => format!("dgcl/checkpoint/head/{sequence}"),
        None => "dgcl/checkpoint/head/genesis".into(),
    }
}

fn checkpoint_head_payload_digest(
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
    sequence: Option<u64>,
    head_digest: Option<&str>,
    issued_at_epoch: u64,
    expires_at_epoch: u64,
) -> String {
    stable_sha256(&format!(
        "epistesys-dgcl-checkpoint-head.v1\0{source_revision}\0{plan_digest}\0{authority_revision}\0{sequence:?}\0{}\0{issued_at_epoch}\0{expires_at_epoch}",
        head_digest.unwrap_or("genesis")
    ))
}

pub fn append_checkpoint(root: &Path, body: CheckpointBody) -> Result<String, ResumeError> {
    check_root(root)?;
    let root = checked_directory(root)?;
    let _lock = writer_lock(&root)?;
    if !checkpoint_schema_matches(&body)
        || body.action_key.trim().is_empty()
        || body.action_key.len() > 1_024
        || body.action_key.chars().any(char::is_control)
        || !valid_digest(&body.source_revision)
        || !valid_digest(&body.plan_digest)
        || body.authority_revision.is_empty()
        || body.authority_revision.len() > 512
        || body.authority_revision.chars().any(char::is_control)
        || !event_result_is_valid(&body)
    {
        return Err(ResumeError::CorruptCheckpoint);
    }
    let latest = read_latest(&root)?;
    match latest {
        Some((previous, digest)) => {
            if body.sequence != previous.sequence + 1
                || body.parent_digest.as_deref() != Some(digest.as_str())
            {
                return Err(ResumeError::SequenceMismatch);
            }
            if previous.source_revision != body.source_revision {
                return Err(ResumeError::StaleSource);
            }
            if previous.plan_digest != body.plan_digest {
                return Err(ResumeError::StalePlan);
            }
            if previous.authority_revision != body.authority_revision {
                return Err(ResumeError::AuthorityChanged);
            }
            if previous.schema_version != body.schema_version {
                return Err(ResumeError::InvalidEventTransition);
            }
            if previous.delivery == DeliveryState::UnknownDelivery
                && previous.action_key != body.action_key
            {
                return Err(ResumeError::UnknownDelivery);
            }
            if previous.action_key == body.action_key {
                validate_event_transition(&previous, &body)?;
            }
        }
        None if body.sequence != 0 || body.parent_digest.is_some() => {
            return Err(ResumeError::SequenceMismatch)
        }
        None => {}
    }
    let canonical = serde_json::to_string(&body).map_err(|_| ResumeError::CorruptCheckpoint)?;
    let digest = stable_sha256(&canonical);
    let encoded = serde_json::to_vec(&StoredCheckpoint {
        body: body.clone(),
        digest: digest.clone(),
    })
    .map_err(|_| ResumeError::CorruptCheckpoint)?;
    if encoded.len() as u64 > MAX_RECORD_BYTES {
        return Err(ResumeError::BudgetExceeded);
    }
    let target = root.join(format!("checkpoint-{:020}.json", body.sequence));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|_| ResumeError::Io)?;
    file.write_all(&encoded).map_err(|_| ResumeError::Io)?;
    file.sync_all().map_err(|_| ResumeError::Io)?;
    Ok(digest)
}

pub fn inspect_resume(
    root: &Path,
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
) -> Result<ResumeObservation, ResumeError> {
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(empty_observation());
        }
        Err(_) => return Err(ResumeError::RootUnsafe),
        Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
            return Err(ResumeError::RootUnsafe);
        }
        Ok(_) => {}
    }
    let latest = read_latest(root)?;
    if let Some((body, digest)) = latest {
        if body.source_revision != source_revision {
            return Err(ResumeError::StaleSource);
        }
        if body.plan_digest != plan_digest {
            return Err(ResumeError::StalePlan);
        }
        if body.authority_revision != authority_revision {
            return Err(ResumeError::AuthorityChanged);
        }
        return Ok(ResumeObservation {
            integrity_state: CheckpointIntegrityState::Unanchored,
            latest_sequence: Some(body.sequence),
            latest_digest: Some(digest),
            latest_delivery: Some(body.delivery),
            latest_event_kind: Some(body.event_kind),
            retry_permitted_by_prior_result: body.event_kind == CheckpointEventKind::ActionObserved
                && body.delivery == DeliveryState::ConfirmedFailed,
            fresh_revalidation_required: true,
            authority_created: false,
            completion_restored: false,
        });
    }
    Ok(empty_observation())
}

fn empty_observation() -> ResumeObservation {
    ResumeObservation {
        integrity_state: CheckpointIntegrityState::Unanchored,
        latest_sequence: None,
        latest_digest: None,
        latest_delivery: None,
        latest_event_kind: None,
        retry_permitted_by_prior_result: false,
        fresh_revalidation_required: true,
        authority_created: false,
        completion_restored: false,
    }
}

fn event_result_is_valid(body: &CheckpointBody) -> bool {
    match body.event_kind {
        CheckpointEventKind::LegacyUnclassified => {
            (body.delivery == DeliveryState::UnknownDelivery && body.result_digest.is_none())
                || (body.delivery != DeliveryState::UnknownDelivery
                    && body.result_digest.as_deref().is_some_and(valid_digest))
        }
        CheckpointEventKind::ActionStarted
        | CheckpointEventKind::CandidatePrepared
        | CheckpointEventKind::CandidateSendAttempted => {
            body.delivery == DeliveryState::UnknownDelivery && body.result_digest.is_none()
        }
        CheckpointEventKind::ActionObserved | CheckpointEventKind::SinkDeliveryObserved => {
            body.delivery != DeliveryState::UnknownDelivery
                && body.result_digest.as_deref().is_some_and(valid_digest)
        }
    }
}

fn checkpoint_schema_matches(body: &CheckpointBody) -> bool {
    match body.schema_version.as_str() {
        DGCL_CHECKPOINT_V1_SCHEMA => body.event_kind == CheckpointEventKind::LegacyUnclassified,
        DGCL_CHECKPOINT_V2_SCHEMA => body.event_kind != CheckpointEventKind::LegacyUnclassified,
        _ => false,
    }
}

fn validate_event_transition(
    previous: &CheckpointBody,
    next: &CheckpointBody,
) -> Result<(), ResumeError> {
    use CheckpointEventKind as Event;
    use DeliveryState as Outcome;
    let allowed = match (previous.event_kind, next.event_kind) {
        (Event::LegacyUnclassified, Event::LegacyUnclassified) => {
            if previous.delivery != Outcome::UnknownDelivery {
                return Err(ResumeError::DuplicateAction);
            }
            if next.delivery == Outcome::UnknownDelivery {
                return Err(ResumeError::UnknownDelivery);
            }
            true
        }
        (Event::ActionStarted, Event::ActionObserved) => {
            previous.delivery == Outcome::UnknownDelivery
                && next.delivery != Outcome::UnknownDelivery
        }
        (Event::ActionObserved, Event::CandidatePrepared) => {
            previous.delivery == Outcome::ConfirmedSuccess
                && next.delivery == Outcome::UnknownDelivery
        }
        (Event::CandidatePrepared, Event::CandidateSendAttempted) => {
            previous.delivery == Outcome::UnknownDelivery
                && next.delivery == Outcome::UnknownDelivery
        }
        (Event::CandidateSendAttempted, Event::SinkDeliveryObserved) => {
            previous.delivery == Outcome::UnknownDelivery
                && next.delivery != Outcome::UnknownDelivery
        }
        _ => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(ResumeError::InvalidEventTransition)
    }
}

fn check_root(root: &Path) -> Result<(), ResumeError> {
    if !root.exists() {
        fs::create_dir(root).map_err(|_| ResumeError::RootUnsafe)?;
    }
    let metadata = fs::symlink_metadata(root).map_err(|_| ResumeError::RootUnsafe)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(ResumeError::RootUnsafe);
    }
    Ok(())
}

fn read_latest(root: &Path) -> Result<Option<(CheckpointBody, String)>, ResumeError> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root).map_err(|_| ResumeError::Io)? {
        if paths.len() >= MAX_RECORDS {
            return Err(ResumeError::BudgetExceeded);
        }
        paths.push(entry.map_err(|_| ResumeError::Io)?.path());
    }
    paths.retain(|path| path.file_name().and_then(|name| name.to_str()) != Some(".writer.lock"));
    paths.sort();
    let mut prior = None::<String>;
    let mut prior_schema = None::<String>;
    let mut latest = None;
    for (index, path) in paths.into_iter().enumerate() {
        let expected_name = format!("checkpoint-{index:020}.json");
        if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
            return Err(ResumeError::CorruptCheckpoint);
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| ResumeError::Io)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() == 0
            || metadata.len() > MAX_RECORD_BYTES
        {
            return Err(ResumeError::CorruptCheckpoint);
        }
        let raw = fs::read_to_string(path).map_err(|_| ResumeError::CorruptCheckpoint)?;
        let stored: StoredCheckpoint =
            serde_json::from_str(&raw).map_err(|_| ResumeError::CorruptCheckpoint)?;
        if !checkpoint_schema_matches(&stored.body)
            || prior_schema
                .as_deref()
                .is_some_and(|schema| schema != stored.body.schema_version)
            || stored.body.sequence != index as u64
            || stored.body.parent_digest != prior
        {
            return Err(ResumeError::ChainMismatch);
        }
        prior_schema = Some(stored.body.schema_version.clone());
        let canonical =
            serde_json::to_string(&stored.body).map_err(|_| ResumeError::CorruptCheckpoint)?;
        if stored.digest != stable_sha256(&canonical) {
            return Err(ResumeError::ChainMismatch);
        }
        prior = Some(stored.digest.clone());
        latest = Some((stored.body, stored.digest));
    }
    Ok(latest)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(label: &str) -> std::path::PathBuf {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dgcl-resume-{label}-{suffix}"));
        fs::create_dir(&path).unwrap();
        path
    }

    fn body(delivery: DeliveryState) -> CheckpointBody {
        CheckpointBody {
            schema_version: DGCL_CHECKPOINT_V1_SCHEMA.into(),
            sequence: 0,
            source_revision: stable_sha256("source"),
            plan_digest: stable_sha256("plan"),
            authority_revision: "authority-r1".into(),
            action_key: "action-1".into(),
            event_kind: CheckpointEventKind::LegacyUnclassified,
            delivery,
            result_digest: (delivery != DeliveryState::UnknownDelivery)
                .then(|| stable_sha256("observed-result")),
            parent_digest: None,
        }
    }

    #[test]
    fn unknown_delivery_cannot_be_retried_or_restored_as_complete() {
        let path = root("unknown");
        let first = body(DeliveryState::UnknownDelivery);
        let digest = append_checkpoint(&path, first.clone()).unwrap();
        let inspected = inspect_resume(
            &path,
            &first.source_revision,
            &first.plan_digest,
            &first.authority_revision,
        )
        .unwrap();
        assert!(!inspected.retry_permitted_by_prior_result);
        assert!(!inspected.completion_restored);
        let mut unrelated = first.clone();
        unrelated.sequence = 1;
        unrelated.parent_digest = Some(digest.clone());
        unrelated.action_key = "different-action".into();
        unrelated.delivery = DeliveryState::ConfirmedSuccess;
        unrelated.result_digest = Some(stable_sha256("other-result"));
        assert_eq!(
            append_checkpoint(&path, unrelated),
            Err(ResumeError::UnknownDelivery)
        );
        let mut second = first;
        second.sequence = 1;
        second.parent_digest = Some(digest);
        assert_eq!(
            append_checkpoint(&path, second),
            Err(ResumeError::UnknownDelivery)
        );
        let digest = inspect_resume(
            &path,
            &stable_sha256("source"),
            &stable_sha256("plan"),
            "authority-r1",
        )
        .unwrap()
        .latest_digest
        .unwrap();
        let mut observed = body(DeliveryState::ConfirmedSuccess);
        observed.sequence = 1;
        observed.parent_digest = Some(digest);
        append_checkpoint(&path, observed).unwrap();
        let recovered = inspect_resume(
            &path,
            &stable_sha256("source"),
            &stable_sha256("plan"),
            "authority-r1",
        )
        .unwrap();
        assert!(!recovered.retry_permitted_by_prior_result);
        assert!(!recovered.completion_restored);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn partial_checkpoint_and_revoked_authority_fail_visible() {
        let path = root("partial");
        let first = body(DeliveryState::ConfirmedFailed);
        append_checkpoint(&path, first.clone()).unwrap();
        assert_eq!(
            inspect_resume(
                &path,
                &first.source_revision,
                &first.plan_digest,
                "authority-r2"
            ),
            Err(ResumeError::AuthorityChanged)
        );
        fs::write(path.join("checkpoint-00000000000000000001.json"), "{broken").unwrap();
        assert_eq!(
            inspect_resume(
                &path,
                &first.source_revision,
                &first.plan_digest,
                &first.authority_revision
            ),
            Err(ResumeError::CorruptCheckpoint)
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn legacy_v1_checkpoint_wire_omits_new_phase_field_and_v2_requires_it() {
        let legacy = body(DeliveryState::UnknownDelivery);
        let legacy_json = serde_json::to_value(&legacy).unwrap();
        assert_eq!(legacy_json["schema_version"], DGCL_CHECKPOINT_V1_SCHEMA);
        assert!(legacy_json.get("event_kind").is_none());
        let decoded: CheckpointBody = serde_json::from_value(legacy_json.clone()).unwrap();
        assert_eq!(decoded.event_kind, CheckpointEventKind::LegacyUnclassified);

        let mut typed = body(DeliveryState::UnknownDelivery);
        typed.schema_version = DGCL_CHECKPOINT_V2_SCHEMA.into();
        typed.event_kind = CheckpointEventKind::ActionStarted;
        let typed_json = serde_json::to_value(typed).unwrap();
        assert_eq!(typed_json["schema_version"], DGCL_CHECKPOINT_V2_SCHEMA);
        assert_eq!(typed_json["event_kind"], "action_started");
    }

    #[test]
    fn checkpoint_chains_do_not_mix_v1_legacy_and_v2_phase_events() {
        let path = root("mixed-wire-version");
        let first = body(DeliveryState::UnknownDelivery);
        let parent = append_checkpoint(&path, first.clone()).unwrap();
        let mut next = first;
        next.sequence = 1;
        next.parent_digest = Some(parent);
        next.schema_version = DGCL_CHECKPOINT_V2_SCHEMA.into();
        next.event_kind = CheckpointEventKind::ActionObserved;
        next.delivery = DeliveryState::ConfirmedSuccess;
        next.result_digest = Some(stable_sha256("result"));
        assert_eq!(
            append_checkpoint(&path, next),
            Err(ResumeError::InvalidEventTransition)
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn inspecting_missing_ledger_does_not_create_persistent_state() {
        let path = std::env::temp_dir().join(format!(
            "dgcl-resume-absent-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!path.exists());
        let observation = inspect_resume(
            &path,
            &stable_sha256("source"),
            &stable_sha256("plan"),
            "authority-r1",
        )
        .unwrap();
        assert!(observation.latest_sequence.is_none());
        assert!(!path.exists());
    }

    #[test]
    fn trusted_external_head_detects_tail_deletion_while_unanchored_read_stays_unknown() {
        let path = root("trusted-head");
        let first = body(DeliveryState::UnknownDelivery);
        let first_digest = append_checkpoint(&path, first.clone()).unwrap();
        let mut second = first.clone();
        second.sequence = 1;
        second.parent_digest = Some(first_digest);
        second.delivery = DeliveryState::ConfirmedSuccess;
        second.result_digest = Some(stable_sha256("confirmed-command-output"));
        let second_digest = append_checkpoint(&path, second.clone()).unwrap();

        let unanchored = inspect_resume(
            &path,
            &second.source_revision,
            &second.plan_digest,
            &second.authority_revision,
        )
        .unwrap();
        assert_eq!(
            unanchored.integrity_state,
            CheckpointIntegrityState::Unanchored
        );

        let key = [141_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("trusted-head", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("trusted-head", &key).unwrap();
        let anchor = issue_checkpoint_head_anchor(
            &issuer,
            CheckpointHeadRequest {
                source_revision: &second.source_revision,
                plan_digest: &second.plan_digest,
                authority_revision: &second.authority_revision,
                sequence: Some(second.sequence),
                head_digest: Some(&second_digest),
                now_epoch: 1_000,
                lease_seconds: 600,
            },
        )
        .unwrap();
        let verified = inspect_resume_with_anchor(
            &path,
            &second.source_revision,
            &second.plan_digest,
            &second.authority_revision,
            Some(&anchor),
            Some(&verifier),
            1_001,
        )
        .unwrap();
        assert_eq!(
            verified.integrity_state,
            CheckpointIntegrityState::TrustedHeadMatched
        );
        fs::remove_file(path.join("checkpoint-00000000000000000001.json")).unwrap();
        let rollback_without_anchor = inspect_resume(
            &path,
            &second.source_revision,
            &second.plan_digest,
            &second.authority_revision,
        )
        .unwrap();
        assert_eq!(rollback_without_anchor.latest_sequence, Some(0));
        assert_eq!(
            rollback_without_anchor.integrity_state,
            CheckpointIntegrityState::Unanchored
        );
        assert_eq!(
            inspect_resume_with_anchor(
                &path,
                &second.source_revision,
                &second.plan_digest,
                &second.authority_revision,
                Some(&anchor),
                Some(&verifier),
                1_002,
            ),
            Err(ResumeError::RollbackDetected)
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn trusted_head_rejects_unanchored_tail_wrong_trust_root_and_expiry() {
        let path = root("head-tail");
        let first = body(DeliveryState::UnknownDelivery);
        let first_digest = append_checkpoint(&path, first.clone()).unwrap();
        let key = [142_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("trusted-head", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("trusted-head", &key).unwrap();
        let anchor = issue_checkpoint_head_anchor(
            &issuer,
            CheckpointHeadRequest {
                source_revision: &first.source_revision,
                plan_digest: &first.plan_digest,
                authority_revision: &first.authority_revision,
                sequence: Some(first.sequence),
                head_digest: Some(&first_digest),
                now_epoch: 2_000,
                lease_seconds: 30,
            },
        )
        .unwrap();
        let mut second = first.clone();
        second.sequence = 1;
        second.parent_digest = Some(first_digest);
        second.delivery = DeliveryState::ConfirmedSuccess;
        second.result_digest = Some(stable_sha256("next-result"));
        append_checkpoint(&path, second).unwrap();
        assert_eq!(
            inspect_resume_with_anchor(
                &path,
                &first.source_revision,
                &first.plan_digest,
                &first.authority_revision,
                Some(&anchor),
                Some(&verifier),
                2_001,
            ),
            Err(ResumeError::UnanchoredTail)
        );
        let wrong_verifier = ReceiptVerifier::from_key_bytes("wrong-root", &[143_u8; 32]).unwrap();
        assert_eq!(
            inspect_resume_with_anchor(
                &path,
                &first.source_revision,
                &first.plan_digest,
                &first.authority_revision,
                Some(&anchor),
                Some(&wrong_verifier),
                2_001,
            ),
            Err(ResumeError::TrustedHeadInvalid)
        );
        assert_eq!(
            inspect_resume_with_anchor(
                &path,
                &first.source_revision,
                &first.plan_digest,
                &first.authority_revision,
                Some(&anchor),
                Some(&verifier),
                2_030,
            ),
            Err(ResumeError::TrustedHeadExpired)
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn recalculated_record_digest_cannot_forge_the_external_head() {
        let path = root("head-rehash");
        let first = body(DeliveryState::ConfirmedFailed);
        let digest = append_checkpoint(&path, first.clone()).unwrap();
        let key = [144_u8; 32];
        let issuer =
            lc631_receipt_kernel::ReceiptIssuer::from_key_bytes("trusted-head", &key).unwrap();
        let verifier = ReceiptVerifier::from_key_bytes("trusted-head", &key).unwrap();
        let anchor = issue_checkpoint_head_anchor(
            &issuer,
            CheckpointHeadRequest {
                source_revision: &first.source_revision,
                plan_digest: &first.plan_digest,
                authority_revision: &first.authority_revision,
                sequence: Some(0),
                head_digest: Some(&digest),
                now_epoch: 3_000,
                lease_seconds: 30,
            },
        )
        .unwrap();
        let record_path = path.join("checkpoint-00000000000000000000.json");
        let mut record: StoredCheckpoint =
            serde_json::from_str(&fs::read_to_string(&record_path).unwrap()).unwrap();
        record.body.action_key = "attacker-recalculated-chain".into();
        let canonical = serde_json::to_string(&record.body).unwrap();
        record.digest = stable_sha256(&canonical);
        fs::write(&record_path, serde_json::to_vec(&record).unwrap()).unwrap();
        assert_eq!(
            inspect_resume_with_anchor(
                &path,
                &first.source_revision,
                &first.plan_digest,
                &first.authority_revision,
                Some(&anchor),
                Some(&verifier),
                3_001,
            ),
            Err(ResumeError::TrustedHeadMismatch)
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn concurrent_same_sequence_writers_never_both_commit() {
        use std::sync::{Arc, Barrier};

        let path = root("two-writers");
        let barrier = Arc::new(Barrier::new(3));
        let mut threads = Vec::new();
        for suffix in ["a", "b"] {
            let worker_path = path.clone();
            let worker_barrier = Arc::clone(&barrier);
            threads.push(std::thread::spawn(move || {
                let mut checkpoint = body(DeliveryState::UnknownDelivery);
                checkpoint.action_key = format!("action-{suffix}");
                worker_barrier.wait();
                append_checkpoint(&worker_path, checkpoint)
            }));
        }
        barrier.wait();
        let outcomes = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(outcomes.iter().filter(|result| result.is_err()).count(), 1);
        let entries = fs::read_dir(&path)
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_name() != ".writer.lock")
            .count();
        assert_eq!(entries, 1);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn event_phases_separate_execution_candidate_send_and_sink_delivery() {
        let path = root("event-phases");
        let mut checkpoint = body(DeliveryState::UnknownDelivery);
        checkpoint.schema_version = DGCL_CHECKPOINT_V2_SCHEMA.into();
        checkpoint.event_kind = CheckpointEventKind::ActionStarted;
        let mut parent = append_checkpoint(&path, checkpoint.clone()).unwrap();

        checkpoint.sequence += 1;
        checkpoint.parent_digest = Some(parent);
        checkpoint.event_kind = CheckpointEventKind::ActionObserved;
        checkpoint.delivery = DeliveryState::ConfirmedSuccess;
        checkpoint.result_digest = Some(stable_sha256("action-output"));
        parent = append_checkpoint(&path, checkpoint.clone()).unwrap();
        let action_observation = inspect_resume(
            &path,
            &checkpoint.source_revision,
            &checkpoint.plan_digest,
            &checkpoint.authority_revision,
        )
        .unwrap();
        assert_eq!(
            action_observation.latest_event_kind,
            Some(CheckpointEventKind::ActionObserved)
        );
        assert!(!action_observation.completion_restored);

        checkpoint.sequence += 1;
        checkpoint.parent_digest = Some(parent);
        checkpoint.event_kind = CheckpointEventKind::CandidatePrepared;
        checkpoint.delivery = DeliveryState::UnknownDelivery;
        checkpoint.result_digest = None;
        parent = append_checkpoint(&path, checkpoint.clone()).unwrap();
        checkpoint.sequence += 1;
        checkpoint.parent_digest = Some(parent);
        checkpoint.event_kind = CheckpointEventKind::CandidateSendAttempted;
        parent = append_checkpoint(&path, checkpoint.clone()).unwrap();
        checkpoint.sequence += 1;
        checkpoint.parent_digest = Some(parent);
        checkpoint.event_kind = CheckpointEventKind::SinkDeliveryObserved;
        checkpoint.delivery = DeliveryState::ConfirmedFailed;
        checkpoint.result_digest = Some(stable_sha256("sink-error"));
        append_checkpoint(&path, checkpoint.clone()).unwrap();
        let sink_observation = inspect_resume(
            &path,
            &checkpoint.source_revision,
            &checkpoint.plan_digest,
            &checkpoint.authority_revision,
        )
        .unwrap();
        assert_eq!(
            sink_observation.latest_event_kind,
            Some(CheckpointEventKind::SinkDeliveryObserved)
        );
        assert!(!sink_observation.retry_permitted_by_prior_result);
        assert!(!sink_observation.completion_restored);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn phase_skips_and_outcome_mismatch_are_rejected_before_persisting() {
        let path = root("phase-skip");
        let mut started = body(DeliveryState::UnknownDelivery);
        started.schema_version = DGCL_CHECKPOINT_V2_SCHEMA.into();
        started.event_kind = CheckpointEventKind::ActionStarted;
        let first_digest = append_checkpoint(&path, started.clone()).unwrap();
        let mut skipped = started.clone();
        skipped.sequence = 1;
        skipped.parent_digest = Some(first_digest);
        skipped.event_kind = CheckpointEventKind::CandidatePrepared;
        assert_eq!(
            append_checkpoint(&path, skipped),
            Err(ResumeError::InvalidEventTransition)
        );
        let mut mismatched = body(DeliveryState::UnknownDelivery);
        mismatched.schema_version = DGCL_CHECKPOINT_V2_SCHEMA.into();
        mismatched.event_kind = CheckpointEventKind::ActionStarted;
        mismatched.delivery = DeliveryState::ConfirmedSuccess;
        mismatched.result_digest = Some(stable_sha256("unobserved"));
        assert_eq!(
            append_checkpoint(&path, mismatched),
            Err(ResumeError::CorruptCheckpoint)
        );
        assert_eq!(
            fs::read_dir(&path)
                .unwrap()
                .filter(|entry| entry.as_ref().unwrap().file_name() != ".writer.lock")
                .count(),
            1
        );
        fs::remove_dir_all(path).unwrap();
    }
}
