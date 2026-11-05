use lc631_core::stable_sha256;
use lc631_process_supervisor::{ResourceError, ResourceReservationView};
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
pub const MAX_CHECKPOINT_RECORD_BYTES: u64 = 16_384;
pub const MAX_CHECKPOINT_RECORDS: usize = 1_000;
pub const MAX_CHECKPOINT_CHAIN_BYTES: usize = 16 * 1024 * 1024;
const MAX_RECORD_BYTES: u64 = MAX_CHECKPOINT_RECORD_BYTES;
const MAX_RECORDS: usize = MAX_CHECKPOINT_RECORDS;
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
    HistoricalHeadMatched,
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
    Resource(String),
}

/// A capped byte view only, not a parsed/trusted checkpoint or authority.
pub fn read_checkpoint_record_with_resources<'a, R: std::io::Read + ?Sized>(
    reader: &mut R,
    source: lc631_substrate::Digest32,
    view: &'a ResourceReservationView<'a>,
) -> Result<crate::bounded_input::AnalysisText<'a>, ResourceError> {
    crate::bounded_input::AnalysisText::read(reader, view, source, MAX_RECORD_BYTES as usize)
}

fn checkpoint_text<'a>(
    path: &Path,
    view: Option<&'a ResourceReservationView<'a>>,
) -> Result<crate::bounded_input::TextInput<'a>, ResumeError> {
    match view {
        None => crate::bounded_input::legacy_text_file(path, MAX_RECORD_BYTES as usize)
            .map(crate::bounded_input::TextInput::Legacy)
            .map_err(|_| ResumeError::CorruptCheckpoint),
        Some(view) => {
            let mut file = File::open(path).map_err(|_| ResumeError::Io)?;
            read_checkpoint_record_with_resources(&mut file, view.source_revision(), view)
                .map(crate::bounded_input::TextInput::Reserved)
                .map_err(resume_resource)
        }
    }
}
fn resume_resource(error: ResourceError) -> ResumeError {
    ResumeError::Resource(format!("{error:?}"))
}
fn check_view(view: Option<&ResourceReservationView<'_>>) -> Result<(), ResumeError> {
    if let Some(view) = view {
        view.check().map_err(resume_resource)?;
    }
    Ok(())
}

fn coordination_lock(path: &Path) -> Result<bool, ResumeError> {
    if !matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".writer.lock" | ".resource-owner-v1.lock")
    ) {
        return Ok(false);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| ResumeError::RootUnsafe)?;
    if !metadata.is_file() || is_reparse(&metadata) || metadata.len() != 0 {
        return Err(ResumeError::RootUnsafe);
    }
    Ok(true)
}
fn check_source(
    view: &ResourceReservationView<'_>,
    source_revision: &str,
) -> Result<(), ResumeError> {
    view.check().map_err(resume_resource)?;
    let mut expected = String::from("sha256:");
    for byte in view.source_revision().as_bytes() {
        use std::fmt::Write as _;
        write!(expected, "{byte:02x}").map_err(|_| ResumeError::Io)?;
    }
    if expected != source_revision {
        return Err(ResumeError::StaleSource);
    }
    Ok(())
}

fn record_write_claim<'a>(
    view: Option<&'a ResourceReservationView<'a>>,
) -> Result<Option<lc631_substrate::resource::ResourceClaim<'a>>, ResumeError> {
    use lc631_substrate::resource::{ResourceAmount, ResourceRequest};
    let Some(view) = view else { return Ok(None) };
    view.check().map_err(resume_resource)?;
    let amount = ResourceAmount::checked(MAX_RECORD_BYTES).map_err(resume_resource)?;
    view.claim_resources(&[
        ResourceRequest::new(lc631_process_supervisor::ResourceKind::Ram, amount),
        ResourceRequest::new(lc631_process_supervisor::ResourceKind::Disk, amount),
    ])
    .map(Some)
    .map_err(resume_resource)
}

fn store_record(
    path: &Path,
    bytes: &[u8],
    view: Option<&ResourceReservationView<'_>>,
) -> Result<(), ResumeError> {
    check_view(view)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| ResumeError::Io)?;
        file.write_all(bytes).map_err(|_| ResumeError::Io)?;
        file.sync_all().map_err(|_| ResumeError::Io)
    })();
    if result.is_err() {
        if let Some(view) = view {
            view.record_cleanup_unknown();
        }
    }
    result?;
    check_view(view)
}

#[derive(Clone, Copy)]
pub struct CheckpointReadIdentity<'a> {
    pub source_revision: &'a str,
    pub plan_digest: &'a str,
    pub authority_revision: &'a str,
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
    persist_head_io(ledger, heads, anchor, verifier, now_epoch, None)
}
pub fn persist_checkpoint_head_with_resources(
    ledger: &Path,
    heads: &Path,
    anchor: &CheckpointHeadAnchor,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    view: &ResourceReservationView<'_>,
) -> Result<(), ResumeError> {
    check_source(view, &anchor.source_revision)?;
    persist_head_io(ledger, heads, anchor, verifier, now_epoch, Some(view))
}
fn persist_head_io(
    ledger: &Path,
    heads: &Path,
    anchor: &CheckpointHeadAnchor,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<(), ResumeError> {
    let _encoding = record_write_claim(view)?;
    let heads = distinct_head_root(ledger, heads)?;
    let _lock = writer_lock(&heads)?;
    inspect_anchor_io(
        ledger,
        CheckpointReadIdentity {
            source_revision: &anchor.source_revision,
            plan_digest: &anchor.plan_digest,
            authority_revision: &anchor.authority_revision,
        },
        Some(anchor),
        Some(verifier),
        now_epoch,
        view,
    )?;
    let index = anchor
        .sequence
        .map_or(0, |sequence| sequence.saturating_add(1));
    if let Some(previous) = read_latest_head_io(&heads, view)? {
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
    store_record(&path, &bytes, view)
}

pub fn load_checkpoint_head(
    ledger: &Path,
    heads: &Path,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    load_checkpoint_head_inner(ledger, heads, verifier, now_epoch, false)
}

/// Verify an authentic stored head as history, never as a current lease.
pub fn load_checkpoint_head_historical(
    ledger: &Path,
    heads: &Path,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    load_checkpoint_head_inner(ledger, heads, verifier, now_epoch, true)
}

fn load_checkpoint_head_inner(
    ledger: &Path,
    heads: &Path,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    historical: bool,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    load_head_io(ledger, heads, verifier, now_epoch, historical, None)
}

pub fn load_checkpoint_head_with_resources(
    ledger: &Path,
    heads: &Path,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    view: &ResourceReservationView<'_>,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    load_head_io(ledger, heads, verifier, now_epoch, false, Some(view))
}
pub fn load_checkpoint_head_historical_with_resources(
    ledger: &Path,
    heads: &Path,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    view: &ResourceReservationView<'_>,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    load_head_io(ledger, heads, verifier, now_epoch, true, Some(view))
}

fn load_head_io(
    ledger: &Path,
    heads: &Path,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    historical: bool,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<CheckpointHeadAnchor, ResumeError> {
    check_view(view)?;
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
    let anchor = read_latest_head_io(&heads, view)?.ok_or(ResumeError::TrustedHeadMissing)?;
    if let Some(view) = view {
        check_source(view, &anchor.source_revision)?;
    }
    if historical && now_epoch < anchor.issued_at_epoch {
        return Err(ResumeError::TrustedHeadExpired);
    }
    verify_checkpoint_head_anchor(
        &anchor,
        verifier,
        &anchor.source_revision,
        &anchor.plan_digest,
        &anchor.authority_revision,
        if historical {
            anchor.issued_at_epoch
        } else {
            now_epoch
        },
    )?;
    Ok(anchor)
}

fn read_latest_head_io(
    root: &Path,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<Option<CheckpointHeadAnchor>, ResumeError> {
    check_view(view)?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(root).map_err(|_| ResumeError::Io)? {
        check_view(view)?;
        let path = entry.map_err(|_| ResumeError::Io)?.path();
        if coordination_lock(&path)? {
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
        check_view(view)?;
        let metadata = fs::symlink_metadata(&path).map_err(|_| ResumeError::Io)?;
        if !metadata.is_file()
            || is_reparse(&metadata)
            || metadata.len() == 0
            || metadata.len() > MAX_RECORD_BYTES
        {
            return Err(ResumeError::TrustedHeadInvalid);
        }
        let input = checkpoint_text(&path, view)?;
        let anchor: CheckpointHeadAnchor = serde_json::from_str(
            input
                .as_str()
                .map_err(|_| ResumeError::TrustedHeadInvalid)?,
        )
        .map_err(|_| ResumeError::TrustedHeadInvalid)?;
        let index = anchor
            .sequence
            .map_or(0, |sequence| sequence.saturating_add(1));
        let ordinary_name = format!("head-{index:020}.json");
        let renewal_name = checkpoint_head_renewal_name(&anchor);
        let is_renewal =
            path.file_name().and_then(|name| name.to_str()) == Some(renewal_name.as_str());
        if (is_renewal
            && latest.as_ref().is_none_or(|prior| {
                prior.sequence != anchor.sequence || prior.head_digest != anchor.head_digest
            }))
            || (!is_renewal
                && path.file_name().and_then(|name| name.to_str()) != Some(ordinary_name.as_str()))
            || latest.as_ref().is_some_and(|prior| {
                prior.sequence > anchor.sequence
                    || (prior.sequence == anchor.sequence
                        && (!is_renewal
                            || prior.head_digest != anchor.head_digest
                            || prior.issued_at_epoch >= anchor.issued_at_epoch))
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

fn checkpoint_head_renewal_name(anchor: &CheckpointHeadAnchor) -> String {
    let index = anchor
        .sequence
        .map_or(0, |sequence| sequence.saturating_add(1));
    format!("head-{index:020}.renew-{:020}.json", anchor.issued_at_epoch)
}

/// Persist an explicitly renewed lease for the exact existing historical head.
/// This neither appends an action nor validates a program; production callers
/// must obtain current operation authority and fresh validation independently.
pub fn persist_checkpoint_head_renewal(
    ledger: &Path,
    heads: &Path,
    renewed: &CheckpointHeadAnchor,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
) -> Result<(), ResumeError> {
    persist_renewal_io(ledger, heads, renewed, verifier, now_epoch, None)
}
pub fn persist_checkpoint_head_renewal_with_resources(
    ledger: &Path,
    heads: &Path,
    renewed: &CheckpointHeadAnchor,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    view: &ResourceReservationView<'_>,
) -> Result<(), ResumeError> {
    check_source(view, &renewed.source_revision)?;
    persist_renewal_io(ledger, heads, renewed, verifier, now_epoch, Some(view))
}
fn persist_renewal_io(
    ledger: &Path,
    heads: &Path,
    renewed: &CheckpointHeadAnchor,
    verifier: &ReceiptVerifier,
    now_epoch: u64,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<(), ResumeError> {
    let _encoding = record_write_claim(view)?;
    let heads = distinct_head_root(ledger, heads)?;
    let _lock = writer_lock(&heads)?;
    let previous = read_latest_head_io(&heads, view)?.ok_or(ResumeError::TrustedHeadMissing)?;
    verify_checkpoint_head_anchor(
        &previous,
        verifier,
        &renewed.source_revision,
        &renewed.plan_digest,
        &renewed.authority_revision,
        previous.issued_at_epoch,
    )?;
    if previous.sequence != renewed.sequence
        || previous.head_digest != renewed.head_digest
        || renewed.issued_at_epoch <= previous.issued_at_epoch
    {
        return Err(ResumeError::SequenceMismatch);
    }
    inspect_anchor_io(
        ledger,
        CheckpointReadIdentity {
            source_revision: &renewed.source_revision,
            plan_digest: &renewed.plan_digest,
            authority_revision: &renewed.authority_revision,
        },
        Some(renewed),
        Some(verifier),
        now_epoch,
        view,
    )?;
    let bytes = serde_json::to_vec(renewed).map_err(|_| ResumeError::TrustedHeadInvalid)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(ResumeError::BudgetExceeded);
    }
    store_record(
        &heads.join(checkpoint_head_renewal_name(renewed)),
        &bytes,
        view,
    )
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
    inspect_anchor_io(
        root,
        CheckpointReadIdentity {
            source_revision,
            plan_digest,
            authority_revision,
        },
        anchor,
        verifier,
        now_epoch,
        None,
    )
}

pub fn inspect_resume_with_anchor_with_resources(
    root: &Path,
    identity: CheckpointReadIdentity<'_>,
    anchor: Option<&CheckpointHeadAnchor>,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
    view: &ResourceReservationView<'_>,
) -> Result<ResumeObservation, ResumeError> {
    check_source(view, identity.source_revision)?;
    inspect_anchor_io(root, identity, anchor, verifier, now_epoch, Some(view))
}

fn inspect_anchor_io(
    root: &Path,
    identity: CheckpointReadIdentity<'_>,
    anchor: Option<&CheckpointHeadAnchor>,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<ResumeObservation, ResumeError> {
    let CheckpointReadIdentity {
        source_revision,
        plan_digest,
        authority_revision,
    } = identity;
    let mut observation =
        inspect_resume_io(root, source_revision, plan_digest, authority_revision, view)?;
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
            return Err(ResumeError::RollbackDetected);
        }
        (Some(expected), Some(actual)) if actual > expected => {
            return Err(ResumeError::UnanchoredTail);
        }
        (None, Some(_)) => return Err(ResumeError::UnanchoredTail),
        _ => return Err(ResumeError::TrustedHeadMismatch),
    }
    observation.integrity_state = CheckpointIntegrityState::TrustedHeadMatched;
    Ok(observation)
}

/// Authenticate the original head and exact ledger chain at the original
/// issuance time while reporting an explicitly historical, non-authorizing state.
#[allow(clippy::too_many_arguments)]
pub fn inspect_resume_with_historical_anchor(
    root: &Path,
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
    anchor: Option<&CheckpointHeadAnchor>,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
) -> Result<ResumeObservation, ResumeError> {
    inspect_historical_io(
        root,
        CheckpointReadIdentity {
            source_revision,
            plan_digest,
            authority_revision,
        },
        anchor,
        verifier,
        now_epoch,
        None,
    )
}

pub fn inspect_resume_with_historical_anchor_with_resources(
    root: &Path,
    identity: CheckpointReadIdentity<'_>,
    anchor: Option<&CheckpointHeadAnchor>,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
    view: &ResourceReservationView<'_>,
) -> Result<ResumeObservation, ResumeError> {
    check_source(view, identity.source_revision)?;
    inspect_historical_io(root, identity, anchor, verifier, now_epoch, Some(view))
}
fn inspect_historical_io(
    root: &Path,
    identity: CheckpointReadIdentity<'_>,
    anchor: Option<&CheckpointHeadAnchor>,
    verifier: Option<&ReceiptVerifier>,
    now_epoch: u64,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<ResumeObservation, ResumeError> {
    let anchor = anchor.ok_or(ResumeError::TrustedHeadMissing)?;
    if now_epoch < anchor.issued_at_epoch {
        return Err(ResumeError::TrustedHeadExpired);
    }
    let mut observation = inspect_anchor_io(
        root,
        identity,
        Some(anchor),
        verifier,
        anchor.issued_at_epoch,
        view,
    )?;
    observation.integrity_state = CheckpointIntegrityState::HistoricalHeadMatched;
    observation.retry_permitted_by_prior_result = false;
    observation.fresh_revalidation_required = true;
    observation.authority_created = false;
    observation.completion_restored = false;
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
        || !valid_digest(&anchor.source_revision)
        || !valid_digest(&anchor.plan_digest)
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
        || anchor.attestation.claims().issued_at_epoch() != anchor.issued_at_epoch
        || anchor.attestation.claims().expires_at_epoch() != Some(anchor.expires_at_epoch)
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
    append_checkpoint_io(root, body, None)
}
pub fn append_checkpoint_with_resources(
    root: &Path,
    body: CheckpointBody,
    view: &ResourceReservationView<'_>,
) -> Result<String, ResumeError> {
    check_source(view, &body.source_revision)?;
    append_checkpoint_io(root, body, Some(view))
}
fn append_checkpoint_io(
    root: &Path,
    body: CheckpointBody,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<String, ResumeError> {
    let _encoding = record_write_claim(view)?;
    check_root(root)?;
    let root = checked_directory(root)?;
    let _lock = writer_lock(&root)?;
    if !checkpoint_body_fields_valid(&body) {
        return Err(ResumeError::CorruptCheckpoint);
    }
    let latest = read_latest_io(&root, view)?;
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
            return Err(ResumeError::SequenceMismatch);
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
    store_record(&target, &encoded, view)?;
    Ok(digest)
}

pub fn inspect_resume(
    root: &Path,
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
) -> Result<ResumeObservation, ResumeError> {
    inspect_resume_io(root, source_revision, plan_digest, authority_revision, None)
}

pub fn inspect_resume_with_resources(
    root: &Path,
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
    view: &ResourceReservationView<'_>,
) -> Result<ResumeObservation, ResumeError> {
    check_source(view, source_revision)?;
    inspect_resume_io(
        root,
        source_revision,
        plan_digest,
        authority_revision,
        Some(view),
    )
}

/// Inspect a caller-supplied ordered sequence of legacy checkpoint bytes.
/// This read-only path performs no filesystem I/O, restores no completion, and
/// always returns Unanchored; callers must freshly validate current tools,
/// authority and any external head before acting on the observation.
pub fn inspect_checkpoint_record_bytes(
    records: &[&[u8]],
    identity: CheckpointReadIdentity<'_>,
) -> Result<ResumeObservation, ResumeError> {
    if records.len() > MAX_RECORDS {
        return Err(ResumeError::BudgetExceeded);
    }
    let mut total_bytes = 0_usize;
    for bytes in records {
        if bytes.is_empty() {
            return Err(ResumeError::CorruptCheckpoint);
        }
        if u64::try_from(bytes.len()).map_err(|_| ResumeError::BudgetExceeded)? > MAX_RECORD_BYTES {
            return Err(ResumeError::BudgetExceeded);
        }
        total_bytes = total_bytes
            .checked_add(bytes.len())
            .filter(|total| *total <= MAX_CHECKPOINT_CHAIN_BYTES)
            .ok_or(ResumeError::BudgetExceeded)?;
    }
    if records.is_empty() {
        return Ok(empty_observation());
    }

    let mut chain = CheckpointChainState::default();

    for (index, bytes) in records.iter().enumerate() {
        let raw = std::str::from_utf8(bytes).map_err(|_| ResumeError::CorruptCheckpoint)?;
        let stored = parse_checkpoint_record(raw)?;
        chain.observe(&stored, index, Some(identity))?;
    }

    let Some(body) = chain.prior_body else {
        return Ok(empty_observation());
    };
    let digest = chain.prior_digest.ok_or(ResumeError::ChainMismatch)?;
    Ok(ResumeObservation {
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
    })
}

fn inspect_resume_io(
    root: &Path,
    source_revision: &str,
    plan_digest: &str,
    authority_revision: &str,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<ResumeObservation, ResumeError> {
    check_view(view)?;
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
    let latest = read_latest_io(root, view)?;
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

/// Shared writer/read validation for fields that cannot be trusted merely
/// because a body digest was recalculated by its caller.
fn checkpoint_body_fields_valid(body: &CheckpointBody) -> bool {
    checkpoint_schema_matches(body)
        && !body.action_key.trim().is_empty()
        && body.action_key.len() <= 1_024
        && !body.action_key.chars().any(char::is_control)
        && valid_digest(&body.source_revision)
        && valid_digest(&body.plan_digest)
        && !body.authority_revision.is_empty()
        && body.authority_revision.len() <= 512
        && !body.authority_revision.chars().any(char::is_control)
        && event_result_is_valid(body)
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

fn read_latest_io(
    root: &Path,
    view: Option<&ResourceReservationView<'_>>,
) -> Result<Option<(CheckpointBody, String)>, ResumeError> {
    check_view(view)?;
    let root = checked_directory(root)?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(&root).map_err(|_| ResumeError::Io)? {
        check_view(view)?;
        let path = entry.map_err(|_| ResumeError::Io)?.path();
        if coordination_lock(&path)? {
            continue;
        }
        if paths.len() >= MAX_RECORDS {
            return Err(ResumeError::BudgetExceeded);
        }
        paths.push(path);
    }
    paths.sort();
    let mut chain = CheckpointChainState::default();
    let mut latest = None;
    for (index, path) in paths.into_iter().enumerate() {
        check_view(view)?;
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
        let input = checkpoint_text(&path, view)?;
        let raw = input.as_str().map_err(|_| ResumeError::CorruptCheckpoint)?;
        let stored = parse_checkpoint_record(raw)?;
        chain.observe(&stored, index, None)?;
        latest = Some((stored.body, stored.digest));
    }
    Ok(latest)
}

/// Shared legacy-wire parser for filesystem and in-memory read-only callers.
/// It preserves the v1/v2 schema and verifies the historical canonical body
/// digest; it emits no fresh authority or external-head result.
fn parse_checkpoint_record(raw: &str) -> Result<StoredCheckpoint, ResumeError> {
    let stored: StoredCheckpoint =
        serde_json::from_str(raw).map_err(|_| ResumeError::CorruptCheckpoint)?;
    if !checkpoint_schema_matches(&stored.body) {
        return Err(ResumeError::ChainMismatch);
    }
    if !checkpoint_body_fields_valid(&stored.body) {
        return Err(ResumeError::CorruptCheckpoint);
    }
    let canonical =
        serde_json::to_string(&stored.body).map_err(|_| ResumeError::CorruptCheckpoint)?;
    if stored.digest != stable_sha256(&canonical) {
        return Err(ResumeError::ChainMismatch);
    }
    Ok(stored)
}

/// Shared ordered-chain validator for filesystem and caller-supplied byte
/// records. The optional identity adds a current-source/plan/authority check;
/// both routes retain schema, digest, sequence, parent, scope and event-state
/// failures before producing a read-only observation.
#[derive(Default)]
struct CheckpointChainState {
    prior_digest: Option<String>,
    prior_schema: Option<String>,
    prior_body: Option<CheckpointBody>,
}

impl CheckpointChainState {
    fn observe(
        &mut self,
        stored: &StoredCheckpoint,
        index: usize,
        identity: Option<CheckpointReadIdentity<'_>>,
    ) -> Result<(), ResumeError> {
        let body = &stored.body;
        let expected_sequence = u64::try_from(index).map_err(|_| ResumeError::BudgetExceeded)?;
        if body.sequence != expected_sequence
            || body.parent_digest != self.prior_digest
            || self
                .prior_schema
                .as_deref()
                .is_some_and(|schema| schema != body.schema_version)
        {
            return Err(ResumeError::ChainMismatch);
        }
        if let Some(identity) = identity {
            if body.source_revision != identity.source_revision {
                return Err(ResumeError::StaleSource);
            }
            if body.plan_digest != identity.plan_digest {
                return Err(ResumeError::StalePlan);
            }
            if body.authority_revision != identity.authority_revision {
                return Err(ResumeError::AuthorityChanged);
            }
        }
        if let Some(previous) = self.prior_body.as_ref() {
            if previous.source_revision != body.source_revision {
                return Err(ResumeError::StaleSource);
            }
            if previous.plan_digest != body.plan_digest {
                return Err(ResumeError::StalePlan);
            }
            if previous.authority_revision != body.authority_revision {
                return Err(ResumeError::AuthorityChanged);
            }
            if previous.action_key == body.action_key {
                validate_event_transition(previous, body)?;
            } else if previous.delivery == DeliveryState::UnknownDelivery {
                return Err(ResumeError::UnknownDelivery);
            }
        }

        self.prior_schema = Some(body.schema_version.clone());
        self.prior_digest = Some(stored.digest.clone());
        self.prior_body = Some(body.clone());
        Ok(())
    }
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
