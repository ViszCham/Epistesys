use lc631_host::{HostPersistentStoreOwner, PersistentStoreLimits};
use lc631_runtime::journal::{
    decode_segment, inspect_legacy_json_read_only, persist_segment,
    verify_and_persist_legacy_chain, JournalCursor, JournalError, JournalHeadCheck,
    JournalPersistenceError, JournalRecordKind, JournalSegmentBuilder,
    LegacyChainPersistenceDecision, LegacyHoldReason, LegacyJsonMigration, RetentionDecision,
    JOURNAL_RECORDS_SECTION_TAG, MAX_JOURNAL_RECORD_BYTES, MAX_JOURNAL_SEGMENT_BYTES,
    RECORD_HEADER_BYTES,
};
use lc631_runtime::resource::{
    Cancellation, Deadline, ResourceAmount, ResourceBroker, ResourceKind, ResourceLimits,
    ResourceRequest,
};
use lc631_substrate::codec::{decode_envelope, encode_frame, SectionInput};
use lc631_substrate::Digest32;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn digest(byte: u8) -> Digest32 {
    Digest32::from_bytes([byte; 32])
}

struct TestScratchRoot(PathBuf);

impl TestScratchRoot {
    fn create() -> Self {
        let parent = std::env::temp_dir()
            .canonicalize()
            .expect("test temporary parent must be accessible");
        for attempt in 0..32_u32 {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .saturating_add(u128::from(attempt));
            let path = parent.join(format!(
                "epistesys-tlcx07-journal-{}-{nonce}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test-owned scratch root create failed: {error}"),
            }
        }
        panic!("could not allocate a unique test scratch root")
    }
}

impl Drop for TestScratchRoot {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.0) else {
            return;
        };
        if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return;
        }
        let (Ok(root), Ok(parent)) = (self.0.canonicalize(), std::env::temp_dir().canonicalize())
        else {
            return;
        };
        if root.parent() != Some(parent.as_path())
            || !root
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("epistesys-tlcx07-journal-"))
        {
            return;
        }
        let Ok(entries) = fs::read_dir(&root) else {
            return;
        };
        let mut owned_files = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name != ".resource-owner-v1.lock"
                && name != "segment-00000000000000000001.bin"
                && name != "segment-00000000000000000002.bin"
                && name != "segment-00000000000000000003.bin"
            {
                return;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                return;
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return;
            }
            owned_files.push(path);
        }
        for path in owned_files {
            if fs::remove_file(path).is_err() {
                return;
            }
        }
        let _ = fs::remove_dir(root);
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

fn runtime_resource_owner(
    source: Digest32,
) -> (
    ResourceBroker,
    lc631_runtime::resource::RunResources,
    lc631_runtime::resource::Reservation,
) {
    let limits = ResourceLimits::uniform(64 * 1024 * 1024).unwrap();
    let broker = ResourceBroker::new(limits).unwrap();
    let run = broker
        .begin_run(
            1,
            source,
            limits,
            Deadline::after(Duration::from_secs(30)).unwrap(),
            Cancellation::default(),
        )
        .unwrap();
    let reservation = run
        .reserve(&[
            ResourceRequest::new(
                ResourceKind::Disk,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Ram,
                ResourceAmount::checked(16 * 1024 * 1024).unwrap(),
            ),
            ResourceRequest::new(
                ResourceKind::Output,
                ResourceAmount::checked(8 * 1024 * 1024).unwrap(),
            ),
        ])
        .unwrap();
    (broker, run, reservation)
}

fn one_record_segment(kind: JournalRecordKind) -> Vec<u8> {
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(1, kind, digest(9), b"bounded-payload")
        .expect("valid fixture record");
    builder.finish().expect("bounded frame")
}

fn malformed_record_frame(declared_payload_bytes: u32, actual_payload: &[u8]) -> Vec<u8> {
    let mut record = vec![0_u8; RECORD_HEADER_BYTES];
    record[..8].copy_from_slice(b"EJREC001");
    record[8..10].copy_from_slice(&1_u16.to_le_bytes());
    record[10..12].copy_from_slice(&6_u16.to_le_bytes());
    record[16..24].copy_from_slice(&1_u64.to_le_bytes());
    record[24..28].copy_from_slice(&declared_payload_bytes.to_le_bytes());
    record[32..64].copy_from_slice(digest(9).as_bytes());
    record.extend_from_slice(actual_payload);
    encode_frame(&[SectionInput {
        tag: JOURNAL_RECORDS_SECTION_TAG,
        version: 1,
        flags: 0,
        count: 1,
        payload: &record,
    }])
    .expect("valid outer frame containing intentionally bad record")
}

#[test]
fn tlcx07_records_roundtrip_through_the_shared_epitlcx1_codec() {
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(
            1,
            JournalRecordKind::ActionStarted,
            digest(1),
            b"operation-start",
        )
        .unwrap();
    builder
        .append_at(
            2,
            JournalRecordKind::Evidence,
            digest(2),
            b"independently-bound-evidence-reference",
        )
        .unwrap();

    let bytes = builder.finish().unwrap();
    let view = decode_segment(&bytes).unwrap();
    assert_eq!(view.records().len(), 2);
    assert_eq!(view.records()[0].sequence(), 1);
    assert_eq!(view.records()[0].kind(), JournalRecordKind::ActionStarted);
    assert_eq!(view.records()[0].object_digest(), digest(1));
    assert_eq!(view.records()[0].payload_offset(), RECORD_HEADER_BYTES);
    assert_eq!(view.records()[0].payload(), b"operation-start");
    assert_eq!(view.records()[1].sequence(), 2);
    assert_eq!(
        view.records()[1].previous_digest(),
        view.records()[0].digest()
    );
    assert_eq!(view.records()[1].kind(), JournalRecordKind::Evidence);
    assert_eq!(
        view.records()[1].payload(),
        b"independently-bound-evidence-reference"
    );
    assert_eq!(view.first_previous_cursor(), JournalCursor::genesis());
    assert!(view.frame_digest() != digest(0));
}

#[test]
fn tlcx07_torn_inner_record_is_held_even_when_outer_frame_is_valid() {
    let frame = malformed_record_frame(2, b"x");
    assert_eq!(
        decode_segment(&frame).unwrap_err(),
        JournalError::TornRecord
    );
}

#[test]
fn tlcx07_record_length_overflow_is_rejected_before_payload_allocation() {
    let frame = malformed_record_frame(u32::MAX, b"");
    assert_eq!(
        decode_segment(&frame).unwrap_err(),
        JournalError::RecordTooLarge
    );
    const {
        assert!(MAX_JOURNAL_RECORD_BYTES < lc631_substrate::codec::MAX_FRAME_BYTES);
    }
}

#[test]
fn tlcx07_untrusted_section_count_is_bounded_by_payload_before_index_reserve() {
    let valid = one_record_segment(JournalRecordKind::Evidence);
    let decoded = decode_envelope(&valid).unwrap();
    let record_payload = decoded.sections()[0].payload().to_vec();
    let malformed = encode_frame(&[SectionInput {
        tag: JOURNAL_RECORDS_SECTION_TAG,
        version: 1,
        flags: 0,
        count: 2,
        payload: &record_payload,
    }])
    .unwrap();

    assert_eq!(
        decode_segment(&malformed).unwrap_err(),
        JournalError::RecordCountMismatch
    );
}

#[test]
fn tlcx07_duplicate_and_nonmonotonic_sequences_never_publish() {
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(1, JournalRecordKind::Checkpoint, digest(3), b"one")
        .unwrap();
    assert_eq!(
        builder.append_at(1, JournalRecordKind::Checkpoint, digest(4), b"duplicate"),
        Err(JournalError::NonMonotonicSequence)
    );
    assert_eq!(
        builder.append_at(3, JournalRecordKind::Checkpoint, digest(5), b"gap"),
        Err(JournalError::SequenceGap)
    );
    let bytes = builder.finish().unwrap();
    let decoded = decode_segment(&bytes).unwrap();
    assert_eq!(decoded.records().len(), 1);
}

#[test]
fn tlcx07_empty_object_digest_is_not_an_implicit_missing_reference() {
    let mut builder = JournalSegmentBuilder::genesis();
    assert_eq!(
        builder.append_at(
            1,
            JournalRecordKind::Evidence,
            Digest32::from_bytes([0; 32]),
            b"payload",
        ),
        Err(JournalError::InvalidObjectDigest)
    );
    assert_eq!(builder.finish(), Err(JournalError::EmptySegment));
}

#[test]
fn tlcx07_stale_and_missing_heads_remain_distinct_from_external_anchoring() {
    let first_bytes = one_record_segment(JournalRecordKind::Evidence);
    let first = decode_segment(&first_bytes).unwrap();
    let old_head = first.local_head();
    let mut next_builder = JournalSegmentBuilder::after(first.cursor());
    next_builder
        .append_at(2, JournalRecordKind::Checkpoint, digest(8), b"next-segment")
        .unwrap();
    let next_bytes = next_builder.finish().unwrap();
    let next = decode_segment(&next_bytes).unwrap();

    assert_eq!(next.verify_predecessor(first.cursor()), Ok(()));
    assert_eq!(
        next.verify_predecessor(JournalCursor::genesis()),
        Err(JournalError::StalePredecessor)
    );
    assert_eq!(next.cursor().record_digest(), next.records()[0].digest());
    assert_eq!(next.check_local_head(None), JournalHeadCheck::Unanchored);
    assert_eq!(
        next.check_local_head(Some(&old_head)),
        JournalHeadCheck::Stale
    );
    let current_head = next.local_head();
    assert_eq!(current_head.sequence(), 2);
    assert_eq!(current_head.segment_digest(), next.frame_digest());
    assert_eq!(current_head.record_digest(), next.cursor().record_digest());
    assert_eq!(
        next.check_local_head(Some(&current_head)),
        JournalHeadCheck::LocalMatchUnanchored
    );
    assert_eq!(JournalCursor::genesis().sequence(), 0);
}

#[test]
fn tlcx07_evidence_is_not_reclaimable_without_a_verified_archive() {
    let bytes = one_record_segment(JournalRecordKind::Evidence);
    let view = decode_segment(&bytes).unwrap();
    assert_eq!(
        view.records()[0].retention_decision(),
        RetentionDecision::RetainEvidenceWithoutVerifiedArchive
    );
    assert!(view.reclaimable_indices().is_empty());
}

#[test]
fn tlcx07_legacy_json_migration_stays_read_only_and_explicitly_held() {
    assert_eq!(
        inspect_legacy_json_read_only(br#"{"schema":"legacy"}"#, 1024).unwrap(),
        LegacyJsonMigration::Hold {
            reason: LegacyHoldReason::LegacyChainRequired
        }
    );
    assert_eq!(
        inspect_legacy_json_read_only(b"too-large", 1),
        Err(JournalError::InputTooLarge)
    );
}

fn legacy_checkpoint(sequence: u64, parent: Option<&str>, delivery: &str) -> (String, String) {
    let source = format!("sha256:{}", "a".repeat(64));
    let plan = format!("sha256:{}", "b".repeat(64));
    let result = if delivery == "unknown_delivery" {
        "null".to_string()
    } else {
        format!("\"sha256:{}\"", "c".repeat(64))
    };
    let parent = parent.map_or_else(|| "null".to_string(), |value| format!("\"{value}\""));
    let body = format!(
        "{{\"schema_version\":\"epistesys-dgcl-checkpoint.v1\",\"sequence\":{sequence},\"source_revision\":\"{source}\",\"plan_digest\":\"{plan}\",\"authority_revision\":\"auth-1\",\"action_key\":\"repair-a\",\"delivery\":\"{delivery}\",\"result_digest\":{result},\"parent_digest\":{parent}}}"
    );
    let digest = lc631_core::stable_sha256(&body);
    let stored = format!("{{\"body\":{body},\"digest\":\"{digest}\"}}");
    (stored, digest)
}

#[test]
fn tlcx07_legacy_chain_digest_and_order_fail_closed_before_migration() {
    let (first, first_digest) = legacy_checkpoint(0, None, "unknown_delivery");
    let (second, _) = legacy_checkpoint(1, Some(&first_digest), "confirmed_success");
    let mut bad_digest = second.clone();
    let last = bad_digest.pop().unwrap();
    let digest_position = bad_digest.rfind(&"c".repeat(64)).unwrap();
    bad_digest.replace_range(digest_position..digest_position + 1, "d");
    bad_digest.push(last);
    assert!(matches!(
        LegacyJsonMigration::verify_and_migrate(&[first.as_bytes(), bad_digest.as_bytes()]),
        Ok(LegacyJsonMigration::Hold {
            reason: LegacyHoldReason::DigestMismatch
        })
    ));

    assert!(matches!(
        LegacyJsonMigration::verify_and_migrate(&[second.as_bytes(), first.as_bytes()]),
        Ok(LegacyJsonMigration::Hold {
            reason: LegacyHoldReason::SequenceMismatch
        })
    ));
}

#[test]
fn tlcx07_legacy_json_oversize_stays_hold_without_partial_migration() {
    let oversize = vec![b'x'; 16 * 1024 + 1];
    assert!(matches!(
        LegacyJsonMigration::verify_and_migrate(&[&oversize]),
        Ok(LegacyJsonMigration::Hold {
            reason: LegacyHoldReason::InputLimitExceeded
        })
    ));
}

#[test]
fn tlcx07_verified_legacy_chain_migrates_original_bytes_as_unanchored_binary() {
    let (first, first_digest) = legacy_checkpoint(0, None, "unknown_delivery");
    let (second, second_digest) = legacy_checkpoint(1, Some(&first_digest), "confirmed_success");
    let migrated =
        LegacyJsonMigration::verify_and_migrate(&[first.as_bytes(), second.as_bytes()]).unwrap();
    let debug = format!("{migrated:?}");
    assert!(debug.contains("binary_segment_bytes"));
    assert!(!debug.contains("repair-a"));
    assert!(debug.len() < 512);
    match migrated {
        LegacyJsonMigration::MigratedUnanchored {
            binary_segment,
            record_count,
            final_legacy_digest,
        } => {
            assert_eq!(record_count, 2);
            assert_eq!(final_legacy_digest, second_digest);
            let decoded = decode_segment(&binary_segment).unwrap();
            assert_eq!(decoded.records().len(), 2);
            assert_eq!(decoded.records()[0].kind(), JournalRecordKind::Checkpoint);
            assert_eq!(decoded.records()[0].payload(), first.as_bytes());
            assert_eq!(decoded.records()[1].kind(), JournalRecordKind::Checkpoint);
            assert_eq!(decoded.records()[1].payload(), second.as_bytes());
        }
        LegacyJsonMigration::Hold { reason } => panic!("valid legacy chain held: {reason:?}"),
    }
}

#[test]
fn tlcx07_verified_legacy_migration_persists_only_after_the_complete_chain_passes() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target = root.join("segment-00000000000000000001.bin");
    let (first, first_digest) = legacy_checkpoint(0, None, "unknown_delivery");
    let (second, second_digest) = legacy_checkpoint(1, Some(&first_digest), "confirmed_success");
    let mut invalid_second = second.clone();
    let position = invalid_second.rfind(&second_digest).unwrap() + "sha256:".len();
    let first_hex_digit = second_digest.as_bytes()["sha256:".len()];
    let replacement = if first_hex_digit == b'0' { "1" } else { "0" };
    invalid_second.replace_range(position..position + 1, replacement);
    let (_broker, _run, mut reservation) = runtime_resource_owner(digest(0x56));

    let result = {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            MAX_JOURNAL_SEGMENT_BYTES as u64,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        let rejected = verify_and_persist_legacy_chain(
            &mut owner,
            &[first.as_bytes(), invalid_second.as_bytes()],
            JournalCursor::genesis(),
        )
        .unwrap();
        assert_eq!(
            rejected,
            LegacyChainPersistenceDecision::Hold {
                reason: LegacyHoldReason::DigestMismatch
            }
        );
        assert!(!owner.target_exists());
        assert!(!target.exists());

        let migrated = verify_and_persist_legacy_chain(
            &mut owner,
            &[first.as_bytes(), second.as_bytes()],
            JournalCursor::genesis(),
        )
        .unwrap();
        let persisted = match migrated {
            LegacyChainPersistenceDecision::Persisted {
                legacy_final_digest,
                record_count,
                storage,
            } => {
                assert_eq!(legacy_final_digest, second_digest);
                assert_eq!(record_count, 2);
                assert_eq!(storage.record_count(), 2);
                storage
            }
            other => panic!("verified legacy chain should persist: {other:?}"),
        };
        assert!(owner.target_exists());

        let current_source = format!("sha256:{}", "a".repeat(64));
        let current_plan = format!("sha256:{}", "b".repeat(64));
        let identity = lc631_analysis::CheckpointReadIdentity {
            source_revision: &current_source,
            plan_digest: &current_plan,
            authority_revision: "auth-1",
        };
        let resume = lc631_runtime::journal::inspect_persisted_checkpoint_resume(
            &owner,
            JournalCursor::genesis(),
            identity,
        );
        let observation = match resume {
            lc631_runtime::journal::PersistedCheckpointResume::Candidate(observation) => {
                observation
            }
            other => panic!("verified stored checkpoints should remain inspectable: {other:?}"),
        };
        assert_eq!(
            observation.integrity_state,
            lc631_analysis::CheckpointIntegrityState::Unanchored
        );
        assert_eq!(observation.latest_sequence, Some(1));
        assert_eq!(
            observation.latest_digest.as_deref(),
            Some(second_digest.as_str())
        );
        assert!(observation.fresh_revalidation_required);
        assert!(!observation.authority_created);
        assert!(!observation.completion_restored);

        let stale_identity = lc631_analysis::CheckpointReadIdentity {
            source_revision: "sha256:stale-source",
            plan_digest: &current_plan,
            authority_revision: "auth-1",
        };
        assert!(matches!(
            lc631_runtime::journal::inspect_persisted_checkpoint_resume(
                &owner,
                JournalCursor::genesis(),
                stale_identity,
            ),
            lc631_runtime::journal::PersistedCheckpointResume::Hold(
                lc631_runtime::journal::PersistedCheckpointResumeHold::Analysis(
                    lc631_analysis::ResumeError::StaleSource
                )
            )
        ));
        persisted
    };

    reservation.release_confirmed().unwrap();
    let persisted_bytes = fs::read(&target).unwrap();
    let decoded = decode_segment(&persisted_bytes).unwrap();
    assert_eq!(decoded.records().len(), 2);
    assert_eq!(decoded.records()[0].payload(), first.as_bytes());
    assert_eq!(decoded.records()[1].payload(), second.as_bytes());
    assert_eq!(result.record_count(), 2);
    assert!(result.claim_boundary().contains("external anchor"));
}

#[test]
fn tlcx07_validated_segment_roundtrips_through_the_owned_binary_file_profile() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target_one = root.join("segment-00000000000000000001.bin");
    let target_two = root.join("segment-00000000000000000002.bin");
    let source = digest(0x55);
    let (_broker, _run, mut reservation) = runtime_resource_owner(source);
    let payload = [0_u8, 0xff, 0x80, b'{', b'}'];
    let mut builder = JournalSegmentBuilder::genesis();
    builder
        .append_at(1, JournalRecordKind::Evidence, digest(0x61), &payload)
        .unwrap();
    let bytes = builder.finish().unwrap();
    let expected_storage_digest = lc631_receipt_kernel::digest_bytes(&bytes);

    {
        let view = reservation.lease();
        let planned_bytes = u64::try_from(bytes.len()).unwrap();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_one,
            planned_bytes,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        let wrong_predecessor = decode_segment(&one_record_segment(JournalRecordKind::Checkpoint))
            .unwrap()
            .cursor();
        assert_eq!(
            persist_segment(&mut owner, &bytes, wrong_predecessor),
            Err(JournalPersistenceError::PredecessorMismatch(
                JournalError::StalePredecessor
            ))
        );
        assert!(
            !owner.target_exists(),
            "rejected chain must not create a segment"
        );
        let persisted = persist_segment(&mut owner, &bytes, JournalCursor::genesis()).unwrap();
        assert_eq!(persisted.head().sequence(), 1);
        assert_eq!(persisted.record_count(), 1);
        assert_eq!(persisted.storage_digest(), expected_storage_digest);
        assert!(persisted.claim_boundary().contains("no external anchor"));
        assert!(owner.target_exists());
        assert_eq!(owner.usage().total_bytes, planned_bytes);

        // Rescanning through a second owner proves the stored opaque segment is
        // admitted as raw bytes (including invalid UTF-8), not parsed as JSON.
        let previous_cursor = decode_segment(&bytes).unwrap().cursor();
        let mut second_builder = JournalSegmentBuilder::after(previous_cursor);
        second_builder
            .append_at(2, JournalRecordKind::Checkpoint, digest(0x62), b"next")
            .unwrap();
        let second_bytes = second_builder.finish().unwrap();
        drop(owner);
        let mut next = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_two,
            1,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(!next.target_exists());
        assert_eq!(next.usage().total_bytes, planned_bytes);
        assert!(matches!(
            persist_segment(&mut next, &second_bytes, previous_cursor),
            Err(JournalPersistenceError::StorageRejectedBeforeWrite(_))
        ));
        assert!(
            !next.target_exists(),
            "underfunded segment must not publish a file"
        );
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_append_rejects_a_caller_cursor_that_forks_the_existing_root() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target_one = root.join("segment-00000000000000000001.bin");
    let target_two = root.join("segment-00000000000000000002.bin");
    let first = one_record_segment(JournalRecordKind::Evidence);
    let first_cursor = decode_segment(&first).unwrap().cursor();

    let mut fork = JournalSegmentBuilder::genesis();
    fork.append_at(1, JournalRecordKind::Checkpoint, digest(0x71), b"fork")
        .unwrap();
    let fork = fork.finish().unwrap();
    let mut next = JournalSegmentBuilder::after(first_cursor);
    next.append_at(2, JournalRecordKind::Checkpoint, digest(0x72), b"next")
        .unwrap();
    let next = next.finish().unwrap();
    let (_broker, _run, mut reservation) = runtime_resource_owner(digest(0x73));

    {
        let view = reservation.lease();
        let mut first_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_one,
            u64::try_from(first.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        persist_segment(&mut first_owner, &first, JournalCursor::genesis()).unwrap();
        drop(first_owner);

        let mut second_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_two,
            u64::try_from(fork.len().max(next.len())).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert_eq!(
            persist_segment(&mut second_owner, &fork, JournalCursor::genesis()),
            Err(JournalPersistenceError::PredecessorMismatch(
                JournalError::StalePredecessor
            ))
        );
        assert!(
            !second_owner.target_exists(),
            "a fork must not create its target"
        );
        assert!(!target_two.exists(), "the fork must not create a file");
        let stored = persist_segment(&mut second_owner, &next, first_cursor).unwrap();
        assert_eq!(stored.head().sequence(), 2);
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_append_holds_when_an_existing_root_segment_is_corrupt() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target_one = root.join("segment-00000000000000000001.bin");
    let target_two = root.join("segment-00000000000000000002.bin");
    let candidate = one_record_segment(JournalRecordKind::Evidence);
    let (_broker, _run, mut reservation) = runtime_resource_owner(digest(0x74));

    {
        let view = reservation.lease();
        let mut first_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_one,
            16,
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        first_owner
            .write_binary_checkpoint_segment(b"invalid-frame")
            .unwrap();
        drop(first_owner);

        let mut second_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_two,
            u64::try_from(candidate.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert!(matches!(
            persist_segment(&mut second_owner, &candidate, JournalCursor::genesis()),
            Err(JournalPersistenceError::InvalidSegment(_))
        ));
        assert!(
            !second_owner.target_exists(),
            "a corrupt root must not publish a target"
        );
        assert!(
            !target_two.exists(),
            "the corrupt root must not create a file"
        );
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_append_rejects_a_gap_in_the_target_file_sequence() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target_one = root.join("segment-00000000000000000001.bin");
    let target_three = root.join("segment-00000000000000000003.bin");
    let first = one_record_segment(JournalRecordKind::Evidence);
    let first_cursor = decode_segment(&first).unwrap().cursor();
    let mut next = JournalSegmentBuilder::after(first_cursor);
    next.append_at(2, JournalRecordKind::Checkpoint, digest(0x75), b"next")
        .unwrap();
    let next = next.finish().unwrap();
    let (_broker, _run, mut reservation) = runtime_resource_owner(digest(0x76));

    {
        let view = reservation.lease();
        let mut first_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_one,
            u64::try_from(first.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        persist_segment(&mut first_owner, &first, JournalCursor::genesis()).unwrap();
        drop(first_owner);

        let mut gap_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_three,
            u64::try_from(next.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        assert_eq!(
            persist_segment(&mut gap_owner, &next, first_cursor),
            Err(JournalPersistenceError::PredecessorMismatch(
                JournalError::SequenceGap
            ))
        );
        assert!(
            !target_three.exists(),
            "a sequence gap must not create a file"
        );
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_append_revalidates_a_max_record_segment_with_the_streaming_path() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target_one = root.join("segment-00000000000000000001.bin");
    let target_two = root.join("segment-00000000000000000002.bin");
    let mut builder = JournalSegmentBuilder::genesis();
    for sequence in 1..=16_384_u64 {
        builder
            .append_at(sequence, JournalRecordKind::Evidence, digest(0x77), &[])
            .unwrap();
    }
    let first = builder.finish().unwrap();
    let first_cursor = decode_segment(&first).unwrap().cursor();
    let mut next = JournalSegmentBuilder::after(first_cursor);
    next.append_at(16_385, JournalRecordKind::Checkpoint, digest(0x78), b"next")
        .unwrap();
    let next = next.finish().unwrap();
    let (_broker, _run, mut reservation) = runtime_resource_owner(digest(0x79));

    {
        let view = reservation.lease();
        let mut first_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_one,
            u64::try_from(first.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        persist_segment(&mut first_owner, &first, JournalCursor::genesis()).unwrap();
        drop(first_owner);

        let mut second_owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target_two,
            u64::try_from(next.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        let stored = persist_segment(&mut second_owner, &next, first_cursor).unwrap();
        assert_eq!(stored.head().sequence(), 16_385);
        assert!(stored.claim_boundary().contains("no external anchor"));
    }
    reservation.release_confirmed().unwrap();
}

#[test]
fn tlcx07_checkpoint_resume_rejects_record_count_before_payload_validation() {
    let scratch = TestScratchRoot::create();
    let root = &scratch.0;
    let target = root.join("segment-00000000000000000001.bin");
    let mut builder = JournalSegmentBuilder::genesis();
    for sequence in 1..=1_001_u64 {
        builder
            .append_at(sequence, JournalRecordKind::Checkpoint, digest(0x7a), b"{}")
            .unwrap();
    }
    let bytes = builder.finish().unwrap();
    let (_broker, _run, mut reservation) = runtime_resource_owner(digest(0x7b));

    {
        let view = reservation.lease();
        let mut owner = HostPersistentStoreOwner::begin_binary_checkpoint_segment(
            root,
            &target,
            u64::try_from(bytes.len()).unwrap(),
            PersistentStoreLimits::default(),
            &view,
        )
        .unwrap();
        persist_segment(&mut owner, &bytes, JournalCursor::genesis()).unwrap();
        let identity = lc631_analysis::CheckpointReadIdentity {
            source_revision: "source",
            plan_digest: "plan",
            authority_revision: "authority",
        };
        assert_eq!(
            lc631_runtime::journal::inspect_persisted_checkpoint_resume(
                &owner,
                JournalCursor::genesis(),
                identity,
            ),
            lc631_runtime::journal::PersistedCheckpointResume::Hold(
                lc631_runtime::journal::PersistedCheckpointResumeHold::Analysis(
                    lc631_analysis::ResumeError::BudgetExceeded
                )
            )
        );
    }
    reservation.release_confirmed().unwrap();
}
