//! C1-F01: public legacy migration must preserve native history refusals.
//! 全試験は中央実行待ち。fixture result は Given の履歴であり実process観測ではない。
//! Authored, unexecuted by C2. Fixture results are Given history, not process success.
#![forbid(unsafe_code)]

use lc631_analysis::{
    inspect_checkpoint_record_bytes, CheckpointBody, CheckpointEventKind,
    CheckpointIntegrityState, CheckpointReadIdentity, DeliveryState, ResumeError,
    ResumeObservation, DGCL_CHECKPOINT_V2_SCHEMA,
};
use lc631_core::stable_sha256;
use lc631_host::{HostPersistentStoreOwner, PersistentStoreLimits};
use lc631_runtime::journal::{
    decode_segment, inspect_persisted_checkpoint_resume, persist_segment,
    verify_and_persist_legacy_chain, JournalCursor, JournalError, JournalPersistenceError,
    JournalRecordKind, JournalSegmentBuilder, LegacyChainPersistenceDecision,
    LegacyChainPersistenceError, LegacyHoldReason, LegacyJsonMigration,
    PersistedCheckpointResume, PersistedCheckpointResumeHold, MAX_JOURNAL_SEGMENT_BYTES,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_substrate::Digest32;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, time::{Duration, SystemTime, UNIX_EPOCH}};

const SOURCE: &str = "C2 bounded migration history fixture";
const AUTHORITY: &str = "given-history-authority-1";
const SEGMENT: &str = "segment-00000000000000000001.bin";

#[derive(Clone, Copy)]
enum Second {
    DuplicateStart,
    DifferentUnknownKey,
    Observed,
}

struct History {
    source: String,
    plan: String,
    raw: [String; 2],
    final_digest: String,
}

impl History {
    fn new(second: Second) -> Self {
        let source = stable_sha256(SOURCE);
        let plan = stable_sha256("given migration plan");
        let first = CheckpointBody {
            schema_version: DGCL_CHECKPOINT_V2_SCHEMA.into(),
            sequence: 0,
            source_revision: source.clone(),
            plan_digest: plan.clone(),
            authority_revision: AUTHORITY.into(),
            action_key: "given-action-a".into(),
            event_kind: CheckpointEventKind::ActionStarted,
            delivery: DeliveryState::UnknownDelivery,
            result_digest: None,
            parent_digest: None,
        };
        let (first_raw, parent) = seal(&first);
        let mut next = first;
        next.sequence = 1;
        next.parent_digest = Some(parent);
        match second {
            Second::DuplicateStart => {}
            Second::DifferentUnknownKey => next.action_key = "given-action-b".into(),
            Second::Observed => {
                next.event_kind = CheckpointEventKind::ActionObserved;
                next.delivery = DeliveryState::ConfirmedSuccess;
                next.result_digest = Some(stable_sha256("Given result; no process was observed"));
            }
        }
        let (next_raw, final_digest) = seal(&next);
        Self { source, plan, raw: [first_raw, next_raw], final_digest }
    }

    fn records(&self) -> [&[u8]; 2] {
        [self.raw[0].as_bytes(), self.raw[1].as_bytes()]
    }

    fn identity(&self) -> CheckpointReadIdentity<'_> {
        CheckpointReadIdentity {
            source_revision: &self.source,
            plan_digest: &self.plan,
            authority_revision: AUTHORITY,
        }
    }
}

fn seal(body: &CheckpointBody) -> (String, String) {
    let canonical = serde_json::to_string(body).unwrap();
    let digest = stable_sha256(&canonical);
    (format!("{{\"body\":{canonical},\"digest\":\"{digest}\"}}"), digest)
}

fn binary(records: &[&[u8]]) -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::genesis();
    for (index, raw) in records.iter().enumerate() {
        builder.append_at(
            u64::try_from(index).unwrap() + 1,
            JournalRecordKind::Checkpoint,
            Digest32::from_bytes(Sha256::digest(*raw).into()),
            *raw,
        ).unwrap();
    }
    builder.finish().unwrap()
}

fn assert_unanchored(observation: &ResumeObservation) {
    assert_eq!(observation.integrity_state, CheckpointIntegrityState::Unanchored);
    assert!(observation.fresh_revalidation_required);
    assert!(!observation.retry_permitted_by_prior_result);
    assert!(!observation.authority_created);
    assert!(!observation.completion_restored);
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        for attempt in 0..32_u32 {
            let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let root = parent.join(format!(
                "epistesys-beta7-c2-migration-{}-{nonce}-{attempt}", std::process::id(),
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("owned fixture root create failed: {error}"),
            }
        }
        panic!("owned fixture root attempts exhausted")
    }
}

#[cfg(windows)]
fn reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}
#[cfg(not(windows))]
fn reparse(_: &fs::Metadata) -> bool { false }

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("C2 owned failure fixture retained: {}", self.0.display());
            return;
        }
        let Ok(metadata) = fs::symlink_metadata(&self.0) else { return };
        if !metadata.is_dir() || metadata.file_type().is_symlink() || reparse(&metadata) {
            return;
        }
        let (Ok(root), Ok(parent)) = (self.0.canonicalize(), std::env::temp_dir().canonicalize())
        else { return };
        if root.parent() != Some(parent.as_path()) || root != self.0
            || !root.file_name().and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("epistesys-beta7-c2-migration-"))
        { return; }
        let Ok(entries) = fs::read_dir(&root) else { return };
        let mut files = Vec::new();
        for entry in entries {
            let Ok(entry) = entry else { return };
            let name = entry.file_name();
            if name != ".resource-owner-v1.lock" && name != SEGMENT { return; }
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else { return };
            if !metadata.is_file() || metadata.file_type().is_symlink() || reparse(&metadata) {
                return;
            }
            files.push(path);
        }
        for path in files { if fs::remove_file(path).is_err() { return; } }
        let _ = fs::remove_dir(root);
    }
}

fn with_store<T>(scratch: &Scratch, inspect: impl FnOnce(&mut HostPersistentStoreOwner<'_>) -> T) -> T {
    let limits = ResourceLimits::uniform(32 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let mut run = broker.begin_run(
        1, Digest32::from_bytes(Sha256::digest(SOURCE.as_bytes()).into()), limits,
        Deadline::after(Duration::from_secs(30)).unwrap(), Cancellation::default(),
    ).unwrap();
    let mut reservation = run.reserve(&[
        ResourceRequest::new(ResourceKind::Ram, ResourceAmount::checked(8 * 1024 * 1024).unwrap()),
        ResourceRequest::new(ResourceKind::Output, ResourceAmount::checked(8 * 1024 * 1024).unwrap()),
        ResourceRequest::new(ResourceKind::Disk, ResourceAmount::checked(8 * 1024 * 1024).unwrap()),
    ]).unwrap();
    let value = {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            &scratch.0, &scratch.0.join(SEGMENT), MAX_JOURNAL_SEGMENT_BYTES as u64,
            PersistentStoreLimits::default(), &view,
        ).unwrap();
        inspect(&mut owner)
    };
    reservation.release_confirmed().unwrap();
    run.close_confirmed().unwrap();
    for kind in [ResourceKind::Ram, ResourceKind::Output, ResourceKind::Disk] {
        assert_eq!(broker.used(kind).unwrap(), 0);
    }
    value
}

#[test]
fn beta7_migration_rejects_duplicate_action_started() {
    let history = History::new(Second::DuplicateStart);
    let records = history.records();
    assert_eq!(inspect_checkpoint_record_bytes(&records, history.identity()),
        Err(ResumeError::InvalidEventTransition));
    assert_eq!(LegacyJsonMigration::verify_and_migrate(&records).unwrap(),
        LegacyJsonMigration::Hold { reason: LegacyHoldReason::InvalidEventState },
        "same canonical bytes: native InvalidEventTransition must precede migration output");
}

#[test]
fn beta7_migration_rejects_key_change_while_delivery_unknown() {
    let history = History::new(Second::DifferentUnknownKey);
    let records = history.records();
    assert_eq!(inspect_checkpoint_record_bytes(&records, history.identity()),
        Err(ResumeError::UnknownDelivery));
    assert_eq!(LegacyJsonMigration::verify_and_migrate(&records).unwrap(),
        LegacyJsonMigration::Hold { reason: LegacyHoldReason::InvalidEventState },
        "same canonical bytes: UnknownDelivery must not become a migrated history");
}

#[test]
fn beta7_migration_bridge_rejects_duplicate_start_before_segment_write() {
    let history = History::new(Second::DuplicateStart);
    let records = history.records();
    assert_eq!(inspect_checkpoint_record_bytes(&records, history.identity()),
        Err(ResumeError::InvalidEventTransition));
    let scratch = Scratch::new();
    let (decision, target_exists, inventory_count, persisted_history) = with_store(&scratch, |owner| {
        assert!(!owner.target_exists());
        assert!(owner.binary_checkpoint_segment_inventory().unwrap().is_empty());
        let decision = verify_and_persist_legacy_chain(owner, &records, JournalCursor::genesis());
        let target_exists = scratch.0.join(SEGMENT).is_file();
        let inventory_count = owner.binary_checkpoint_segment_inventory().map(|items| items.len());
        let persisted_history = target_exists.then(||
            inspect_persisted_checkpoint_resume(owner, JournalCursor::genesis(), history.identity()));
        (decision, target_exists, inventory_count, persisted_history)
    });
    assert_eq!(decision.unwrap(),
        LegacyChainPersistenceDecision::Hold { reason: LegacyHoldReason::InvalidEventState },
        "native InvalidEventTransition before write; target={target_exists}, inventory={inventory_count:?}, downstream={persisted_history:?}");
    assert!(!target_exists);
    assert_eq!(inventory_count.unwrap(), 0);
}

#[test]
fn beta7_given_observed_history_roundtrips_unanchored() {
    let history = History::new(Second::Observed);
    let records = history.records();
    let native = inspect_checkpoint_record_bytes(&records, history.identity()).unwrap();
    assert_unanchored(&native);
    assert_eq!(native.latest_sequence, Some(1));
    assert_eq!(native.latest_digest.as_deref(), Some(history.final_digest.as_str()));
    assert_eq!(native.latest_delivery, Some(DeliveryState::ConfirmedSuccess));
    assert_eq!(native.latest_event_kind, Some(CheckpointEventKind::ActionObserved));
    let migrated = LegacyJsonMigration::verify_and_migrate(&records).unwrap();
    let LegacyJsonMigration::MigratedUnanchored { binary_segment, record_count, final_legacy_digest } = migrated
    else { panic!("valid Given ActionObserved chain must migrate") };
    assert_eq!(record_count, 2);
    assert_eq!(final_legacy_digest, history.final_digest);
    let decoded = decode_segment(&binary_segment).unwrap();
    assert_eq!(decoded.records().len(), 2);
    for (record, raw) in decoded.records().iter().zip(&records) {
        assert_eq!(record.kind(), JournalRecordKind::Checkpoint);
        assert_eq!(record.payload(), *raw);
    }
    let scratch = Scratch::new();
    let observed = with_store(&scratch, |owner| {
        let decision = verify_and_persist_legacy_chain(owner, &records, JournalCursor::genesis()).unwrap();
        let LegacyChainPersistenceDecision::Persisted { legacy_final_digest, record_count, storage } = decision
        else { panic!("valid Given history must persist through the public bridge") };
        assert_eq!(legacy_final_digest, history.final_digest);
        assert_eq!(record_count, 2);
        assert_eq!(storage.record_count(), 2);
        assert_eq!(owner.binary_checkpoint_segment_inventory().unwrap().len(), 1);
        let read = owner.read_binary_checkpoint_segment().unwrap();
        assert_eq!(read.as_bytes(), binary_segment.as_slice());
        drop(read);
        inspect_persisted_checkpoint_resume(owner, JournalCursor::genesis(), history.identity())
    });
    assert_eq!(observed, PersistedCheckpointResume::Candidate(native));
}

#[test]
fn beta7_unknown_delivery_remains_unknown_without_retry_or_completion() {
    let history = History::new(Second::DuplicateStart);
    let records = [history.raw[0].as_bytes()];
    let observed = inspect_checkpoint_record_bytes(&records, history.identity()).unwrap();
    assert_unanchored(&observed);
    assert_eq!(observed.latest_delivery, Some(DeliveryState::UnknownDelivery));
    assert_eq!(observed.latest_event_kind, Some(CheckpointEventKind::ActionStarted));
}

#[test]
fn beta7_binary_storage_preserves_typed_historical_refusals() {
    for (case, expected) in [
        (Second::DuplicateStart, ResumeError::InvalidEventTransition),
        (Second::DifferentUnknownKey, ResumeError::UnknownDelivery),
    ] {
        let history = History::new(case);
        let records = history.records();
        assert_eq!(inspect_checkpoint_record_bytes(&records, history.identity()), Err(expected.clone()));
        let bytes = binary(&records);
        let scratch = Scratch::new();
        let refusal = with_store(&scratch, |owner| {
            let receipt = persist_segment(owner, &bytes, JournalCursor::genesis()).unwrap();
            assert_eq!(receipt.record_count(), 2);
            let inventory = owner.binary_checkpoint_segment_inventory().unwrap();
            assert_eq!(inventory.len(), 1);
            let stored = owner.read_binary_checkpoint_segment_descriptor(&inventory[0]).unwrap();
            assert_eq!(stored.as_bytes(), bytes.as_slice());
            drop(stored);
            drop(inventory);
            inspect_persisted_checkpoint_resume(owner, JournalCursor::genesis(), history.identity())
        });
        assert_eq!(refusal,
            PersistedCheckpointResume::Hold(PersistedCheckpointResumeHold::Analysis(expected)));
        assert!(scratch.0.join(SEGMENT).is_file(), "read-only refusal must retain actual stored bytes");
    }
}

#[test]
fn beta7_migration_bridge_preserves_predecessor_failure_before_write() {
    let history = History::new(Second::Observed);
    let records = history.records();
    let bytes = binary(&records);
    let non_genesis = decode_segment(&bytes).unwrap().cursor();
    let scratch = Scratch::new();
    let (result, target_exists, inventory_count) = with_store(&scratch, |owner| {
        let result = verify_and_persist_legacy_chain(owner, &records, non_genesis);
        (result, owner.target_exists(), owner.binary_checkpoint_segment_inventory().unwrap().len())
    });
    assert_eq!(result, Err(LegacyChainPersistenceError::Persistence(
        JournalPersistenceError::PredecessorMismatch(JournalError::StalePredecessor))));
    assert!(!target_exists);
    assert!(!scratch.0.join(SEGMENT).exists());
    assert_eq!(inventory_count, 0);
}

#[test]
fn beta7_migration_three_mib_limit_precedes_record_parsing() {
    let record = vec![b'x'; 16 * 1024];
    let at_limit = vec![record.as_slice(); 192];
    assert_eq!(at_limit.iter().map(|raw| raw.len()).sum::<usize>(), 3 * 1024 * 1024);
    assert_eq!(LegacyJsonMigration::verify_and_migrate(&at_limit).unwrap(),
        LegacyJsonMigration::Hold { reason: LegacyHoldReason::UnsupportedLegacyEncoding });
    let mut over_limit = at_limit;
    over_limit.push(&record[..1]);
    assert_eq!(LegacyJsonMigration::verify_and_migrate(&over_limit).unwrap(),
        LegacyJsonMigration::Hold { reason: LegacyHoldReason::InputLimitExceeded });
}
